use std::fmt::Display;
use std::str::FromStr;
use serde::{Deserialize, Deserializer};

pub fn deserialize_number_from_string<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: FromStr + serde::Deserialize<'de>,
        <T as FromStr>::Err: Display,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrInt<T> {
        String(String),
        Number(T),
    }

    match StringOrInt::<T>::deserialize(deserializer)? {
        StringOrInt::String(s) => s.parse::<T>().map_err(serde::de::Error::custom),
        StringOrInt::Number(i) => Ok(i),
    }
}

pub fn deserialize_default_as_none<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
    where D: Deserializer<'de>,
          T : Default + serde::Deserialize<'de> + PartialEq
{
    let v = T::deserialize(deserializer)?;

    Ok(if v == T::default() {
        None
    } else {
        Some(v)
    })
}


pub fn deserialize_ton_account_balance<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
    where D: Deserializer<'de>
{
    let v: i64  = deserialize_number_from_string(deserializer)?;

    Ok(if v == -1 {
        None
    } else {
        Some(v)
    })
}
