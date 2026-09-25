#![forbid(unsafe_code)]

//! Routes a backend's [`Event`]s to the widget that owns the target node.
//!
//! A backend decodes native input into [`Event`]s and calls
//! [`Router::dispatch`] with the target [`WidgetId`]. Widgets register a
//! handler for their node, which maps the event to the application's `Msg`.
//! This replaces the old per-`HWND` registries: identity is a portable
//! [`WidgetId`], not a platform handle.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::backend::{Event, WidgetId};

/// A handler that consumes a widget's node events, returning whether it did.
type Handler = Rc<dyn Fn(&Event) -> bool>;

/// Receives events a backend decoded, already targeted at a node.
pub trait WidgetHost {
    /// Delivers `event` to the widget owning `target`, returning whether it was
    /// consumed. A backend may bubble an unconsumed event to the parent.
    fn deliver(&self, target: WidgetId, event: &Event) -> bool;
}

/// The table of node event handlers for one window.
///
/// A handler is cloned out before it runs, so a handler may register or drop
/// another widget without fighting the borrow.
#[derive(Default)]
pub struct Router {
    handlers: RefCell<HashMap<u64, Handler>>,
}

impl Router {
    /// An empty router.
    pub fn new() -> Router {
        Router::default()
    }

    /// Registers the handler for `id`, replacing any previous one.
    pub fn register(&self, id: WidgetId, handler: impl Fn(&Event) -> bool + 'static) {
        self.handlers
            .borrow_mut()
            .insert(id.raw(), Rc::new(handler));
    }

    /// Removes the handler for `id`, if any.
    pub fn unregister(&self, id: WidgetId) {
        self.handlers.borrow_mut().remove(&id.raw());
    }

    /// Offers `event` to the handler for `id`, returning whether it consumed
    /// it. An unregistered target consumes nothing.
    pub fn dispatch(&self, id: WidgetId, event: &Event) -> bool {
        let handler = self.handlers.borrow().get(&id.raw()).cloned();
        match handler {
            Some(handler) => handler(event),
            None => false,
        }
    }

    /// Whether any handler is registered.
    pub fn is_empty(&self) -> bool {
        self.handlers.borrow().is_empty()
    }
}

impl WidgetHost for Router {
    fn deliver(&self, target: WidgetId, event: &Event) -> bool {
        self.dispatch(target, event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Event;

    #[test]
    fn a_registered_handler_receives_its_targets_events() {
        let router = Router::new();
        let id = WidgetId::from_raw(7);
        router.register(id, |event| matches!(event, Event::Close));

        assert!(router.dispatch(id, &Event::Close));
        assert!(!router.dispatch(id, &Event::Wake));
    }

    #[test]
    fn an_unregistered_target_consumes_nothing() {
        let router = Router::new();
        assert!(!router.dispatch(WidgetId::from_raw(1), &Event::Close));
    }

    #[test]
    fn unregistering_removes_the_handler() {
        let router = Router::new();
        let id = WidgetId::from_raw(3);
        router.register(id, |_| true);
        assert!(!router.is_empty());

        router.unregister(id);
        assert!(router.is_empty());
        assert!(!router.dispatch(id, &Event::Close));
    }

    #[test]
    fn a_handler_may_unregister_itself_during_dispatch() {
        let router = Rc::new(Router::new());
        let id = WidgetId::from_raw(9);
        let weak = Rc::downgrade(&router);
        router.register(id, move |_| {
            if let Some(router) = weak.upgrade() {
                router.unregister(id);
            }
            true
        });

        assert!(router.dispatch(id, &Event::Close));
        assert!(router.is_empty());
    }

    #[test]
    fn the_host_trait_forwards_to_dispatch() {
        use std::cell::Cell;

        let router = Router::new();
        let id = WidgetId::from_raw(5);
        let seen = Rc::new(Cell::new(0));
        let counter = Rc::clone(&seen);
        router.register(id, move |event| {
            counter.set(counter.get() + 1);
            event.is_input()
        });

        assert!(router.deliver(id, &Event::Char('x')));
        assert!(!router.deliver(id, &Event::Wake));
        assert_eq!(seen.get(), 2, "the handler ran for both events");
    }
}
