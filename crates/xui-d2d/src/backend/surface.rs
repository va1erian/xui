#![forbid(unsafe_code)]

//! Native child windows that nodes own, for a GPU API to present into: which
//! nodes have one, where it was last put, and the pointer input it passes on
//! as the node's.
//!
//! A child window is made at once (Win32 needs no event loop turn for it) and
//! kept on its node's bounds by [`Shared::place_surfaces`], which the backend
//! runs whenever bounds, visibility, clips, order or popups may have changed.

use std::rc::Rc;
use std::sync::Arc;

use windows::Win32::UI::WindowsAndMessaging::{
    WM_CAPTURECHANGED, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDBLCLK,
    WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE, WM_RBUTTONDBLCLK, WM_RBUTTONDOWN, WM_RBUTTONUP,
    WM_SETCURSOR, WM_XBUTTONDBLCLK, WM_XBUTTONDOWN, WM_XBUTTONUP,
};
use xui_core::backend::{Event, NativeSurface, WidgetId, WindowId};
use xui_core::geometry::Rect;

use super::Shared;
use super::handler::{HTCLIENT, WM_MOUSELEAVE, button_event, high, low};
use super::scene::intersect;
use crate::sys::surface::{self, SurfaceWindow, WM_SURFACE_DESTROY};
use crate::sys::window::{self, Hwnd, Msg};

/// `MK_LBUTTON | MK_RBUTTON | MK_MBUTTON | MK_XBUTTON1 | MK_XBUTTON2`
/// (winuser.h): the button bits of a mouse message's `wparam`.
const BUTTON_BITS: usize = 0x0001 | 0x0002 | 0x0010 | 0x0020 | 0x0040;

/// One node's native child window.
#[derive(Default)]
pub(super) struct Surface {
    /// The OS window, once made.
    window: Option<Arc<SurfaceWindow>>,
    /// Where it was last put and whether it was shown; `None` before the
    /// first placement.
    placed: Option<(Rect, bool)>,
    /// Whether a `WM_MOUSELEAVE` of the child itself is requested.
    tracking: bool,
    /// Whether the child holds the mouse capture a press gave it.
    grabbed: bool,
}

/// Where a node's window goes (its bounds) and whether it is shown: only while
/// the node is visible, has an area, lies wholly inside its clip (a window
/// cannot be clipped the way a painter is, and a part shown would not be the
/// part the app drew) and no popup lies over it (a popup is drawn in the
/// parent, under the window).
pub(super) fn place(
    bounds: Rect,
    clip: Option<Rect>,
    visible: bool,
    popups: &[Rect],
) -> (Rect, bool) {
    let whole = clip.is_none_or(|clip| intersect(bounds, clip) == bounds);
    let covered = popups.iter().any(|p| !intersect(*p, bounds).is_empty());
    (bounds, visible && !bounds.is_empty() && whole && !covered)
}

/// What to change to put a window at `rect` (shown or not), from where it was
/// last put: its position and size, and its visibility; `None` for what stays.
pub(super) fn placement_change(
    placed: Option<(Rect, bool)>,
    rect: Rect,
    shown: bool,
) -> (Option<Rect>, Option<bool>) {
    let moved = (placed.map(|(r, _)| r) != Some(rect) && !rect.is_empty()).then_some(rect);
    let shown = (placed.map(|(_, s)| s) != Some(shown)).then_some(shown);
    (moved, shown)
}

impl Shared {
    /// Records that `id` wants a native child window (`on`) and makes it, or
    /// gives its window up: hidden at once and its handles refused from then
    /// on, as the holder may keep it alive while its parent closes unheard.
    /// A window that could not be made is not tried again until it is asked
    /// for again.
    pub(super) fn set_surface(&self, id: WidgetId, on: bool) {
        if !on {
            if let Some(window) = self.window_of(id) {
                self.drop_surface(id, window, false);
            }
            return;
        }
        if self.surfaces.borrow().contains_key(&id) {
            return;
        }
        let surface = Surface {
            window: self.make_surface(id),
            ..Surface::default()
        };
        self.surfaces.borrow_mut().insert(id, surface);
        if let Some(window) = self.window_of(id) {
            self.place_surfaces(window);
        }
    }

