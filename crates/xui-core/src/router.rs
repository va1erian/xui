#![forbid(unsafe_code)]

//! Routes a backend's [`Event`]s to the widget that owns the target node.
//!
//! A backend decodes native input into [`Event`]s and calls
//! [`Router::dispatch`] with the target [`WidgetId`]. Widgets register a
//! handler for their node, which maps the event to the application's `Msg`.
//! This replaces the old per-`HWND` registries: identity is a portable
//! [`WidgetId`], not a platform handle.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use crate::backend::{Event, WidgetId};

/// A handler that consumes a widget's node events, returning whether it did.
type Handler = Rc<dyn Fn(&Event) -> bool>;

/// A listener installed with [`Router::add`], to remove it again.
type Token = usize;

/// Receives events a backend decoded, already targeted at a node.
pub trait WidgetHost {
    /// Delivers `event` to the widget owning `target`, returning whether it was
    /// consumed. Window-level events target [`WidgetId::NONE`].
    ///
    /// The return value lets a host report that it handled the event; event
    /// bubbling to a parent is not implemented yet, so a backend that gets
    /// `false` may, for example, fall through to its platform default.
    fn deliver(&self, target: WidgetId, event: &Event) -> bool;
}

/// The table of node event handlers for one window.
///
/// A node usually has one handler, registered by the widget that owns it, but
/// it may have several: [`Router::add`] appends a listener without disturbing
/// the owner's handler, so a side feature (a tooltip) can observe a widget's
/// events. Every handler for a node runs, in registration order, and the
/// handlers are cloned out before any runs, so a handler may register or drop
/// another widget without fighting the borrow.
#[derive(Default)]
pub struct Router {
    handlers: RefCell<HashMap<u64, Vec<(Token, Handler)>>>,
    next_token: Cell<Token>,
}

impl Router {
    /// An empty router.
    pub fn new() -> Router {
        Router::default()
    }

    /// Allocates a token unique within this router.
    fn token(&self) -> Token {
        let token = self.next_token.get();
        self.next_token.set(token.wrapping_add(1));
        token
    }

    /// Registers the handler for `id`, replacing any previous one.
    pub fn register(&self, id: WidgetId, handler: impl Fn(&Event) -> bool + 'static) {
        let token = self.token();
        self.handlers
            .borrow_mut()
            .insert(id.raw(), vec![(token, Rc::new(handler))]);
    }

    /// Adds `handler` to `id`'s listeners without removing the ones already
    /// there, and returns a token to remove it again with [`Router::remove`].
    pub(crate) fn add(&self, id: WidgetId, handler: impl Fn(&Event) -> bool + 'static) -> Token {
        let token = self.token();
        self.handlers
            .borrow_mut()
            .entry(id.raw())
            .or_default()
            .push((token, Rc::new(handler)));
        token
    }

    /// Removes the listener `token` returned by [`Router::add`], leaving the
    /// node's other handlers untouched.
    pub(crate) fn remove(&self, id: WidgetId, token: Token) {
        let mut handlers = self.handlers.borrow_mut();
        if let Some(listeners) = handlers.get_mut(&id.raw()) {
            listeners.retain(|(existing, _)| *existing != token);
            if listeners.is_empty() {
                handlers.remove(&id.raw());
            }
        }
    }

    /// Removes every handler for `id`, if any.
    pub fn unregister(&self, id: WidgetId) {
        self.handlers.borrow_mut().remove(&id.raw());
    }

    /// Offers `event` to every handler for `id`, returning whether any
    /// consumed it. An unregistered target consumes nothing.
    pub fn dispatch(&self, id: WidgetId, event: &Event) -> bool {
        let handlers = self.handlers.borrow().get(&id.raw()).cloned();
        match handlers {
            Some(handlers) => handlers
                .iter()
                .fold(false, |used, (_, handler)| handler(event) || used),
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
    fn an_added_listener_does_not_replace_the_registered_handler() {
        let router = Router::new();
        let id = WidgetId::from_raw(4);
        let seen = Rc::new(Cell::new(0));
        let owner = Rc::clone(&seen);
        let listener = Rc::clone(&seen);
        router.register(id, move |_| {
            owner.set(owner.get() + 1);
            true
        });
        let token = router.add(id, move |_| {
            listener.set(listener.get() + 10);
            false
        });

        assert!(router.dispatch(id, &Event::Close));
        assert_eq!(seen.get(), 11, "both handlers ran");

        router.remove(id, token);
        assert!(router.dispatch(id, &Event::Close));
        assert_eq!(seen.get(), 12, "removing the listener kept the owner");
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
