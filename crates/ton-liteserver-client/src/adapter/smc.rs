#[cfg(feature = "emulator")]
use crate::api::{AccountClient, ConfigClient, MasterchainConfig};
use crate::client::LiteServerClient;
#[cfg(not(feature = "emulator"))]
use crate::tl::{Int31, LiteServerAccountId, LiteServerRunSmcMethod, TonNodeBlockIdExt};
#[cfg(feature = "emulator")]
use crate::tl::{LiteServerGetLibraries, TonNodeBlockIdExt};
#[cfg(feature = "emulator")]
use crate::tlb::hashmap::{Hashmap, HashmapAugNode, HashmapNode, HashmapTree};
use crate::tlb::vm_stack::{VmCellSlice, VmStack, VmStackValue, VmStkTuple};
use anyhow::{Result, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as base64_standard;
use crc::Crc;
use num_bigint::BigUint;
#[cfg(feature = "emulator")]
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;
use ton_address::SmartContractAddress;
use ton_tower::response::{SmcRunResult, StackEntry};
use toner::tlb::ser::CellSerializeExt;
#[cfg(feature = "emulator")]
use toner::tlb::ser::CellSerializeWrapAsExt;
use toner::tlb::{BagOfCellsArgs, BoC};
#[cfg(feature = "emulator")]
use toner::tlb::{Ref, Same};
use tower::ServiceExt;

// `mode.2` -> include `result` (serialized VM stack) in the response.
// We do not request any proofs (`mode.0`/`mode.1`) or auxiliary VM context
// (`mode.3` init_c7, `mode.4` lib_extras) here. Proof verification can be added
// later, analogous to `verify_account_proofs` in `account.rs`.
#[cfg(not(feature = "emulator"))]
const RUN_METHOD_MODE_RESULT: Int31 = 0x4;

// The default remote path cannot report gas because liteServer.runSmcMethod
// returns only exit_code and result. The optional emulator path reports real gas.
#[cfg(any(not(feature = "emulator"), test))]
pub(super) const GAS_USED_PLACEHOLDER: i64 = 0;

const CRC16: Crc<u16> = Crc::<u16>::new(&crc::CRC_16_XMODEM);

fn method_id_from_name(name: &str) -> i64 {
    (CRC16.checksum(name.as_bytes()) as i64) | 0x10000
}

#[cfg(not(feature = "emulator"))]
pub(super) async fn run_get_method_inner(
    client: LiteServerClient,
    address: SmartContractAddress,
    block_id: TonNodeBlockIdExt,
    method: &str,
    stack: Vec<StackEntry>,
) -> Result<SmcRunResult> {
    let account = LiteServerAccountId {
        workchain: address.workchain_id(),
        id: *address.to_internal(),
    };
    let method_id = method_id_from_name(method);
    let params = encode_input_stack(stack)?;

    let response = client
        .oneshot(LiteServerRunSmcMethod {
            mode: RUN_METHOD_MODE_RESULT,
            id: block_id,
            account,
            method_id,
            params,
        })
        .await
        .map_err(|e| anyhow!(e))?;

    // TODO[smc]: verify shard_proof/proof/state_proof bind to block_id when the mode
    // includes bits 0/1. Mirrors `verify_account_proofs` in adapter/account.rs.

    let stack = match response.result.as_deref() {
        Some(bytes) if !bytes.is_empty() => decode_result_stack(bytes)?,
        _ => Vec::new(),
    };

    Ok(SmcRunResult {
        gas_used: GAS_USED_PLACEHOLDER,
        exit_code: response.exit_code,
        stack,
    })
}

#[cfg(feature = "emulator")]
pub(super) async fn run_get_method_inner(
    client: LiteServerClient,
    address: SmartContractAddress,
    block_id: TonNodeBlockIdExt,
    method: &str,
    stack: Vec<StackEntry>,
) -> Result<SmcRunResult> {
    let mut account_client = AccountClient::new(client.clone());
    let mut config_client = ConfigClient::new(client.clone());
    let account_block_id = block_id.clone();
    let (account, config) = tokio::try_join!(
        async {
            account_client
                .get(address.clone(), account_block_id)
                .await
                .map_err(|error| anyhow!(error))
        },
        async {
            config_client
                .get_all(block_id)
                .await
                .map_err(|error| anyhow!(error))
        }
    )?;

    let method_id = i32::try_from(method_id_from_name(method))?;
    let stack_boc = base64_standard.encode(encode_input_stack(stack)?);
    let code_boc = encode_cell_b64(account.code())?;
    let data_boc = encode_cell_b64(account.data())?;
    let config_boc = encode_config_b64(&config)?;
    let prev_blocks_info_boc = encode_vm_value_b64(config.prev_blocks_info()?)?;
    let balance = u64::try_from(account.balance())
        .map_err(|_| anyhow!("account balance does not fit u64"))?;
    let extra_currencies = encode_extra_currencies(account.extra_currencies());
    let account_address = format!(
        "{}:{}",
        address.workchain_id(),
        hex::encode(address.to_internal())
    );

    let input = EmulatorInput {
        code_boc,
        data_boc,
        config_boc,
        prev_blocks_info_boc,
        stack_boc,
        account_address,
        extra_currencies,
        gen_utime: account.gen_utime(),
        balance,
        method_id,
    };
    let mut libraries = BTreeMap::new();
    let mut requested_libraries = BTreeSet::new();

    for _ in 0..16 {
        let libraries_boc = (!libraries.is_empty())
            .then(|| encode_libraries_b64(&libraries))
            .transpose()?;
        let input = input.clone();
        let json = tokio::task::spawn_blocking(move || run_emulator(input, libraries_boc))
            .await
            .map_err(|error| anyhow!("emulator worker failed: {error}"))??;

        match parse_emulator_result(&json)? {
            EmulatorOutcome::Complete(result) => return Ok(result),
            EmulatorOutcome::MissingLibrary(hash) => {
                if !requested_libraries.insert(hash) {
                    return Err(anyhow!(
                        "emulator repeatedly requested library {}",
                        hex::encode(hash)
                    ));
                }
                let response = client
                    .clone()
                    .oneshot(LiteServerGetLibraries {
                        library_list: vec![hash],
                    })
                    .await
                    .map_err(|error| anyhow!(error))?;
                let entry = response
                    .result
                    .into_iter()
                    .find(|entry| entry.hash == hash)
                    .ok_or_else(|| {
                        anyhow!("lite-server did not return library {}", hex::encode(hash))
                    })?;
                let boc = BoC::deserialize(&entry.data)?;
                let cell = boc
                    .single_root()
                    .ok_or_else(|| anyhow!("library BoC must have one root"))?;
                if cell.hash() != hash {
                    return Err(anyhow!(
                        "library content hash mismatch for {}",
                        hex::encode(hash)
                    ));
                }
                if cell.level_hash(0).0 > 512 {
                    return Err(anyhow!(
                        "library {} exceeds maximum depth",
                        hex::encode(hash)
                    ));
                }
                libraries.insert(hash, (**cell).clone());
            }
        }
    }

    Err(anyhow!("too many missing library retries"))
}

#[cfg(feature = "emulator")]
#[derive(Clone)]
struct EmulatorInput {
    code_boc: String,
    data_boc: String,
    config_boc: String,
    prev_blocks_info_boc: String,
    stack_boc: String,
    account_address: String,
    extra_currencies: String,
    gen_utime: u32,
    balance: u64,
    method_id: i32,
}

#[cfg(feature = "emulator")]
fn run_emulator(input: EmulatorInput, libraries_boc: Option<String>) -> Result<String> {
    let emulator = ton_emulator::TvmEmulator::new(&input.code_boc, &input.data_boc, 0)
        .map_err(|error| anyhow!(error))?;
    if let Some(libraries_boc) = libraries_boc
        && !emulator
            .set_libraries(&libraries_boc)
            .map_err(|error| anyhow!(error))?
    {
        return Err(anyhow!("failed to set emulator libraries"));
    }
    if !emulator
        .set_c7(
            &input.account_address,
            input.gen_utime,
            input.balance,
            &"0".repeat(64),
            &input.config_boc,
        )
        .map_err(|error| anyhow!(error))?
    {
        return Err(anyhow!("failed to set emulator c7"));
    }
    if !emulator
        .set_extra_currencies(&input.extra_currencies)
        .map_err(|error| anyhow!(error))?
    {
        return Err(anyhow!("failed to set emulator extra currencies"));
    }
    if !emulator
        .set_prev_blocks_info(&input.prev_blocks_info_boc)
        .map_err(|error| anyhow!(error))?
    {
        return Err(anyhow!("failed to set emulator previous blocks"));
    }

    let result = emulator
        .run_get_method(input.method_id, &input.stack_boc)
        .map_err(|error| anyhow!(error))?;
    Ok(result.as_str().to_owned())
}

#[cfg(feature = "emulator")]
#[derive(serde::Deserialize)]
struct EmulatorResult {
    success: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    stack: Option<String>,
    #[serde(default)]
    gas_used: Option<String>,
    #[serde(default)]
    vm_exit_code: Option<i32>,
    #[serde(default)]
    missing_library: Option<String>,
}

#[cfg(feature = "emulator")]
enum EmulatorOutcome {
    Complete(SmcRunResult),
    MissingLibrary([u8; 32]),
}

#[cfg(feature = "emulator")]
fn parse_emulator_result(json: &str) -> Result<EmulatorOutcome> {
    let response: EmulatorResult = serde_json::from_str(json)?;
    if !response.success {
        return Err(anyhow!(
            "emulator failed: {}",
            response.error.as_deref().unwrap_or("unknown error")
        ));
    }
    if let Some(hash) = response.missing_library {
        let hash = hex::decode(hash)?;
        let hash = hash.try_into().map_err(|hash: Vec<u8>| {
            anyhow!("library hash must be 32 bytes, got {}", hash.len())
        })?;
        return Ok(EmulatorOutcome::MissingLibrary(hash));
    }
    let stack_boc = response
        .stack
        .ok_or_else(|| anyhow!("emulator response has no stack"))?;
    let stack_bytes = base64_standard.decode(stack_boc)?;
    let gas_used = response
        .gas_used
        .ok_or_else(|| anyhow!("emulator response has no gas_used"))?
        .parse()?;

    Ok(EmulatorOutcome::Complete(SmcRunResult {
        gas_used,
        exit_code: response
            .vm_exit_code
            .ok_or_else(|| anyhow!("emulator response has no vm_exit_code"))?,
        stack: decode_result_stack(&stack_bytes)?,
    }))
}

#[cfg(feature = "emulator")]
fn encode_config_b64(config: &MasterchainConfig) -> Result<String> {
    let cell = config
        .params()
        .clone()
        .wrap_as::<Hashmap<u32, Ref, 32, Same>>()
        .to_cell(((), ()))?;
    encode_cell_b64(&cell)
}

#[cfg(feature = "emulator")]
fn encode_vm_value_b64(value: VmStackValue) -> Result<String> {
    let cell = value.to_cell(())?;
    encode_cell_b64(&cell)
}

#[cfg(feature = "emulator")]
fn encode_extra_currencies(currencies: &std::collections::HashMap<u32, BigUint>) -> String {
    let mut currencies = currencies.iter().collect::<Vec<_>>();
    currencies.sort_unstable_by_key(|(id, _)| **id);
    currencies
        .into_iter()
        .map(|(id, amount)| format!("{id}={amount}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(feature = "emulator")]
fn encode_libraries_b64(libraries: &BTreeMap<[u8; 32], toner::tlb::Cell>) -> Result<String> {
    let entries = libraries
        .iter()
        .map(|(key, cell)| (*key, cell.clone()))
        .collect::<Vec<_>>();
    let tree = build_library_tree(&entries, 0)?;
    let hashmap = Hashmap::<[u8; 32], _, 256>::new(tree);
    let cell = hashmap
        .wrap_as::<Hashmap<[u8; 32], Ref, 256, Same>>()
        .to_cell(((), ()))?;
    encode_cell_b64(&cell)
}

#[cfg(feature = "emulator")]
fn build_library_tree(
    entries: &[([u8; 32], toner::tlb::Cell)],
    offset: usize,
) -> Result<HashmapTree<toner::tlb::Cell>> {
    use toner::tlb::bits::bitvec::order::Msb0;
    use toner::tlb::bits::bitvec::view::AsBits;

    let first = entries
        .first()
        .ok_or_else(|| anyhow!("cannot build empty libraries dictionary"))?;
    let first_bits = first.0.as_bits::<Msb0>();
    let mut prefix_len = 0;
    while offset + prefix_len < 256
        && entries.iter().all(|(key, _)| {
            key.as_bits::<Msb0>()[offset + prefix_len] == first_bits[offset + prefix_len]
        })
    {
        prefix_len += 1;
    }
    let prefix = first_bits[offset..offset + prefix_len].to_bitvec();
    let next = offset + prefix_len;

    let node = if entries.len() == 1 {
        if next != 256 {
            return Err(anyhow!("incomplete library dictionary key"));
        }
        HashmapNode::Leaf(entries[0].1.clone())
    } else {
        if next >= 256 {
            return Err(anyhow!("duplicate library dictionary key"));
        }
        let split = entries.partition_point(|(key, _)| !key.as_bits::<Msb0>()[next]);
        if split == 0 || split == entries.len() {
            return Err(anyhow!("invalid library dictionary fork"));
        }
        HashmapNode::Fork([
            Box::new(build_library_tree(&entries[..split], next + 1)?),
            Box::new(build_library_tree(&entries[split..], next + 1)?),
        ])
    };

    Ok(HashmapTree::Edge {
        prefix,
        node: HashmapAugNode { extra: (), node },
    })
}

fn encode_input_stack(stack: Vec<StackEntry>) -> Result<Vec<u8>> {
    let mut items = Vec::with_capacity(stack.len());
    for entry in stack {
        items.push(stack_entry_to_vm(entry)?);
    }
    let vm_stack = VmStack(items);
    let cell = vm_stack
        .to_cell(())
        .map_err(|e| anyhow!("serialize VmStack to cell: {e}"))?;
    let bytes = BoC::from_root(cell)
        .serialize(BagOfCellsArgs::default())
        .map_err(|e| anyhow!("serialize VmStack BoC: {e}"))?;
    Ok(bytes)
}

fn stack_entry_to_vm(entry: StackEntry) -> Result<VmStackValue> {
    match entry {
        StackEntry::Slice { bytes } => {
            let cell = decode_single_root_cell(&bytes)?;
            let end_bits = u16::try_from(cell.data.len())
                .map_err(|_| anyhow!("slice cell.data.len() exceeds u16"))?;
            let end_ref = u8::try_from(cell.references.len())
                .map_err(|_| anyhow!("slice cell.references.len() exceeds u8"))?;
            Ok(VmStackValue::Slice {
                slice: VmCellSlice {
                    cell,
                    st_bits: 0,
                    end_bits,
                    st_ref: 0,
                    end_ref,
                },
            })
        }
        StackEntry::Cell { bytes } => {
            let cell = decode_single_root_cell(&bytes)?;
            Ok(VmStackValue::Cell { cell })
        }
        StackEntry::Number { number } => parse_number_to_vm(&number),
        // TVM has no distinct "list" stack value; the higher-level tonlibjson API exposes
        // List separately, but at the VM level it is just a Tuple. Map identically.
        StackEntry::Tuple { elements } | StackEntry::List { elements } => {
            let mut items = Vec::with_capacity(elements.len());
            for e in elements {
                items.push(stack_entry_to_vm(e)?);
            }
            Ok(VmStackValue::Tuple {
                tuple: VmStkTuple(items),
            })
        }
        StackEntry::Unsupported => Err(anyhow!(
            "cannot encode StackEntry::Unsupported into VmStack"
        )),
    }
}

fn parse_number_to_vm(number: &str) -> Result<VmStackValue> {
    if let Ok(v) = i64::from_str(number) {
        return Ok(VmStackValue::TinyInt { value: v });
    }
    if number.starts_with('-') {
        // VmStackValue::Int currently uses BigUint (see vm_stack.rs:54-58 TODO);
        // negative int257 cannot be represented until that is fixed in toner.
        return Err(anyhow!(
            "negative int257 not supported (toner BigInt limitation): {number}"
        ));
    }
    let big = BigUint::from_str(number)
        .map_err(|e| anyhow!("invalid Number stack entry {number:?}: {e}"))?;
    Ok(VmStackValue::Int { value: big })
}

fn decode_single_root_cell(b64: &str) -> Result<toner::tlb::Cell> {
    let boc = BoC::parse_base64(b64).map_err(|e| anyhow!("StackEntry bytes: invalid BoC: {e}"))?;
    let cell = boc
        .single_root()
        .ok_or_else(|| anyhow!("StackEntry bytes: BoC must have exactly one root cell"))?;
    Ok((**cell).clone())
}

fn decode_result_stack(bytes: &[u8]) -> Result<Vec<StackEntry>> {
    let boc = BoC::deserialize(bytes).map_err(|e| anyhow!("result stack: invalid BoC: {e}"))?;
    let root = boc
        .single_root()
        .ok_or_else(|| anyhow!("result stack: BoC must have exactly one root cell"))?
        .clone();
    let stack: VmStack = root
        .parse_fully(())
        .map_err(|e| anyhow!("result stack: parse VmStack: {e}"))?;
    stack.0.into_iter().map(vm_to_stack_entry).collect()
}

fn vm_to_stack_entry(value: VmStackValue) -> Result<StackEntry> {
    match value {
        VmStackValue::Null | VmStackValue::Nan => Ok(StackEntry::Unsupported),
        VmStackValue::TinyInt { value } => Ok(StackEntry::Number {
            number: value.to_string(),
        }),
        VmStackValue::Int { value } => Ok(StackEntry::Number {
            number: value.to_str_radix(10),
        }),
        VmStackValue::Cell { cell } => Ok(StackEntry::Cell {
            bytes: encode_cell_b64(&cell)?,
        }),
        VmStackValue::Slice { slice } => Ok(StackEntry::Slice {
            // NB: st_bits/end_bits/st_ref/end_ref are dropped — ton_tower::response::StackEntry::Slice
            // carries only the raw cell BoC.
            bytes: encode_cell_b64(&slice.cell)?,
        }),
        // Closest fit for Builder; ton_client has no Builder variant.
        VmStackValue::Builder { cell } => Ok(StackEntry::Cell {
            bytes: encode_cell_b64(&cell)?,
        }),
        VmStackValue::Cont { .. } => Ok(StackEntry::Unsupported),
        VmStackValue::Tuple { tuple } => {
            let elements = tuple
                .0
                .into_iter()
                .map(vm_to_stack_entry)
                .collect::<Result<Vec<_>>>()?;
            Ok(StackEntry::Tuple { elements })
        }
    }
}

fn encode_cell_b64(cell: &toner::tlb::Cell) -> Result<String> {
    let bytes = BoC::from_root(cell.clone())
        .serialize(BagOfCellsArgs::default())
        .map_err(|e| anyhow!("serialize cell BoC: {e}"))?;
    Ok(base64_standard.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::empty(vec![])]
    #[case::tinyint(vec![StackEntry::Number {
        number: "42".to_string(),
    }])]
    #[case::bigint(vec![StackEntry::Number {
        number: "123456789012345678901234567890".to_string(),
    }])]
    #[case::nested_tuple(vec![StackEntry::Tuple {
        elements: vec![
            StackEntry::Number {
                number: "1".to_string(),
            },
            StackEntry::Tuple {
                elements: vec![StackEntry::Number {
                    number: "2".to_string(),
                }],
            },
        ],
    }])]
    #[case::negative_tinyint(vec![StackEntry::Number {
            number: "-1".to_string(),
    }])]
    fn test_stack_roundtrip(#[case] expected: Vec<StackEntry>) {
        let actual = decode_result_stack(&encode_input_stack(expected.clone()).unwrap()).unwrap();

        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::negative_bigint(vec![StackEntry::Number {
            number: "-123456789012345678901234567890".to_string(),
    }], "negative")]
    #[case::unsupported(vec![StackEntry::Unsupported], "Unsupported")]
    fn test_encode_errors(#[case] input: Vec<StackEntry>, #[case] msg_contains: &str) {
        let err = encode_input_stack(input).unwrap_err();

        assert!(format!("{err}").contains(msg_contains));
    }

    #[cfg(feature = "emulator")]
    #[test]
    fn should_parse_emulator_result() {
        let stack = base64_standard.encode(encode_input_stack(Vec::new()).unwrap());
        let json = format!(
            r#"{{"success":true,"stack":"{stack}","gas_used":"42","vm_exit_code":0,"vm_log":"","missing_library":null}}"#
        );

        let EmulatorOutcome::Complete(result) = parse_emulator_result(&json).unwrap() else {
            panic!("expected complete result")
        };

        assert_eq!(result.gas_used, 42);
        assert_eq!(result.exit_code, 0);
        assert!(result.stack.is_empty());
    }

    #[cfg(feature = "emulator")]
    #[test]
    fn should_encode_multiple_libraries() {
        let first = toner::tlb::Cell::new();
        let second = toner::tlb::Cell::builder().into_cell();
        let libraries =
            BTreeMap::from([([0_u8; 32], first.clone()), ([0xff_u8; 32], second.clone())]);

        let encoded = encode_libraries_b64(&libraries).unwrap();
        let root = BoC::parse_base64(encoded)
            .unwrap()
            .single_root()
            .unwrap()
            .clone();
        let hashmap: Hashmap<[u8; 32], toner::tlb::Cell, 256> = root
            .parse_fully_as::<_, Hashmap<[u8; 32], Ref, 256, Same>>(((), ()))
            .unwrap();

        assert_eq!(
            hashmap.lookup(&[0_u8; 32]),
            crate::tlb::hashmap::HashmapLookup::Found(&first)
        );
        assert_eq!(
            hashmap.lookup(&[0xff_u8; 32]),
            crate::tlb::hashmap::HashmapLookup::Found(&second)
        );
    }
}

