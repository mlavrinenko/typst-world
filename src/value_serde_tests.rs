#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;

use serde_json::json;

use crate::HVal;

fn dict(pairs: Vec<(&str, HVal)>) -> HVal {
    HVal::Dict(
        pairs
            .into_iter()
            .map(|(key, val)| (key.to_owned(), val))
            .collect::<BTreeMap<_, _>>(),
    )
}

#[test]
fn serializes_every_variant_and_a_date_as_its_text() {
    let value = dict(vec![
        ("none", HVal::Null),
        ("yes", HVal::Bool(true)),
        ("int", HVal::Int(-3)),
        ("float", HVal::Float(1.5)),
        ("text", HVal::Str("a".to_owned())),
        ("day", HVal::Date("2026-10-28".to_owned())),
        ("list", HVal::Array(vec![HVal::Int(1)])),
    ]);
    let expected = json!({
        "none": null, "yes": true, "int": -3, "float": 1.5, "text": "a",
        "day": "2026-10-28", "list": [1],
    });
    assert_eq!(serde_json::to_value(&value).unwrap(), expected);
}

#[test]
fn deserializes_every_json_shape() {
    let json = json!({
        "none": null, "yes": false, "int": -3, "whole": 2.0, "huge": u64::MAX,
        "text": "2026-10-28", "list": [1, "b"], "empty": {},
    });
    let expected = dict(vec![
        ("none", HVal::Null),
        ("yes", HVal::Bool(false)),
        ("int", HVal::Int(-3)),
        ("whole", HVal::Float(2.0)),
        ("huge", HVal::Float(u64::MAX as f64)),
        ("text", HVal::Str("2026-10-28".to_owned())),
        (
            "list",
            HVal::Array(vec![HVal::Int(1), HVal::Str("b".to_owned())]),
        ),
        ("empty", dict(vec![])),
    ]);
    assert_eq!(serde_json::from_value::<HVal>(json).unwrap(), expected);
}

#[test]
fn a_json_round_trip_keeps_every_variant_but_date() {
    let value = dict(vec![
        ("float", HVal::Float(-0.25)),
        ("int", HVal::Int(i64::MIN)),
        ("nested", HVal::Array(vec![dict(vec![("in", HVal::Null)])])),
    ]);
    let text = serde_json::to_string(&value).unwrap();
    assert_eq!(serde_json::from_str::<HVal>(&text).unwrap(), value);
}