    fn make_surface(&self, id: WidgetId) -> Option<Arc<SurfaceWindow>> {
        let parent = self.hwnd_of(self.window_of(id)?)?;
        let this = self.this.clone();
        let handler: window::Handler = Rc::new(move |hwnd, msg| {
            if msg.id == WM_SURFACE_DESTROY {
                window::destroy(hwnd);
                return Some(0);
            }
            this.upgrade()
                .and_then(|shared| shared.handle_surface(id, hwnd, msg))
        });
        surface::create(parent, handler).map(|hwnd| Arc::new(SurfaceWindow::new(hwnd)))
    }

    /// The native child window `id` was given, once it exists.
    pub(super) fn surface_of(&self, id: WidgetId) -> Option<NativeSurface> {
        let window = self.surfaces.borrow().get(&id)?.window.clone()?;
        Some(window)
    }

    /// Gives `id`'s window up (it is hidden and its handles refused; the OS
    /// window goes when the last holder drops it), or, with `gone`, forgets
    /// one the destruction of its parent already took.
    pub(super) fn drop_surface(&self, id: WidgetId, owner: WindowId, gone: bool) {
        let removed = self.surfaces.borrow_mut().remove(&id);
        let Some(window) = removed.and_then(|surface| surface.window) else {
            return;
        };
        if gone {
            window.gone();
            return;
        }
        window.orphan();
        surface::place(window.hwnd(), None, Some(false));
        // A holder may keep it past its parent: remember it, so closing the
        // parent can say the window went with it.
        let mut orphans = self.orphans.borrow_mut();
        orphans.retain(|(_, kept)| kept.strong_count() > 0);
        orphans.push((owner, Arc::downgrade(&window)));
    }

    /// Says the child windows `window` took down with it are gone, including
    /// the ones their nodes gave up but a holder still has.
    pub(super) fn orphans_gone(&self, window: WindowId) {
        self.orphans.borrow_mut().retain(|(owner, kept)| {
            if *owner != window {
                return kept.strong_count() > 0;
            }
            if let Some(surface) = kept.upgrade() {
                surface.gone();
            }
            false
        });
    }

    /// Makes, moves, shows and hides the native child windows of `window` to
    /// match their nodes.
    pub(super) fn place_surfaces(&self, window: WindowId) {
        if self.surfaces.borrow().is_empty() {
            return;
        }
        let ids: Vec<WidgetId> = self.surfaces.borrow().keys().copied().collect();
        for id in ids {
            if self.window_of(id) != Some(window) {
                continue;
            }
            let Some((rect, shown)) = self.surface_place(id, window) else {
                continue;
            };
            // Recorded before the window is moved: moving it sends messages
            // that read the record.
            let change = {
                let mut surfaces = self.surfaces.borrow_mut();
                let Some(surface) = surfaces.get_mut(&id) else {
                    continue;
                };
                let Some(os) = surface.window.clone() else {
                    continue;
                };
                let (moved, visible) = placement_change(surface.placed, rect, shown);
                let kept = surface.placed.map_or(rect, |(r, _)| r);
                surface.placed = Some((moved.unwrap_or(kept), shown));
                (os, moved, visible)
            };
            let (os, moved, visible) = change;
            if moved.is_some() || visible.is_some() {
                surface::place(os.hwnd(), moved, visible);
            }
        }
    }

