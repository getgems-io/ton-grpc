use anyhow::anyhow;
use base64::Engine;
use base64::engine::general_purpose::{STANDARD as base64_standard, URL_SAFE as base64_url_safe};
use bytes::BufMut;
use crc::Crc;
use std::fmt::Display;
use std::str::FromStr;

const CRC16: Crc<u16> = Crc::<u16>::new(&crc::CRC_16_XMODEM);

pub type WorkchainId = i32;
pub type SmartContractInternalAddress = [u8; 32];

#[derive(Debug, Clone, PartialEq)]
pub enum SmartContractAddress {
    Raw {
        workchain_id: WorkchainId,
        data: SmartContractInternalAddress,
    },
    UserFriendly {
        flags: u8,
        workchain_id: WorkchainId,
        data: SmartContractInternalAddress,
    },
}

impl FromStr for SmartContractAddress {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() == 48 {
            // convert url safe to standard
            let s = s.replace('-', "+").replace('_', "/");
            let Ok(data) = base64_standard.decode(&s) else {
                return Err(anyhow!("invalid base64 address: {}", &s));
            };

            let crc16 = CRC16.checksum(&data[..34]);
            let [flags, workchain_id, data @ .., crc16_l, crc16_r] = &data[..] else {
                return Err(anyhow!("invalid base64 address: {}", &s));
            };

            if u16::from_be_bytes([*crc16_l, *crc16_r]) != crc16 {
                return Err(anyhow!("invalid base64 address crc16: {}", &s));
            }

            let mut bytes: [u8; 32] = [0; 32];
            bytes.copy_from_slice(&data[0..32]);

            return Ok(Self::UserFriendly {
                flags: *flags,
                workchain_id: if *workchain_id == u8::MAX {
                    -1
                } else {
                    *workchain_id as i32
                },
                data: bytes,
            });
        } else if let Some((workchain_id, hex_bytes)) = s.split_once(':')
            && hex_bytes.len() == 64
        {
            let workchain_id = workchain_id.parse()?;
            let mut bytes: [u8; 32] = [0; 32];

            hex::decode_to_slice(hex_bytes, &mut bytes)?;

            return Ok(Self::Raw {
                workchain_id,
                data: bytes,
            });
        }

        Err(anyhow!("invalid address: {}", s))
    }
}

impl Display for SmartContractAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            SmartContractAddress::Raw { workchain_id, data } => {
                format!("{}:{}", workchain_id, hex::encode(data))
            }
            SmartContractAddress::UserFriendly {
                flags,
                workchain_id,
                data,
            } => {
                let mut buf = vec![];
                buf.put_u8(*flags);
                buf.put_u8(if *workchain_id == -1 {
                    u8::MAX
                } else {
                    *workchain_id as u8
                });
                buf.put_slice(data.as_ref());
                buf.put_u16(CRC16.checksum(&buf));

                base64_url_safe.encode(buf)
            }
        };
        write!(f, "{}", str)
    }
}

impl SmartContractAddress {
    const BOUNCEABLE: u8 = 0x11;
    const NON_BOUNCEABLE: u8 = 0x51;

    pub fn bounceable(&self) -> Self {
        match self {
            SmartContractAddress::Raw { workchain_id, data }
            | SmartContractAddress::UserFriendly {
                flags: _,
                workchain_id,
                data,
            } => Self::UserFriendly {
                flags: Self::BOUNCEABLE,
                workchain_id: *workchain_id,
                data: *data,
            },
        }
    }

    pub fn non_bounceable(&self) -> Self {
        match self {
            SmartContractAddress::Raw { workchain_id, data }
            | SmartContractAddress::UserFriendly {
                flags: _,
                workchain_id,
                data,
            } => Self::UserFriendly {
                flags: Self::NON_BOUNCEABLE,
                workchain_id: *workchain_id,
                data: *data,
            },
        }
    }