#[cfg(test)]
mod integration {
    use super::*;
    use crate::adapter::LiteServerAdapter;
    use crate::client::LiteServerClient;
    use rstest::rstest;
    use std::str::FromStr;
    use testcontainers_ton::{LocalLiteServer, SharedLiteServer};
    use ton_tower::request::RunGetMethod;
    use tower::ServiceExt;

    const FAUCET_WALLET_ADDR: &str =
        "-1:22f53b7d9aba2cef44755f7078b01614cd4dde2388a1729c2c386cf8f9898afe";

    #[rstest]
    #[case::seqno("seqno", SmcRunResult {
        gas_used: GAS_USED_PLACEHOLDER,
        exit_code: 0,
        stack: vec![StackEntry::Number {
            number: "0".to_string()
        }]
    })]
    #[case::unknown_method("definitely_not_a_method_xyz", SmcRunResult {
        gas_used: 0,
        exit_code: 32,
        stack: vec![StackEntry::Number {
            number: "0".to_string()
        }]
    })]
    // public key is "880db994b01ecd06fccc6099bf094997e94f5ada0f31f5604148f098ca037402"
    #[case::public_key("get_public_key", SmcRunResult {
        gas_used: 0,
        exit_code: 0,
        stack: vec![StackEntry::Number {
            number: "61538797250860244891658288584886086813375283594678556485491459892974908044290".to_string()
        }]
    })]
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_run_get_method(#[case] method: String, #[case] expected: SmcRunResult) {
        let (adapter, _server) = setup().await.unwrap();
        let address = SmartContractAddress::from_str(FAUCET_WALLET_ADDR).unwrap();

        let actual = adapter
            .oneshot(RunGetMethod {
                address,
                method,
                stack: vec![],
            })
            .await
            .unwrap();

        #[cfg(feature = "emulator")]
        {
            assert!(actual.gas_used > 0);
            assert_eq!(actual.exit_code, expected.exit_code);
            assert_eq!(actual.stack, expected.stack);
        }
        #[cfg(not(feature = "emulator"))]
        assert_eq!(actual, expected);
    }

    async fn setup() -> Result<(LiteServerAdapter, SharedLiteServer)> {
        let server = LocalLiteServer::shared().await?;
        let client = LiteServerClient::connect(server.addr(), server.server_key()).await?;
        Ok((LiteServerAdapter::new(client), server))
    }
}
