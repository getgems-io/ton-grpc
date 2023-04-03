use std::str::FromStr;
use anyhow::anyhow;
use base64::Engine;
use crc::{Crc};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use bytes::BufMut;

#[derive(Debug, Clone, PartialEq)]
pub struct AccountAddressValue {
    flags: u8,
    pub chain_id: i32,
    pub bytes: [u8; 32],
}

impl FromStr for AccountAddressValue {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let chain_id: i32;
        let mut bytes: [u8; 32] = [0; 32];
        let mut flags: u8 = 17;

        if let Some((workchain_id, hex_bytes)) = s.split_once(':') {
            chain_id = workchain_id.parse()?;
            hex::decode_to_slice(hex_bytes, &mut bytes)?
        } else if hex::decode_to_slice(s, &mut bytes).is_ok() {
            chain_id = -1;
        } else {
            // convert url safe to standard
            let s = s
                .replace('-', "+")
                .replace('_', "/");

            let Ok(data) = base64::engine::general_purpose::STANDARD.decode(s) else {
                return Err(anyhow!("unexpected address format"))
            };

            let [_flags, workchain_id, data @ ..] = &data[..] else {
                return Err(anyhow!("invalid base64 address: {}", String::from_utf8(data).unwrap()))
            };

            // 32 is length of address and 2 is length of crc16
            if data.len() != 32 + 2 {
                return Err(anyhow!(
                    "invalid address length, expected 34 got {} bytes", data.len()));
            }

            flags = *_flags;

            chain_id = if *workchain_id == u8::MAX {
                 -1
            } else {
                *workchain_id as i32
            };

            bytes.copy_from_slice(&data[0..32]);
        };

        Ok(Self {
            flags,
            chain_id,
            bytes
        })
    }
}

const CRC16: Crc<u16> = Crc::<u16>::new(&crc::CRC_16_XMODEM);

const BOUNCABLE: u8 = 0x11;
const NON_BOUNCABLE: u8 = 0x51;

impl AccountAddressValue {
    pub fn as_string(&self) -> String {
        format!("{}:{}", self.chain_id, hex::encode(&self.bytes))
    }

    pub fn as_base64_string(&self) -> String {
        let mut buf = vec![];
        buf.put_u8(self.flags);
        buf.put_u8(if self.chain_id == -1 { u8::MAX } else { self.chain_id as u8 });
        buf.put_slice(&self.bytes);

        let crc16 = CRC16.checksum(&buf);

        buf.put_u16(crc16);

        base64::engine::general_purpose::URL_SAFE.encode(buf)
    }

    pub fn bounceable(&self) -> Self {
        Self {
            flags: BOUNCABLE,
            chain_id: self.chain_id,
            bytes: self.bytes
        }
    }

    pub fn non_bounceable(&self) -> Self {
        Self {
            flags: NON_BOUNCABLE,
            chain_id: self.chain_id,
            bytes: self.bytes
        }
    }
}

impl Serialize for AccountAddressValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        serializer.serialize_str(&self.as_string())
    }
}

impl<'de> Deserialize<'de> for AccountAddressValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        let s = String::deserialize(deserializer)?;

        FromStr::from_str(&s)
            .map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use crate::block::account_address::AccountAddressValue;

    #[test]
    fn account_address_correct() {
        assert!(AccountAddressValue::from_str("EQBO_mAVkaHxt6Ibz7wqIJ_UIDmxZBFcgkk7fvIzkh7l42wO").is_ok())
    }

    #[test]
    fn account_address_serialize() {
        let address = AccountAddressValue::from_str("EQCjk1hh952vWaE9bRguFkAhDAL5jj3xj9p0uPWrFBq_GEMS").unwrap();

        assert_eq!(serde_json::to_string(&address).unwrap(), "\"0:a3935861f79daf59a13d6d182e1640210c02f98e3df18fda74b8f5ab141abf18\"")
    }

    #[test]
    fn account_address_deserialize() {
        let json = "\"0:a3935861f79daf59a13d6d182e1640210c02f98e3df18fda74b8f5ab141abf18\"";
        let address = serde_json::from_str::<AccountAddressValue>(json).unwrap();
        assert_eq!(AccountAddressValue::from_str("EQCjk1hh952vWaE9bRguFkAhDAL5jj3xj9p0uPWrFBq_GEMS").unwrap(), address);
    }

    #[test]
    fn account_address_base64_fail() {
        assert!(AccountAddressValue::from_str("YXNkcXdl").is_err());
    }

    #[test]
    fn account_address_base64() {
        assert_eq!(AccountAddressValue::from_str("EQCjk1hh952vWaE9bRguFkAhDAL5jj3xj9p0uPWrFBq_GEMS").unwrap().as_base64_string(), "EQCjk1hh952vWaE9bRguFkAhDAL5jj3xj9p0uPWrFBq_GEMS");
        assert_eq!(AccountAddressValue::from_str("EQB5HQfjevz9su4ZQGcDT_4IB0IUGh5PM2vAXPU2e4O6_d2j").unwrap().as_base64_string(), "EQB5HQfjevz9su4ZQGcDT_4IB0IUGh5PM2vAXPU2e4O6_d2j")
    }

    #[test]
    fn account_address_base64_bounceable() {
        assert_eq!(AccountAddressValue::from_str("EQCjk1hh952vWaE9bRguFkAhDAL5jj3xj9p0uPWrFBq_GEMS").unwrap().non_bounceable().as_base64_string(), "UQCjk1hh952vWaE9bRguFkAhDAL5jj3xj9p0uPWrFBq_GB7X");
        assert_eq!(AccountAddressValue::from_str("EQB5HQfjevz9su4ZQGcDT_4IB0IUGh5PM2vAXPU2e4O6_d2j").unwrap().non_bounceable().as_base64_string(), "UQB5HQfjevz9su4ZQGcDT_4IB0IUGh5PM2vAXPU2e4O6_YBm")
    }
}
