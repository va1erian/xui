use super::*;

#[test]
fn an_int_reads_as_a_float() {
    assert_eq!(Value::Int(3).as_float(), Some(3.0));
    let ty = ValueType::Float {
        min: Some(0.0),
        max: Some(10.0),
    };
    assert!(ty.accepts(&Value::Int(3)));
    assert!(!ty.accepts(&Value::Float(11.0)));
}

#[test]
fn enum_membership_and_ranges_are_checked() {
    let ty = ValueType::Enum {
        variants: vec!["Horizontal".to_owned()],
    };
    assert!(ty.accepts(&Value::Enum("Horizontal".to_owned())));
    assert!(!ty.accepts(&Value::Enum("Sideways".to_owned())));
    let index = ValueType::Int {
        min: Some(0),
        max: None,
    };
    assert!(!index.accepts(&Value::Int(-1)));
    assert!(!index.accepts(&Value::Text("0".to_owned())));
}

#[test]
fn values_serialise_as_plain_json() {
    let json = serde_json::to_string(&[
        Value::Bool(true),
        Value::Int(-4),
        Value::Text("hi".to_owned()),
        Value::Color(Color::rgb(0x12, 0x34, 0x56)),
        Value::List(vec!["a".to_owned()]),
    ])
    .expect("values serialise");
    assert_eq!(json, r##"[true,-4,"hi","#123456",["a"]]"##);
}
