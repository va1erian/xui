//! The view's input as Blitz's: pointer, wheel and keys, in CSS pixels.
//!
//! The UI thread sends [`Input`] in client CSS pixels; the engine knows the
//! document's scroll offset and turns it into a [`UiEvent`] with page
//! coordinates. Typed text arrives as `Char` and becomes a key press carrying
//! that text; a letter key is passed on only with Ctrl or Alt held (Ctrl+C,
//! Ctrl+A), since its character follows as a `Char`.

use std::sync::Arc;

use blitz_traits::events::{
    BlitzKeyEvent, BlitzPointerEvent, BlitzPointerId, BlitzWheelDelta, BlitzWheelEvent, KeyState,
    MouseEventButton, MouseEventButtons, PointerCoords, PointerDetails, UiEvent,
};
use keyboard_types::{Code, Key, Location, Modifiers};
use xui_core::message::{Key as XKey, Modifiers as XMods, MouseButton};

/// CSS pixels one wheel notch scrolls.
pub(crate) const WHEEL_LINE: f64 = 60.0;

/// What the pointer did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PointerAction {
    Move,
    Down,
    Up,
}

/// One piece of input, in client CSS pixels.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Input {
    Pointer {
        action: PointerAction,
        x: f32,
        y: f32,
        button: MouseButton,
        /// The buttons held after this event.
        held: MouseEventButtons,
        mods: XMods,
    },
    /// `notches` of the wheel; positive rolls away from the user (up).
    Wheel {
        notches: f64,
        horizontal: bool,
        x: f32,
        y: f32,
        mods: XMods,
    },
    Key {
        key: XKey,
        mods: XMods,
        repeat: bool,
    },
    Char(char),
}

fn mods(m: XMods) -> Modifiers {
    let mut out = Modifiers::empty();
    out.set(Modifiers::CONTROL, m.ctrl);
    out.set(Modifiers::SHIFT, m.shift);
    out.set(Modifiers::ALT, m.alt);
    out.set(Modifiers::META, m.win);
    out
}

fn button(b: MouseButton) -> MouseEventButton {
    match b {
        MouseButton::Right => MouseEventButton::Secondary,
        MouseButton::Middle => MouseEventButton::Auxiliary,
        _ => MouseEventButton::Main,
    }
}

/// `input` as Blitz's event, with the document scrolled by `scroll`; `None`
/// for a key Blitz has no use for.
pub(crate) fn to_ui_event(input: &Input, scroll: (f64, f64)) -> Option<UiEvent> {
    let coords = |x: f32, y: f32| PointerCoords {
        page_x: x + scroll.0 as f32,
        page_y: y + scroll.1 as f32,
        screen_x: x,
        screen_y: y,
        client_x: x,
        client_y: y,
    };
    Some(match *input {
        Input::Pointer {
            action,
            x,
            y,
            button: b,
            held,
            mods: m,
        } => {
            let event = BlitzPointerEvent {
                id: BlitzPointerId::Mouse,
                is_primary: true,
                coords: coords(x, y),
                button: button(b),
                buttons: held,
                mods: mods(m),
                details: PointerDetails {
                    pressure: if held.is_empty() { 0.0 } else { 0.5 },
                    ..PointerDetails::default()
                },
                element: Default::default(),
                active_pointers: Arc::default(),
            };
            match action {
                PointerAction::Move => UiEvent::PointerMove(event),
                PointerAction::Down => UiEvent::PointerDown(event),
                PointerAction::Up => UiEvent::PointerUp(event),
            }
        }
        Input::Wheel {
            notches,
            horizontal,
            x,
            y,
            mods: m,
        } => {
            // Blitz takes winit's signs: a positive delta moves the content
            // down (scrolls up), as rolling the wheel away does; tilting it
            // right (a positive horizontal notch) scrolls right.
            let pixels = notches * WHEEL_LINE;
            let (dx, dy) = if horizontal {
                (-pixels, 0.0)
            } else {
                (0.0, pixels)
            };
            UiEvent::Wheel(BlitzWheelEvent {
                delta: BlitzWheelDelta::Pixels(dx, dy),
                coords: coords(x, y),
                buttons: MouseEventButtons::None,
                mods: mods(m),
                element: Default::default(),
            })
        }
        Input::Key {
            key,
            mods: m,
            repeat,
        } => UiEvent::KeyDown(key_event(blitz_key(key, m)?, None, mods(m), repeat)),
        Input::Char(c) => {
            let text = c.to_string();
            UiEvent::KeyDown(key_event(
                Key::Character(text.clone()),
                Some(text),
                Modifiers::empty(),
                false,
            ))
        }
    })
}

