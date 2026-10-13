#![forbid(unsafe_code)]

//! The macros that declare node types once: each declaration makes the serde
//! struct of the file format, its defaults, a serializer that omits them,
//! and the schema the `Catalog` reports, so the format and the schema cannot
//! drift.

/// Declares a node struct from its own fields, then appends the layout
/// fields every node has (see [`Place`](crate::model::Place)).
macro_rules! node {
    (
        $(#[doc = $doc:literal])*
        $name:ident {
            $( $(#[doc = $fdoc:literal])* $field:ident : $ty:ty = $default:expr ),* $(,)?
        }
    ) => {
        $crate::model::macros::node_inner! {
            $(#[doc = $doc])*
            $name {
                $( $(#[doc = $fdoc])* $field : $ty = $default, )*
                /// A share of the parent's leftover space along its main axis, by
                /// weight (in a grid, the entry's row fills).
                fill: Option<u32> = None,
                /// Exactly this wide, in design units.
                width: Option<$crate::model::Length> = None,
                /// Exactly this tall, in design units.
                height: Option<$crate::model::Length> = None,
                /// At most this wide, in design units.
                max_width: Option<$crate::model::Length> = None,
                /// At most this tall, in design units.
                max_height: Option<$crate::model::Length> = None,
                /// Where the entry sits across its parent's main axis (in a grid,
                /// within its cell), instead of the parent's `align`.
                align: Option<$crate::model::Align> = None,
                /// In a `Grid`, the number of columns the entry covers.
                span: Option<u32> = None,
                /// In an `Absolute` layout, `(x, y, width, height)` in design
                /// units from the layout's inner top-left corner.
                at: Option<(
                    $crate::model::Length,
                    $crate::model::Length,
                    $crate::model::Length,
                    $crate::model::Length,
                )> = None,
                /// In an `Absolute` layout, how the entry follows the layout as
                /// it grows or shrinks from its design size.
                anchor: Option<$crate::model::Anchor> = None,
            }
        }

        impl $name {
            /// The node's layout fields.
            pub fn place(&self) -> $crate::model::Place {
                $crate::model::Place {
                    fill: self.fill,
                    width: self.width,
                    height: self.height,
                    max_width: self.max_width,
                    max_height: self.max_height,
                    align: self.align,
                    span: self.span,
                    at: self.at,
                    anchor: self.anchor,
                }
            }
        }
    };
}

/// Declares the struct, its [`Default`], a serializer that omits every field
/// equal to its default, and the field list for the schema.
macro_rules! node_inner {
    (
        $(#[doc = $doc:literal])*
        $name:ident {
            $( $(#[doc = $fdoc:literal])* $field:ident : $ty:ty = $default:expr ),* $(,)?
        }
    ) => {
        $(#[doc = $doc])*
        #[derive(Clone, Debug, PartialEq, serde::Deserialize)]
        #[serde(default, deny_unknown_fields)]
        pub struct $name {
            $( $(#[doc = $fdoc])* pub $field: $ty, )*
        }

        impl Default for $name {
            fn default() -> $name {
                $name { $( $field: $default, )* }
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                use serde::ser::SerializeStruct;
                let default = <$name as Default>::default();
                // RON lays a struct out by its field count, so it must be exact.
                let len = 0 $( + usize::from(self.$field != default.$field) )*;
                let mut out = serializer.serialize_struct(stringify!($name), len)?;
                $(
                    if self.$field == default.$field {
                        out.skip_field(stringify!($field))?;
                    } else {
                        out.serialize_field(stringify!($field), &self.$field)?;
                    }
                )*
                out.end()
            }
        }

        impl $name {
            /// The fields of the file format, with their RON types and
            /// defaults, for the schema.
            pub(crate) fn fields() -> Vec<$crate::schema::FieldSpec> {
                let default = <$name as Default>::default();
                vec![
                    $(
                        $crate::schema::FieldSpec::new(
                            stringify!($field),
                            stringify!($ty),
                            &default.$field,
                            &[$($fdoc),*],
                        ),
                    )*
                ]
            }
        }
    };
}

/// Declares a widget node: a [`node`] with `name`, `array`, `index`, its
/// properties, `visible`, `enabled` and any `content` (nested nodes, which
/// are not properties), plus its [`Widget`](crate::model::Widget)
/// implementation, whose spec lists the properties and events.
///
/// A property is `name: Type = default; Category`, optionally followed by
/// `(design)` for a construction-only property and `=> ValueType` for a type
/// other than the Rust type's own.
macro_rules! widget {
    (
        $(#[doc = $doc:literal])*
        $name:ident {
            $(
                $(#[doc = $pdoc:literal])*
                $prop:ident : $ty:ty = $default:expr ; $category:ident
                $( ( $access:ident ) )? $( => $vt:expr )?
            ),* $(,)?
        }
        events {
            $( $(#[doc = $edoc:literal])* $event:ident ( $( $arg:ident : $aty:ty ),* ) ),* $(,)?
        }
        $(
            content {
                $( $(#[doc = $cdoc:literal])* $cfield:ident : $cty:ty = $cdefault:expr ),* $(,)?
            }
        )?
    ) => {
        $crate::model::macros::node! {
            $(#[doc = $doc])*
            $name {
                /// The widget's name: the identifier scripts, binders and the
                /// live form address it by.
                name: String = String::new(),
                /// Makes this node a control array of this many elements, named
                /// `name[0]`, `name[1]`, …; a `{index}` in its text becomes the
                /// element's index.
                array: Option<u32> = None,
                /// Makes this node one element of the control array `name`.
                index: Option<u32> = None,
                $( $(#[doc = $pdoc])* $prop : $ty = $default, )*
                /// Whether the widget is shown.
                visible: bool = true,
                /// Whether the widget accepts input.
                enabled: bool = true,
                $($( $(#[doc = $cdoc])* $cfield : $cty = $cdefault, )*)?
            }
        }

        impl $crate::model::Widget for $name {
            fn kind(&self) -> &'static str {
                stringify!($name)
            }

            fn name(&self) -> &str {
                &self.name
            }

            fn array(&self) -> Option<u32> {
                self.array
            }

            fn index(&self) -> Option<u32> {
                self.index
            }

            fn props(&self) -> std::collections::BTreeMap<String, $crate::value::Value> {
                use $crate::model::PropType;
                let mut props = std::collections::BTreeMap::new();
                $( props.insert(stringify!($prop).to_owned(), self.$prop.to_value()); )*
                props.insert("visible".to_owned(), self.visible.to_value());
                props.insert("enabled".to_owned(), self.enabled.to_value());
                props
            }

            fn place(&self) -> $crate::model::Place {
                $name::place(self)
            }
        }

        impl $name {
            /// The widget's spec: its properties and events.
            pub(crate) fn spec() -> $crate::schema::WidgetSpec {
                #[allow(unused_imports)]
                use $crate::model::PropType;
                let default = <$name as Default>::default();
                let mut events: Vec<$crate::schema::EventSpec> = vec![
                    $(
                        $crate::schema::EventSpec {
                            name: stringify!($event).to_owned(),
                            args: vec![
                                $( $crate::schema::ArgSpec {
                                    name: stringify!($arg).to_owned(),
                                    ty: <$aty as PropType>::value_type(),
                                }, )*
                            ],
                            is_default: false,
                            description: $crate::model::doc(&[$($edoc),*]),
                        },
                    )*
                ];
                if let Some(first) = events.first_mut() {
                    first.is_default = true;
                }
                $crate::schema::WidgetSpec {
                    kind: stringify!($name).to_owned(),
                    description: $crate::model::doc(&[$($doc),*]),
                    properties: vec![
                        $(
                            $crate::schema::PropertySpec {
                                name: stringify!($prop).to_owned(),
                                ty: $crate::model::macros::value_type!($ty $(, $vt)?),
                                default: default.$prop.to_value(),
                                category: stringify!($category).to_owned(),
                                description: $crate::model::doc(&[$($pdoc),*]),
                                access: $crate::model::macros::access!($($access)?),
                            },
                        )*
                    ],
                    events,
                    fields: $name::fields(),
                }
            }
        }
    };
}

/// The schema type of a property: the Rust type's own, or an override.
macro_rules! value_type {
    ($ty:ty) => {
        <$ty as $crate::model::PropType>::value_type()
    };
    ($ty:ty, $vt:expr) => {
        $vt
    };
}

/// A property's access: read-write, or `design` for construction-only.
macro_rules! access {
    () => {
        $crate::schema::Access::ReadWrite
    };
    (design) => {
        $crate::schema::Access::DesignOnly
    };
}

pub(crate) use {access, node, node_inner, value_type, widget};
