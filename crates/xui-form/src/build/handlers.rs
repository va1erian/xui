#![forbid(unsafe_code)]

//! [`Handlers`]: a [`Binder`] made of closures attached by name.

use std::collections::BTreeMap;
use std::rc::Rc;

use super::{Binder, EventHandler, EventRef};
use crate::value::Value;

/// A [`Binder`] for a Rust app: closures attached by widget and event name.
///
/// ```
/// use xui_form::{Handlers, Value};
///
/// #[derive(Clone)]
/// enum Msg { Greet, Digit(usize) }
///
/// let handlers = Handlers::new()
///     .on("greet_button", "Click", Msg::Greet)
///     .on_with("digit", "Click", |index, _args: &[Value]| index.map(Msg::Digit));
/// ```
pub struct Handlers<M> {
    handlers: BTreeMap<(String, String), Handler<M>>,
}

/// One attached closure: from the element's index and the event's arguments
/// to a message.
type Handler<M> = Rc<dyn Fn(Option<usize>, &[Value]) -> Option<M>>;

impl<M> Default for Handlers<M> {
    fn default() -> Handlers<M> {
        Handlers {
            handlers: BTreeMap::new(),
        }
    }
}

impl<M: 'static> Handlers<M> {
    /// No handlers: every event stays unwired.
    pub fn new() -> Handlers<M> {
        Handlers::default()
    }

    /// Raises `msg` when the widget (or control array) `name` raises `event`.
    pub fn on(self, name: &str, event: &str, msg: M) -> Handlers<M>
    where
        M: Clone,
    {
        self.on_with(name, event, move |_, _| Some(msg.clone()))
    }

    /// Maps `name`'s `event` through `f`, which gets the element's index (for
    /// a control array) and the event's arguments, and may raise nothing.
    pub fn on_with(
        mut self,
        name: &str,
        event: &str,
        f: impl Fn(Option<usize>, &[Value]) -> Option<M> + 'static,
    ) -> Handlers<M> {
        self.handlers
            .insert((name.to_owned(), event.to_owned()), Rc::new(f));
        self
    }
}

impl<M: 'static> Binder<M> for Handlers<M> {
    fn bind(&self, event: EventRef<'_>) -> Option<EventHandler<M>> {
        let key = (event.node.to_owned(), event.event.to_owned());
        let handler = Rc::clone(self.handlers.get(&key)?);
        let index = event.index;
        Some(Rc::new(move |args| handler(index, args)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Catalog;

    #[test]
    fn handlers_bind_by_name_and_pass_the_index() {
        let handlers =
            Handlers::new()
                .on("ok", "Click", "ok")
                .on_with("digit", "Click", |index, _| index.map(|_| "digit"));
        let catalog = Catalog::xui();
        let click = catalog
            .get("Button")
            .and_then(|spec| spec.event("Click"))
            .expect("a button clicks");
        let bind = |node, index| {
            handlers.bind(EventRef {
                node,
                index,
                event: "Click",
                spec: click,
            })
        };
        assert_eq!(bind("ok", None).and_then(|h| h(&[])), Some("ok"));
        assert_eq!(bind("digit", Some(3)).and_then(|h| h(&[])), Some("digit"));
        assert!(bind("other", None).is_none());
    }
}
