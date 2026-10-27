//! `serde` for [`HVal`], behind the `serde` feature.
//!
//! An `HVal` serializes as the plain data it holds: `null`, a bool, a number,
//! a string, a sequence or a string-keyed map. A [`HVal::Date`] serializes as
//! its `YYYY-MM-DD` text, so a format with no date type keeps it readable.
//! Deserializing reads the same shapes back, which can't tell a date from a
//! string: a string is always a [`HVal::Str`]. A whole number is a
//! [`HVal::Int`] when it fits an `i64`, else a [`HVal::Float`].

use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::HVal;

impl Serialize for HVal {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Null => serializer.serialize_unit(),
            Self::Bool(flag) => serializer.serialize_bool(*flag),
            Self::Int(num) => serializer.serialize_i64(*num),
            Self::Float(num) => serializer.serialize_f64(*num),
            Self::Str(text) | Self::Date(text) => serializer.serialize_str(text),
            Self::Array(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Self::Dict(map) => {
                let mut out = serializer.serialize_map(Some(map.len()))?;
                for (key, val) in map {
                    out.serialize_entry(key, val)?;
                }
                out.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for HVal {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(HValVisitor)
    }
}

struct HValVisitor;

impl<'de> Visitor<'de> for HValVisitor {
    type Value = HVal;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("null, a bool, a number, a string, a sequence or a map")
    }

    fn visit_unit<E: de::Error>(self) -> Result<HVal, E> {
        Ok(HVal::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<HVal, E> {
        Ok(HVal::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<HVal, D::Error> {
        HVal::deserialize(deserializer)
    }

    fn visit_bool<E: de::Error>(self, flag: bool) -> Result<HVal, E> {
        Ok(HVal::Bool(flag))
    }

    fn visit_i64<E: de::Error>(self, num: i64) -> Result<HVal, E> {
        Ok(HVal::Int(num))
    }

    fn visit_u64<E: de::Error>(self, num: u64) -> Result<HVal, E> {
        Ok(i64::try_from(num).map_or(HVal::Float(num as f64), HVal::Int))
    }

    fn visit_f64<E: de::Error>(self, num: f64) -> Result<HVal, E> {
        Ok(HVal::Float(num))
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<HVal, E> {
        Ok(HVal::Str(text.to_owned()))
    }

    fn visit_string<E: de::Error>(self, text: String) -> Result<HVal, E> {
        Ok(HVal::Str(text))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<HVal, A::Error> {
        let mut items = Vec::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(HVal::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<HVal, A::Error> {
        let mut out = BTreeMap::new();
        while let Some((key, val)) = map.next_entry::<String, HVal>()? {
            out.insert(key, val);
        }
        Ok(HVal::Dict(out))
    }
}

#[cfg(test)]
#[path = "value_serde_tests.rs"]
mod tests;
