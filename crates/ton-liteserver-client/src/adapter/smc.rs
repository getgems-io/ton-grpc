use crate::adapter::account::get_emulator_state_inner;
use crate::client::LiteServerClient;
use crate::tl::{Int31, LiteServerAccountId, LiteServerRunSmcMethod, TonNodeBlockIdExt};
use crate::tlb::vm_stack::{VmCellSlice, VmStack, VmStackValue, VmStkTuple};
use anyhow::{Result, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as base64_standard;
use crc::Crc;
use num_bigint::BigUint;
use std::str::FromStr;
use ton_address::SmartContractAddress;
use ton_tower::response::{SmcRunResult, StackEntry};
use toner::tlb::bits::NBits;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::bits::ser::BitWriterExt;
use toner::tlb::ser::CellSerializeExt;
use toner::tlb::{BagOfCellsArgs, BoC, Ref};
use tower::ServiceExt;

// Request the result and the complete c7 tuple used by the lite server. The
// latter lets libemulator repeat the execution with identical VM context.
const RUN_METHOD_MODE_RESULT_WITH_FULL_CONTEXT: Int31 = 0x4 | 0x8 | 0x10 | 0x20;
const GET_METHOD_GAS_LIMIT: i64 = 10_000_000;

const CRC16: Crc<u16> = Crc::<u16>::new(&crc::CRC_16_XMODEM);

fn method_id_from_name(name: &str) -> i64 {
    (CRC16.checksum(name.as_bytes()) as i64) | 0x10000
}

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

    let run_method = client.clone().oneshot(LiteServerRunSmcMethod {
        mode: RUN_METHOD_MODE_RESULT_WITH_FULL_CONTEXT,
        id: block_id.clone(),
        account,
        method_id,
        params: params.clone(),
    });
    let account_state = get_emulator_state_inner(&client, &address, block_id);
    let (response, (code, data)) = tokio::try_join!(
        async { run_method.await.map_err(|e| anyhow!(e)) },
        account_state,
    )?;

    // TODO[smc]: verify shard_proof/proof/state_proof bind to block_id when the mode
    // includes bits 0/1. Mirrors `verify_account_proofs` in adapter/account.rs.

    let (emulator_exit_code, gas_used) = emulate_get_method(
        &code,
        &data,
        &params,
        response
            .init_c_7
            .as_deref()
            .ok_or_else(|| anyhow!("lite server omitted requested init_c7"))?,
        response.lib_extras.as_deref().unwrap_or_default(),
        method_id,
    )?;
    if emulator_exit_code != response.exit_code {
        return Err(anyhow!(
            "emulator exit code {emulator_exit_code} differs from lite server exit code {}",
            response.exit_code
        ));
    }
    let stack = match response.result.as_deref() {
        Some(bytes) if !bytes.is_empty() => decode_result_stack(bytes)?,
        _ => Vec::new(),
    };

    Ok(SmcRunResult {
        gas_used,
        exit_code: response.exit_code,
        stack,
    })
}

fn emulate_get_method(
    code: &toner::tlb::Cell,
    data: &toner::tlb::Cell,
    stack_boc: &[u8],
    c7_boc: &[u8],
    libraries_boc: &[u8],
    method_id: i64,
) -> Result<(i32, i64)> {
    let stack = decode_boc_root(stack_boc, "input stack")?;
    let c7_value: VmStackValue = decode_boc_root(c7_boc, "init_c7")?
        .parse_fully(())
        .map_err(|e| anyhow!("init_c7: parse VmStackValue: {e}"))?;
    let c7 = VmStack(vec![c7_value])
        .to_cell(())
        .map_err(|e| anyhow!("init_c7: serialize VmStack: {e}"))?;
    let libraries = if libraries_boc.is_empty() {
        toner::tlb::Cell::new()
    } else {
        decode_boc_root(libraries_boc, "libraries")?
    };
    let method_id = i32::try_from(method_id).map_err(|_| anyhow!("method id exceeds i32"))?;

    let mut context = toner::tlb::Cell::builder();
    context
        .store_as::<_, Ref>(c7, ())?
        .store_as::<_, Ref>(libraries, ())?;
    let mut request = toner::tlb::Cell::builder();
    request
        .store_as::<_, Ref>(code, ())?
        .store_as::<_, Ref>(data, ())?
        .store_as::<_, Ref>(stack, ())?
        .store_as::<_, Ref>(context.into_cell(), ())?
        .pack_as::<_, NBits<32>>(method_id, ())?;
    let request = BoC::from_root(request.into_cell()).serialize(BagOfCellsArgs::default())?;
    let response = ton_emulator::TvmEmulator::run_get_method_once(&request, GET_METHOD_GAS_LIMIT)?;
    let root = decode_boc_root(response.as_ref(), "emulator result")?;
    let mut parser = root.parser();
    let exit_code: i32 = parser.unpack_as::<_, NBits<32>>(())?;
    let gas_used: i64 = parser.unpack_as::<_, NBits<64>>(())?;

    Ok((exit_code, gas_used))
}

fn decode_boc_root(bytes: &[u8], name: &str) -> Result<toner::tlb::Cell> {
    BoC::deserialize(bytes)?
        .single_root()
        .map(|cell| (**cell).clone())
        .ok_or_else(|| anyhow!("{name}: BoC must have exactly one root"))
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

    #[test]
    fn full_context_mode_requests_result_c7_and_libraries() {
        assert_eq!(RUN_METHOD_MODE_RESULT_WITH_FULL_CONTEXT, 0x3c);
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
        gas_used: 549,
        exit_code: 0,
        stack: vec![StackEntry::Number {
            number: "0".to_string()
        }]
    })]
    #[case::unknown_method("definitely_not_a_method_xyz", SmcRunResult {
        gas_used: 328,
        exit_code: 32,
        stack: vec![StackEntry::Number {
            number: "0".to_string()
        }]
    })]
    // public key is "880db994b01ecd06fccc6099bf094997e94f5ada0f31f5604148f098ca037402"
    #[case::public_key("get_public_key", SmcRunResult {
        gas_used: 549,
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

        assert_eq!(actual, expected);
    }

    async fn setup() -> Result<(LiteServerAdapter, SharedLiteServer)> {
        let server = LocalLiteServer::shared().await?;
        let client = LiteServerClient::connect(server.addr(), server.server_key()).await?;
        Ok((LiteServerAdapter::new(client), server))
    }
}
