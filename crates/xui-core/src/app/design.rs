#![forbid(unsafe_code)]

//! Design-mode scopes: a form designer switches a subtree, not the window.

use std::cell::Cell;
use std::rc::Rc;

/// One node of the design-mode scope chain. A scope is in design mode when its
/// own flag or any ancestor's is set, so the check is a short pointer walk
/// (containers nest a few levels) and always sees later changes.
pub(crate) struct DesignScope {
    on: Cell<bool>,
    parent: Option<Rc<DesignScope>>,
}

impl DesignScope {
    /// A root scope, off.
    pub(crate) fn root() -> Rc<DesignScope> {
        Rc::new(DesignScope {
            on: Cell::new(false),
            parent: None,
        })
    }

    /// A child scope of `parent`, off itself but inheriting `parent`'s state.
    pub(crate) fn child(parent: &Rc<DesignScope>) -> Rc<DesignScope> {
        Rc::new(DesignScope {
            on: Cell::new(false),
            parent: Some(Rc::clone(parent)),
        })
    }

    /// Whether this scope or an ancestor is in design mode.
    pub(crate) fn active(&self) -> bool {
        let mut scope = self;
        loop {
            if scope.on.get() {
                return true;
            }
            match &scope.parent {
                Some(parent) => scope = parent,
                None => return false,
            }
        }
    }

    /// Sets this scope's own flag.
    pub(crate) fn set(&self, on: bool) {
        self.on.set(on);
    }
}
