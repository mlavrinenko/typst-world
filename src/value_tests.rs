#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;

use typst::foundations::{Datetime, NativeElement, Value};
use typst_library::model::{EnumItem, LinkElem, ListItem, ParElem, ParbreakElem, Url};
use typst_library::text::TextElem;

use super::{HVal, convert, format_date};

#[test]
fn projects_standard_link_to_its_destination() {
    // `link("stem.typ")` is the standard Typst hyperlink element; harvest reads
    // its URL destination, not the rendered element's repr.
    let url = Url::new("converge-speconaut.typ").unwrap();
    let link = Value::Content(LinkElem::from_url(url).pack());
    assert_eq!(
        convert(&link),
        HVal::Str("converge-speconaut.typ".to_owned())
    );
}

#[test]
fn projects_content_to_its_plain_text() {
    // A trailing `[note]` block reaches harvest as content; project it to the
    // concatenated text, not the element's repr.
    let note = Value::Content(TextElem::new("shim gone".into()).pack());
    assert_eq!(convert(&note), HVal::Str("shim gone".to_owned()));
}

#[test]
fn projects_multi_paragraph_content_with_separators() {
    // A status note's trailing `[…]` block is authored with blank-line
    // paragraph breaks and a bullet list; `Content::plain_text()` alone
    // concatenates every leaf text run with nothing between them, welding
    // "outcome." straight onto "Second" and dropping the list structure
    // entirely. A paragraph break and a list item must each cost a
    // separator so the round trip stays readable.
    let text = |s: &str| TextElem::new(s.into()).pack();
    let note = ParElem::new(text("First line of the outcome.")).pack()
        + ParElem::new(text("Second paragraph explaining the decision.")).pack()
        + ListItem::new(text("a bullet")).pack()
        + ListItem::new(text("another")).pack();
    assert_eq!(
        convert(&Value::Content(note)),
        HVal::Str(
            "First line of the outcome. Second paragraph explaining the decision. \
             a bullet another"
                .to_owned()
        )
    );
}

#[test]
fn projects_a_blank_line_break_with_a_separator() {
    // A blank line in markup that has never been laid out compiles to a
    // `ParbreakElem`, not a `ParElem` — this crate is eval-only, and
    // paragraphs are only grouped into `ParElem` during realization, a
    // layout-adjacent step eval never runs. A harvested note's blank-line
    // break is always this shape, never the hand-built one above.
    let text = |s: &str| TextElem::new(s.into()).pack();
    let note = text("First line.") + ParbreakElem::shared().clone() + text("Second paragraph.");
    assert_eq!(
        convert(&Value::Content(note)),
        HVal::Str("First line. Second paragraph.".to_owned())
    );
}

#[test]
fn projects_enum_items_with_separators() {
    let text = |s: &str| TextElem::new(s.into()).pack();
    let note = EnumItem::new(text("first")).pack() + EnumItem::new(text("second")).pack();
    assert_eq!(
        convert(&Value::Content(note)),
        HVal::Str("first second".to_owned())
    );
}

#[test]
fn converts_scalars() {
    assert_eq!(convert(&Value::None), HVal::Null);
    assert_eq!(convert(&Value::Bool(true)), HVal::Bool(true));
    assert_eq!(convert(&Value::Int(7)), HVal::Int(7));
    assert_eq!(convert(&Value::Float(1.5)), HVal::Float(1.5));
    assert_eq!(
        convert(&Value::Str("hi".into())),
        HVal::Str("hi".to_owned())
    );
}

#[test]
fn converts_date() {
    let date = Datetime::from_ymd(2026, 6, 14).unwrap();
    assert_eq!(format_date(&date), "2026-06-14");
    assert_eq!(
        convert(&Value::Datetime(date)),
        HVal::Date("2026-06-14".to_owned())
    );
}

#[test]
fn converts_nested() {
    let array = Value::Array(typst::foundations::Array::from_iter([
        Value::Int(1),
        Value::Str("a".into()),
    ]));
    let HVal::Array(items) = convert(&array) else {
        panic!("expected array");
    };
    assert_eq!(items.len(), 2);
    assert_eq!(items.first(), Some(&HVal::Int(1)));
}