fn key_event(key: Key, text: Option<String>, modifiers: Modifiers, repeat: bool) -> BlitzKeyEvent {
    BlitzKeyEvent {
        key,
        code: Code::Unidentified,
        modifiers,
        location: Location::Standard,
        is_auto_repeating: repeat,
        is_composing: false,
        state: KeyState::Pressed,
        text: text.map(Into::into),
    }
}

/// The Blitz key for an xui key, when Blitz needs it: editing and movement
/// keys, and letters only as shortcuts.
fn blitz_key(key: XKey, m: XMods) -> Option<Key> {
    Some(match key {
        XKey::BACK => Key::Backspace,
        XKey::TAB => Key::Tab,
        XKey::RETURN => Key::Enter,
        XKey::ESCAPE => Key::Escape,
        XKey::LEFT => Key::ArrowLeft,
        XKey::RIGHT => Key::ArrowRight,
        XKey::UP => Key::ArrowUp,
        XKey::DOWN => Key::ArrowDown,
        XKey::HOME => Key::Home,
        XKey::END => Key::End,
        XKey::PAGE_UP => Key::PageUp,
        XKey::PAGE_DOWN => Key::PageDown,
        XKey::DELETE => Key::Delete,
        XKey::INSERT => Key::Insert,
        k if (m.ctrl || m.alt) && (XKey::A.code()..=XKey::Z.code()).contains(&k.code()) => {
            let letter = (b'a' + (k.code() - XKey::A.code()) as u8) as char;
            Key::Character(letter.to_string())
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL: XMods = XMods {
        ctrl: true,
        ..XMods::NONE
    };

    #[test]
    fn pointer_events_land_on_the_scrolled_page() {
        let input = Input::Pointer {
            action: PointerAction::Down,
            x: 10.0,
            y: 20.0,
            button: MouseButton::Left,
            held: MouseEventButtons::Primary,
            mods: XMods::NONE,
        };
        let Some(UiEvent::PointerDown(e)) = to_ui_event(&input, (0.0, 300.0)) else {
            panic!("not a pointer down");
        };
        assert_eq!((e.client_x(), e.client_y()), (10.0, 20.0));
        assert_eq!((e.page_x(), e.page_y()), (10.0, 320.0));
        assert_eq!(e.button, MouseEventButton::Main);
    }

    #[test]
    fn rolling_the_wheel_toward_the_user_scrolls_down() {
        let input = Input::Wheel {
            notches: -1.0,
            horizontal: false,
            x: 0.0,
            y: 0.0,
            mods: XMods::NONE,
        };
        let Some(UiEvent::Wheel(e)) = to_ui_event(&input, (0.0, 0.0)) else {
            panic!("not a wheel");
        };
        assert!(matches!(e.delta, BlitzWheelDelta::Pixels(x, y) if x == 0.0 && y == -WHEEL_LINE));
    }

    #[test]
    fn letters_pass_only_as_shortcuts_and_chars_carry_text() {
        let key = |k, m| {
            to_ui_event(
                &Input::Key {
                    key: k,
                    mods: m,
                    repeat: false,
                },
                (0.0, 0.0),
            )
        };
        assert!(key(XKey::C, XMods::NONE).is_none());
        let Some(UiEvent::KeyDown(e)) = key(XKey::C, CTRL) else {
            panic!("no ctrl+c");
        };
        assert_eq!(e.key, Key::Character("c".into()));
        assert!(e.modifiers.contains(Modifiers::CONTROL));
        let Some(UiEvent::KeyDown(e)) = to_ui_event(&Input::Char('é'), (0.0, 0.0)) else {
            panic!("no char");
        };
        assert_eq!(e.text.as_deref(), Some("é"));
    }
}
