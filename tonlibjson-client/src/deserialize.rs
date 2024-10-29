use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Display;
use std::str::FromStr;

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
where
    D: Deserializer<'de>,
    T: Default + serde::Deserialize<'de> + PartialEq,
{
    let v = T::deserialize(deserializer)?;

    Ok(if v == T::default() { None } else { Some(v) })
}

pub fn deserialize_empty_as_none<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr + serde::Deserialize<'de>,
    <T as FromStr>::Err: Display,
{
    let v = String::deserialize(deserializer)?;

    if v.is_empty() {
        Ok(None)
    } else {
        Ok(Some(T::from_str(&v).map_err(de::Error::custom)?))
    }
}

pub fn deserialize_ton_account_balance<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let v: i64 = deserialize_number_from_string(deserializer)?;

    Ok(if v == -1 { None } else { Some(v) })
}

pub fn serialize_none_as_empty<S, T>(v: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Serialize,
{
    match v {
        None => serializer.serialize_str(""),
        Some(v) => v.serialize(serializer),
    }
}
