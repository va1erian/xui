use super::*;

#[test]
fn ints_decode_where_floats_are_expected() {
    let ty = ValueType::Float {
        min: None,
        max: None,
    };
    assert_eq!(
        Value::from_toml(&toml::Value::Integer(3), &ty),
        Ok(Value::Float(3.0))
    );
    assert_eq!(
        Value::from_toml(&toml::Value::Float(1.5), &ty),
        Ok(Value::Float(1.5))
    );
}

#[test]
fn the_schema_decides_text_versus_enum() {
    let text = ValueType::Text { multiline: false };
    let enumeration = ValueType::Enum {
        variants: vec!["left".to_owned(), "right".to_owned()],
    };
    let raw = toml::Value::String("left".to_owned());
    assert_eq!(
        Value::from_toml(&raw, &text),
        Ok(Value::Text("left".to_owned()))
    );
    assert_eq!(
        Value::from_toml(&raw, &enumeration),
        Ok(Value::Enum("left".to_owned()))
    );
}

#[test]
fn colours_parse_with_and_without_alpha() {
    let ty = ValueType::Color;
    let red = toml::Value::String("#ff0000".to_owned());
    let red_alpha = toml::Value::String("#ff000080".to_owned());
    assert_eq!(
        Value::from_toml(&red, &ty),
        Ok(Value::Color(Color::rgb(255, 0, 0)))
    );
    assert_eq!(
        Value::from_toml(&red_alpha, &ty),
        Ok(Value::Color(Color::rgb(255, 0, 0)))
    );
    assert_eq!(
        Value::from_toml(&toml::Value::String("nope".to_owned()), &ty),
        Err(DecodeError::new("a `#rrggbb` colour", "`nope`"))
    );
}

#[test]
fn a_colour_encodes_as_lowercase_hex() {
    let value = Value::Color(Color::rgb(0x12, 0x34, 0x56));
    assert_eq!(toml_literal(&value), "\"#123456\"");
}

#[test]
fn ranges_are_checked() {
    let ty = ValueType::Int {
        min: Some(0),
        max: Some(10),
    };
    assert!(Value::from_toml(&toml::Value::Integer(5), &ty).is_ok());
    assert!(Value::from_toml(&toml::Value::Integer(11), &ty).is_err());
}

#[test]
fn enums_accept_unknown_members_for_validation() {
    let ty = ValueType::Enum {
        variants: vec!["a".to_owned()],
    };
    assert_eq!(
        Value::from_toml(&toml::Value::String("b".to_owned()), &ty),
        Ok(Value::Enum("b".to_owned()))
    );
    assert!(!ty.accepts(&Value::Enum("b".to_owned())));
}

#[test]
fn lists_require_strings() {
    let ty = ValueType::List;
    let raw = toml::Value::Array(vec![
        toml::Value::String("one".to_owned()),
        toml::Value::String("two".to_owned()),
    ]);
    assert_eq!(
        Value::from_toml(&raw, &ty),
        Ok(Value::List(vec!["one".to_owned(), "two".to_owned()]))
    );
    let bad = toml::Value::Array(vec![toml::Value::Integer(1)]);
    assert!(Value::from_toml(&bad, &ty).is_err());
}

#[test]
fn values_round_trip_through_toml() {
    for value in [
        Value::Bool(true),
        Value::Int(-4),
        Value::Float(1.5),
        Value::Text("hello \"world\"".to_owned()),
        Value::Enum("left".to_owned()),
        Value::Color(Color::rgb(1, 2, 3)),
        Value::List(vec!["a".to_owned(), "b".to_owned()]),
    ] {
        let raw = value.to_toml();
        let ty = match &value {
            Value::Bool(_) => ValueType::Bool,
            Value::Int(_) => ValueType::Int {
                min: None,
                max: None,
            },
            Value::Float(_) => ValueType::Float {
                min: None,
                max: None,
            },
            Value::Text(_) => ValueType::Text { multiline: false },
            Value::Enum(_) => ValueType::Enum {
                variants: Vec::new(),
            },
            Value::Color(_) => ValueType::Color,
            Value::List(_) => ValueType::List,
        };
        assert_eq!(Value::from_toml(&raw, &ty), Ok(value));
    }
}