    fn surface_place(&self, id: WidgetId, window: WindowId) -> Option<(Rect, bool)> {
        let scene = self.scene.borrow();
        let bounds = scene.absolute_bounds(id)?;
        let clip = scene.ancestor_clip(id);
        let popups: Vec<Rect> = scene
            .iter()
            .filter(|(other, node)| {
                node.popup
                    && node.window == window
                    && *other != id
                    && !scene.is_ancestor(*other, id)
            })
            .filter(|(other, _)| scene.effectively_visible(*other))
            .filter_map(|(other, _)| scene.absolute_bounds(other))
            .collect();
        Some(place(bounds, clip, scene.effectively_visible(id), &popups))
    }

    /// Passes a message of `id`'s child window on to its parent window as
    /// pointer input there.
    fn handle_surface(&self, id: WidgetId, hwnd: Hwnd, msg: Msg) -> Option<isize> {
        let window = self.window_of(id)?;
        let parent = self.hwnd_of(window)?;
        let origin = {
            let surfaces = self.surfaces.try_borrow().ok()?;
            surfaces.get(&id)?.placed?.0
        };
        let (x, y) = (low(msg.lparam) + origin.left, high(msg.lparam) + origin.top);
        let modifiers = window::modifiers();
        match msg.id {
            WM_MOUSEMOVE => {
                self.track_surface(id, hwnd);
                self.pointer(window, Event::MouseMove { x, y, modifiers });
                Some(0)
            }
            WM_MOUSELEAVE => {
                self.untrack_surface(id);
                if !surface::pointer_over(parent) {
                    self.leave(window);
                }
                Some(0)
            }
            WM_CAPTURECHANGED => {
                // Windows took the capture a press gave the child (a modal
                // window, alt-tab) without the node asking: tell the node, as
                // for its own capture. Our own parent taking it is a handoff.
                let ours = msg.lparam == parent.value();
                if !ours && self.set_grabbed(id, false) {
                    self.deliver(window, id, &Event::CaptureChanged);
                }
                Some(0)
            }
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
            | WM_LBUTTONDBLCLK | WM_RBUTTONDBLCLK | WM_MBUTTONDBLCLK | WM_XBUTTONDBLCLK => {
                self.pointer(window, button_event(msg, x, y, modifiers)?);
                // Keep the drag, as a native control would, unless the widget
                // took the capture itself.
                if self.captured.get().is_none() {
                    window::capture(hwnd);
                    self.set_grabbed(id, true);
                }
                Some(0)
            }
            WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP | WM_XBUTTONUP => {
                self.pointer(window, button_event(msg, x, y, modifiers)?);
                if msg.wparam & BUTTON_BITS == 0 && self.set_grabbed(id, false) {
                    window::release_capture();
                }
                Some(0)
            }
            WM_SETCURSOR if low(msg.lparam) as isize == HTCLIENT => self.on_set_cursor(window),
            _ => None,
        }
    }

    /// Records whether the child holds the capture; whether it did before.
    fn set_grabbed(&self, id: WidgetId, grabbed: bool) -> bool {
        self.surfaces
            .borrow_mut()
            .get_mut(&id)
            .is_some_and(|surface| std::mem::replace(&mut surface.grabbed, grabbed))
    }

    fn track_surface(&self, id: WidgetId, hwnd: Hwnd) {
        if let Some(surface) = self.surfaces.borrow_mut().get_mut(&id)
            && !surface.tracking
        {
            surface.tracking = true;
            window::track_leave(hwnd);
        }
    }

    fn untrack_surface(&self, id: WidgetId) {
        if let Some(surface) = self.surfaces.borrow_mut().get_mut(&id) {
            surface.tracking = false;
        }
    }

    /// The pointer left the parent's own area: when it went onto a native
    /// child it is still in the window, so the hover stays.
    pub(super) fn parent_left(&self, window: WindowId, hwnd: Hwnd) {
        if surface::pointer_over_child(hwnd) {
            if let Some(state) = self.windows.borrow_mut().get_mut(&window.raw()) {
                state.tracking = false;
            }
        } else {
            self.leave(window);
        }
    }
}

#[cfg(test)]
#[path = "surface_tests.rs"]
mod tests;
