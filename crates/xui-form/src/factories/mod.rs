#![forbid(unsafe_code)]

//! The [`Factories::xui`] registry: a factory for every widget kind in
//! [`Catalog::xui`](crate::Catalog::xui).
//!
//! Each factory describes its widget with the `xui_core::arrange` builder,
//! forwards the properties the widget can set, wires only the events the
//! widget raises, and binds a [`Handle`] its property surface reads and
//! writes once the form is mounted.

use xui_core::WidgetId;
use xui_core::arrange::Handle;
use xui_core::widget::Placeable;

use crate::build::Factories;

mod choice;
mod containers;
mod range;
mod text;

impl<M: 'static> Factories<M> {
    /// A registry with a factory for every built-in `xui-core` kind.
    pub fn xui() -> Self {
        let mut factories = Factories::new();
        text::register(&mut factories);
        choice::register(&mut factories);
        range::register(&mut factories);
        containers::register(&mut factories);
        factories
    }
}

/// The node of the widget behind `handle`.
fn id_of<M: 'static, W: Placeable<M>>(handle: &Handle<W>) -> WidgetId {
    handle.get().id()
}

/// Declares a unit factory struct for `$kind` whose `create` is `$create`.
macro_rules! factory {
    ($name:ident, $kind:literal, $create:expr) => {
        struct $name;

        impl<M: 'static> crate::build::WidgetFactory<M> for $name {
            fn kind(&self) -> &str {
                $kind
            }

            fn create(&self, cx: &mut crate::build::BuildCx<'_, M>) -> crate::build::Created<M> {
                $create(cx)
            }
        }
    };
}
use factory;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_xui_registry_covers_every_catalog_kind() {
        let catalog = crate::Catalog::xui();
        let factories: Factories<()> = Factories::xui();
        for kind in catalog.kinds() {
            assert!(
                factories.get(kind).is_some(),
                "no factory registered for {kind}"
            );
        }
    }
}
