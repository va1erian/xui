#![forbid(unsafe_code)]

//! A generic property surface for widgets.
//!
//! A form designer (or any tool) needs to read and edit a widget's properties
//! without knowing its concrete type. A widget implements [`Properties`] and
//! reports a small, stable set: a name and a typed [`Value`]. Setters go
//! through the widget's own typed API, so editing a property is exactly what
//! the widget would do itself. [`Properties::schema`] describes the set for a
//! designer's editors.

use crate::Color;
use crate::image::Image;
use crate::units::Dip;

/// A font override for a widget that paints text: a family, a size in design
/// units and the two styles.
///
/// A widget whose theme style already sets a weight (a title, a caption)
/// keeps it; `bold` adds to it. A `family` of `None` follows the theme's font.
#[derive(Clone, Debug, PartialEq)]
pub struct Font {
    /// The font family, or `None` for the theme's own.
    pub family: Option<String>,
    /// The size in design units.
    pub size: Dip,
    /// Whether the text is bold.
    pub bold: bool,
    /// Whether the text is italic.
    pub italic: bool,
}

impl Font {
    /// A regular, upright font of `size` design units in the theme's family.
    pub fn new(size: Dip) -> Font {
        Font {
            family: None,
            size,
            bold: false,
            italic: false,
        }
    }

    /// Uses `family`.
    pub fn family(mut self, family: impl Into<String>) -> Font {
        self.family = Some(family.into());
        self
    }

    /// Uses a bold face.
    pub fn bold(mut self) -> Font {
        self.bold = true;
        self
    }

    /// Uses an italic face.
    pub fn italic(mut self) -> Font {
        self.italic = true;
        self
    }
}

/// A property's value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A boolean (enabled, visible, checked).
    Bool(bool),
    /// A whole number (a progress value or range).
    Integer(i64),
    /// A real number (a slider's value).
    Float(f64),
    /// A string (a label, an edit's text).
    Text(String),
    /// One of a fixed set of strings (an icon size).
    Enum(String),
    /// An opaque colour (a colour picker's selection).
    Color(Color),
    /// A font override (a label's font).
    Font(Font),
    /// A length in design units (a split's divider).
    Dip(Dip),
    /// A picture (a picture widget's content). No xui-core widget reports one
    /// yet: it is the typed home for a picture widget's property, so a
    /// designer's image editor has one.
    Image(Image),
}

impl Value {
    /// The variant's name, for diagnostics.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Bool(_) => "bool",
            Value::Integer(_) => "int",
            Value::Float(_) => "float",
            Value::Text(_) => "text",
            Value::Enum(_) => "enum",
            Value::Color(_) => "color",
            Value::Font(_) => "font",
            Value::Dip(_) => "dip",
            Value::Image(_) => "image",
        }
    }

    /// The boolean inside, if any.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// The integer inside, if any.
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Value::Integer(value) => Some(*value),
            _ => None,
        }
    }

    /// The real number inside, widening a [`Value::Integer`].
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(value) => Some(*value),
            Value::Integer(value) => Some(*value as f64),
            _ => None,
        }
    }

    /// The string inside, for both [`Value::Text`] and [`Value::Enum`].
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Text(value) | Value::Enum(value) => Some(value),
            _ => None,
        }
    }

    /// The colour inside, if any.
    pub fn as_color(&self) -> Option<Color> {
        match self {
            Value::Color(value) => Some(*value),
            _ => None,
        }
    }

    /// The font inside, if any.
    pub fn as_font(&self) -> Option<&Font> {
        match self {
            Value::Font(value) => Some(value),
            _ => None,
        }
    }

    /// The design-unit length inside, if any.
    pub fn as_dip(&self) -> Option<Dip> {
        match self {
            Value::Dip(value) => Some(*value),
            _ => None,
        }
    }

    /// The picture inside, if any.
    pub fn as_image(&self) -> Option<&Image> {
        match self {
            Value::Image(value) => Some(value),
            _ => None,
        }
    }
}

/// What a property's value is, so a designer can build the right editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PropertyType {
    /// A boolean (a check box).
    Bool,
    /// A whole number (a stepper).
    Integer,
    /// A real number (a number field).
    Float,
    /// A string (a text field).
    Text,
    /// One of a fixed set of strings (a drop-down).
    Enum(&'static [&'static str]),
    /// A colour (a colour picker).
    Color,
    /// A font (a font editor).
    Font,
    /// A length in design units (a number with a unit).
    Dip,
    /// A picture (a picture editor).
    Image,
}
/// One property's schema: what a designer needs to present it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PropertyInfo {
    /// The stable property name, as [`Properties::properties`] reports it.
    pub name: &'static str,
    /// The value's type.
    pub ty: PropertyType,
    /// Whether the property is shown but not editable.
    pub read_only: bool,
    /// The group a property grid sorts it under (empty for none).
    pub category: &'static str,
    /// A one-line description, for a tooltip (empty for none).
    pub description: &'static str,
}

impl PropertyInfo {
    /// A read-write property with no category or description.
    pub const fn new(name: &'static str, ty: PropertyType) -> PropertyInfo {
        PropertyInfo {
            name,
            ty,
            read_only: false,
            category: "",
            description: "",
        }
    }