#[test]
fn numeric_accessors_accept_int_and_float() {
    assert_eq!(HVal::Int(3).as_i64(), Some(3));
    assert_eq!(HVal::Int(3).as_f64(), Some(3.0));
    assert_eq!(HVal::Float(2.5).as_f64(), Some(2.5));
    assert_eq!(HVal::Float(2.5).as_i64(), None);
}

#[test]
fn dict_lookup() {
    let mut map = BTreeMap::new();
    map.insert("marker".to_owned(), HVal::Str("mindtape.task".to_owned()));
    let dict = HVal::Dict(map);
    assert_eq!(
        dict.get("marker").and_then(HVal::as_str),
        Some("mindtape.task")
    );
    assert_eq!(dict.get("missing"), None);
}

#[test]
fn null_and_string_helpers() {
    assert!(HVal::Null.is_null());
    assert!(!HVal::Bool(false).is_null());
    assert_eq!(HVal::Str("x".to_owned()).as_str(), Some("x"));
    assert_eq!(
        HVal::Date("2026-01-01".to_owned()).as_str(),
        Some("2026-01-01")
    );
}

/// Evaluate `#let out = <syntax>` in a world holding nothing else, and convert
/// the result back.
fn eval_syntax(syntax: &str) -> HVal {
    use comemo::Track as _;
    use typst::World as _;
    use typst::engine::{Route, Sink, Traced};
    let main = crate::root_file_id("main.typ").unwrap();
    let source = format!("#let out = {syntax}");
    let world = crate::World::with_provider(
        main,
        std::path::PathBuf::new(),
        Box::new(crate::BytesProvider::new().with_file(main, source.into_bytes())),
    );
    let world_dyn: &dyn typst::World = &world;
    let mut sink = Sink::new();
    let module = typst_eval::eval(
        world_dyn.track(),
        world.library(),
        Traced::default().track(),
        sink.track_mut(),
        Route::default().track(),
        &world.source(main).unwrap(),
    )
    .unwrap_or_else(|diags| panic!("{syntax}: {}", world.eval_error(&diags)));
    convert(module.scope().get("out").unwrap().read())
}

fn every_variant() -> HVal {
    let map = |pairs: Vec<(&str, HVal)>| {
        HVal::Dict(
            pairs
                .into_iter()
                .map(|(key, val)| (key.to_owned(), val))
                .collect(),
        )
    };
    map(vec![
        ("none", HVal::Null),
        ("yes", HVal::Bool(true)),
        ("neg", HVal::Int(-3)),
        ("whole", HVal::Float(2.0)),
        ("frac", HVal::Float(-1.5)),
        ("inf", HVal::Float(f64::NEG_INFINITY)),
        ("text", HVal::Str("a \"q\" \\ b\nc\t#d $e$".to_owned())),
        ("day", HVal::Date("2026-10-28".to_owned())),
        ("empty", HVal::Array(vec![])),
        ("one", HVal::Array(vec![HVal::Int(1)])),
        ("odd key: \"x\"", map(vec![])),
        (
            "nested",
            map(vec![(
                "in",
                HVal::Array(vec![HVal::Null, HVal::Str(String::new())]),
            )]),
        ),
    ])
}

#[test]
fn to_typst_evaluates_back_to_the_same_value() {
    let value = every_variant();
    assert_eq!(eval_syntax(&value.to_typst()), value);
}

#[test]
fn to_value_is_the_inverse_of_convert() {
    let value = every_variant();
    assert_eq!(convert(&value.to_value()), value);
}

#[test]
fn to_value_makes_a_date_a_datetime() {
    let date = HVal::Date("2026-10-28".to_owned()).to_value();
    assert_eq!(
        date,
        Value::Datetime(Datetime::from_ymd(2026, 10, 28).unwrap())
    );
}

#[test]
fn to_value_keeps_a_date_that_names_no_day_as_its_text() {
    let date = HVal::Date("0000-00-00".to_owned()).to_value();
    assert_eq!(date, Value::Str("0000-00-00".into()));
}