    pub fn raw(&self) -> Self {
        match self {
            SmartContractAddress::Raw { workchain_id, data }
            | SmartContractAddress::UserFriendly {
                flags: _,
                workchain_id,
                data,
            } => Self::Raw {
                workchain_id: *workchain_id,
                data: *data,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{SmartContractAddress, SmartContractInternalAddress};
    use std::str::FromStr;

    #[test]
    fn user_friendly_smart_contract_address_from_string() {
        let actual =
            SmartContractAddress::from_str("Ef_lZ1T4NCb2mwkme9h2rJfESCE0W34ma9lWp7-_uY3zXDvq")
                .unwrap();

        assert_eq!(actual, ADDRESS_USER_FRIENDLY)
    }

    #[test]
    fn user_friendly_smart_contract_address_from_base64_standard_string() {
        let actual =
            SmartContractAddress::from_str("Ef/lZ1T4NCb2mwkme9h2rJfESCE0W34ma9lWp7+/uY3zXDvq")
                .unwrap();

        assert_eq!(actual, ADDRESS_USER_FRIENDLY)
    }

    #[test]
    fn user_friendly_smart_contract_address_to_string() {
        let actual = ADDRESS_USER_FRIENDLY.to_string();

        assert_eq!(actual, "Ef_lZ1T4NCb2mwkme9h2rJfESCE0W34ma9lWp7-_uY3zXDvq");
    }

    #[test]
    fn raw_smart_contract_address_from_string() {
        let actual = SmartContractAddress::from_str(
            "-1:e56754f83426f69b09267bd876ac97c44821345b7e266bd956a7bfbfb98df35c",
        )
        .unwrap();

        assert_eq!(actual, RAW_ADDRESS)
    }

    #[test]
    fn raw_smart_contract_address_to_string() {
        let actual = RAW_ADDRESS.to_string();

        assert_eq!(
            actual,
            "-1:e56754f83426f69b09267bd876ac97c44821345b7e266bd956a7bfbfb98df35c"
        );
    }

    #[test]
    fn smart_contract_address_from_str_invalid_address() {
        let err = SmartContractAddress::from_str("YXNkcXdl").unwrap_err();

        assert_eq!(err.to_string(), "invalid address: YXNkcXdl");
    }

    #[test]
    fn smart_contract_address_to_bounceable() {
        let internal_address = INTERNAL_ADDRESS;
        let address = SmartContractAddress::Raw {
            workchain_id: -1,
            data: internal_address,
        };

        let actual = address.bounceable();

        assert_eq!(
            actual,
            SmartContractAddress::UserFriendly {
                flags: SmartContractAddress::BOUNCEABLE,
                workchain_id: -1,
                data: internal_address,
            }
        )
    }

    #[test]
    fn smart_contract_address_to_non_bounceable() {
        let internal_address = INTERNAL_ADDRESS;
        let address = SmartContractAddress::Raw {
            workchain_id: -1,
            data: internal_address,
        };

        let actual = address.non_bounceable();

        assert_eq!(
            actual,
            SmartContractAddress::UserFriendly {
                flags: SmartContractAddress::NON_BOUNCEABLE,
                workchain_id: -1,
                data: internal_address,
            }
        )
    }

    #[test]
    fn smart_contract_address_to_raw() {
        let internal_address = INTERNAL_ADDRESS;
        let address = SmartContractAddress::Raw {
            workchain_id: -1,
            data: internal_address,
        };

        let actual = address.raw();

        assert_eq!(
            actual,
            SmartContractAddress::Raw {
                workchain_id: -1,
                data: internal_address,
            }
        )
    }

    ///
    /// Example from https://github.com/ton-blockchain/TEPs/blob/master/text/0002-address.md
    ///
    #[test]
    fn smart_contract_address_example_from_tep() {
        let address = SmartContractAddress::from_str(
            "-1:e56754f83426f69b09267bd876ac97c44821345b7e266bd956a7bfbfb98df35c",
        )
        .unwrap();

        let bounceable = address.bounceable().to_string();
        let non_bounceable = address.non_bounceable().to_string();

        assert_eq!(
            bounceable,
            "Ef_lZ1T4NCb2mwkme9h2rJfESCE0W34ma9lWp7-_uY3zXDvq"
        );
        assert_eq!(
            non_bounceable,
            "Uf_lZ1T4NCb2mwkme9h2rJfESCE0W34ma9lWp7-_uY3zXGYv"
        );
    }

    const INTERNAL_ADDRESS: SmartContractInternalAddress = [
        229, 103, 84, 248, 52, 38, 246, 155, 9, 38, 123, 216, 118, 172, 151, 196, 72, 33, 52, 91,
        126, 38, 107, 217, 86, 167, 191, 191, 185, 141, 243, 92,
    ];

    const ADDRESS_USER_FRIENDLY: SmartContractAddress = SmartContractAddress::UserFriendly {
        flags: 0x11,
        workchain_id: -1,
        data: INTERNAL_ADDRESS,
    };

    const RAW_ADDRESS: SmartContractAddress = SmartContractAddress::Raw {
        workchain_id: -1,
        data: INTERNAL_ADDRESS,
    };
}
