use pyo3::PyResult;
use pyo3::exceptions::PyValueError;
use serde::de::{self, DeserializeOwned, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

use std::fmt;

pub(crate) fn parse_json_arg<T: DeserializeOwned>(json: &str) -> PyResult<T> {
    let checked = serde_json::from_str::<DuplicateCheckedJson>(json)
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
    serde_json::from_value(checked.0).map_err(|error| PyValueError::new_err(error.to_string()))
}

struct DuplicateCheckedJson(serde_json::Value);

impl<'de> Deserialize<'de> for DuplicateCheckedJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(DuplicateCheckedJsonVisitor)
    }
}

struct DuplicateCheckedJsonVisitor;

impl<'de> Visitor<'de> for DuplicateCheckedJsonVisitor {
    type Value = DuplicateCheckedJson;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedJson(serde_json::Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedJson(serde_json::Value::Number(
            serde_json::Number::from(value),
        )))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedJson(serde_json::Value::Number(
            serde_json::Number::from(value),
        )))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let number =
            serde_json::Number::from_f64(value).ok_or_else(|| E::custom("invalid JSON number"))?;
        Ok(DuplicateCheckedJson(serde_json::Value::Number(number)))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedJson(serde_json::Value::String(
            value.to_string(),
        )))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedJson(serde_json::Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedJson(serde_json::Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedJson(serde_json::Value::Null))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        DuplicateCheckedJson::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut access: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::with_capacity(access.size_hint().unwrap_or(0));
        while let Some(value) = access.next_element::<DuplicateCheckedJson>()? {
            values.push(value.0);
        }
        Ok(DuplicateCheckedJson(serde_json::Value::Array(values)))
    }

    fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = serde_json::Map::new();
        while let Some(key) = access.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom(format!(
                    "duplicate JSON object key `{key}`"
                )));
            }
            let value = access.next_value::<DuplicateCheckedJson>()?;
            values.insert(key, value.0);
        }
        Ok(DuplicateCheckedJson(serde_json::Value::Object(values)))
    }
}