    /// Marks the property as shown but not editable.
    pub const fn read_only(mut self) -> PropertyInfo {
        self.read_only = true;
        self
    }

    /// Sets the group a property grid sorts it under and its description.
    pub const fn described(
        mut self,
        category: &'static str,
        description: &'static str,
    ) -> PropertyInfo {
        self.category = category;
        self.description = description;
        self
    }
}

/// One named property of a widget.
#[derive(Clone, Debug, PartialEq)]
pub struct Property {
    /// The stable property name (a form file keys on it).
    pub name: &'static str,
    /// The current value.
    pub value: Value,
}

/// A widget that reports and edits a small, generic property set.
pub trait Properties {
    /// A snapshot of the widget's properties, in a stable order.
    fn properties(&self) -> Vec<Property>;

    /// The value of the property named `name`, if the widget has it.
    fn property(&self, name: &str) -> Option<Value> {
        self.properties()
            .into_iter()
            .find(|property| property.name == name)
            .map(|property| property.value)
    }

    /// Sets the property named `name` to `value`, returning whether it applied.
    /// A mismatched value type is ignored (returns `false`).
    fn set_property(&self, name: &str, value: Value) -> bool;

    /// The schema of the widget's properties: one [`PropertyInfo`] per
    /// property [`properties`](Self::properties) reports, so a designer can
    /// build the right editor for each.
    ///
    /// The default infers each type from the current value and carries no
    /// category or description; a widget whose property is an enum, a colour,
    /// a font, a length or a picture overrides it with the exact type.
    fn schema(&self) -> Vec<PropertyInfo> {
        self.properties()
            .into_iter()
            .map(|property| {
                let ty = match property.value {
                    Value::Bool(_) => PropertyType::Bool,
                    Value::Integer(_) => PropertyType::Integer,
                    Value::Float(_) => PropertyType::Float,
                    Value::Text(_) => PropertyType::Text,
                    Value::Enum(_) => PropertyType::Enum(&[]),
                    Value::Color(_) => PropertyType::Color,
                    Value::Font(_) => PropertyType::Font,
                    Value::Dip(_) => PropertyType::Dip,
                    Value::Image(_) => PropertyType::Image,
                };
                PropertyInfo::new(property.name, ty)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::dip;

    #[test]
    fn values_carry_their_type() {
        assert_eq!(Value::Bool(true).type_name(), "bool");
        assert_eq!(Value::Integer(3).as_integer(), Some(3));
        assert_eq!(Value::Integer(3).as_float(), Some(3.0));
        assert_eq!(Value::Float(1.5).as_float(), Some(1.5));
        assert_eq!(Value::Text("a".to_owned()).as_str(), Some("a"));
        assert_eq!(Value::Enum("small".to_owned()).as_str(), Some("small"));
        assert_eq!(
            Value::Color(Color::rgb(1, 2, 3)).as_color(),
            Some(Color::rgb(1, 2, 3))
        );
        assert_eq!(Value::Dip(dip(4.0)).as_dip(), Some(dip(4.0)));
        let font = Value::Font(Font::new(dip(14.0)).bold());
        assert_eq!(font.as_font(), Some(&Font::new(dip(14.0)).bold()));
        assert!(Value::Bool(false).as_font().is_none());
    }

    #[test]
    fn a_picture_value_keeps_the_image() {
        let image = Image::from_rgba(1, 1, vec![1, 2, 3, 4]).expect("one pixel");
        let value = Value::Image(image);
        assert_eq!(value.type_name(), "image");
        assert_eq!(
            value.as_image().map(Image::id),
            Some(value.as_image().unwrap().id())
        );
    }

    /// A widget that reports one property of each kind.
    struct EveryKind;

    impl Properties for EveryKind {
        fn properties(&self) -> Vec<Property> {
            vec![
                Property {
                    name: "flag",
                    value: Value::Bool(true),
                },
                Property {
                    name: "step",
                    value: Value::Integer(2),
                },
            ]
        }

        fn set_property(&self, _name: &str, _value: Value) -> bool {
            false
        }
    }

    #[test]
    fn the_default_schema_infers_each_type() {
        assert_eq!(
            EveryKind.schema(),
            [
                PropertyInfo::new("flag", PropertyType::Bool),
                PropertyInfo::new("step", PropertyType::Integer),
            ]
        );
    }

    #[test]
    fn a_property_info_carries_its_metadata() {
        let info = PropertyInfo::new("font", PropertyType::Font)
            .read_only()
            .described("Appearance", "The label's font.");
        assert_eq!(info.name, "font");
        assert_eq!(info.ty, PropertyType::Font);
        assert!(info.read_only);
        assert_eq!(info.category, "Appearance");
        assert_eq!(info.description, "The label's font.");
    }

    #[test]
    fn a_font_builds_from_its_parts() {
        let font = Font::new(dip(14.0)).family("Consolas").bold().italic();
        assert_eq!(
            font,
            Font {
                family: Some("Consolas".to_owned()),
                size: dip(14.0),
                bold: true,
                italic: true,
            }
        );
        assert_eq!(Font::new(dip(12.0)).family, None);
    }
}
