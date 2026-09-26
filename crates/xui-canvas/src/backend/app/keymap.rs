#![forbid(unsafe_code)]

//! Pure translations from `winit`'s input vocabulary to the portable one:
//! pointer shapes, mouse buttons and logical keys.

use winit::event::MouseButton as WinitButton;
use winit::keyboard::{Key as WinitKey, NamedKey};
use winit::window::CursorIcon;

use xui_core::backend::Cursor;
use xui_core::message::{Key, MouseButton};

/// The `winit` icon for a portable pointer shape.
pub(super) fn cursor_icon(cursor: Cursor) -> CursorIcon {
    match cursor {
        Cursor::Default => CursorIcon::Default,
        Cursor::Hand => CursorIcon::Pointer,
        Cursor::Text => CursorIcon::Text,
        Cursor::SizeHorizontal => CursorIcon::EwResize,
        Cursor::SizeVertical => CursorIcon::NsResize,
    }
}

pub(super) fn mouse_button(button: WinitButton) -> Option<MouseButton> {
    Some(match button {
        WinitButton::Left => MouseButton::Left,
        WinitButton::Right => MouseButton::Right,
        WinitButton::Middle => MouseButton::Middle,
        WinitButton::Back => MouseButton::X1,
        WinitButton::Forward => MouseButton::X2,
        _ => return None,
    })
}

/// Maps a `winit` logical key to the `VK_*` code the portable widgets use.
pub(super) fn virtual_key(key: &WinitKey) -> Option<Key> {
    Some(match key {
        WinitKey::Named(named) => match named {
            NamedKey::Enter => Key::RETURN,
            NamedKey::Space => Key::SPACE,
            NamedKey::Escape => Key::ESCAPE,
            NamedKey::Tab => Key::TAB,
            NamedKey::Backspace => Key::BACK,
            NamedKey::Delete => Key::DELETE,
            NamedKey::Insert => Key::INSERT,
            NamedKey::Home => Key::HOME,
            NamedKey::End => Key::END,
            NamedKey::PageUp => Key::PAGE_UP,
            NamedKey::PageDown => Key::PAGE_DOWN,
            NamedKey::ArrowLeft => Key::LEFT,
            NamedKey::ArrowRight => Key::RIGHT,
            NamedKey::ArrowUp => Key::UP,
            NamedKey::ArrowDown => Key::DOWN,
            NamedKey::F1 => Key::F1,
            NamedKey::F2 => Key::F2,
            NamedKey::F3 => Key::F3,
            NamedKey::F4 => Key::F4,
            _ => return None,
        },
        WinitKey::Character(text) => {
            let character = text.chars().next()?;
            if character.is_ascii_alphabetic() {
                Key::from_code(character.to_ascii_uppercase() as u16)
            } else if character.is_ascii_digit() {
                Key::from_code(character as u16)
            } else {
                return None;
            }
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_keys_map_to_their_vk_codes() {
        assert_eq!(
            virtual_key(&WinitKey::Named(NamedKey::Enter)),
            Some(Key::RETURN)
        );
        assert_eq!(
            virtual_key(&WinitKey::Named(NamedKey::ArrowLeft)),
            Some(Key::LEFT)
        );
        assert_eq!(
            virtual_key(&WinitKey::Named(NamedKey::Escape)),
            Some(Key::ESCAPE)
        );
        assert_eq!(virtual_key(&WinitKey::Named(NamedKey::F1)), Some(Key::F1));
    }

    #[test]
    fn character_keys_map_case_insensitively() {
        assert_eq!(virtual_key(&WinitKey::Character("a".into())), Some(Key::A));
        assert_eq!(
            virtual_key(&WinitKey::Character("7".into())),
            Some(Key::DIGIT7)
        );
        assert_eq!(virtual_key(&WinitKey::Character("-".into())), None);
    }

    #[test]
    fn the_resize_cursors_map_to_their_winit_icons() {
        assert_eq!(cursor_icon(Cursor::Hand), CursorIcon::Pointer);
        assert_eq!(cursor_icon(Cursor::SizeHorizontal), CursorIcon::EwResize);
        assert_eq!(cursor_icon(Cursor::SizeVertical), CursorIcon::NsResize);
    }

    #[test]
    fn only_the_mouse_buttons_the_core_models_are_mapped() {
        assert_eq!(mouse_button(WinitButton::Left), Some(MouseButton::Left));
        assert_eq!(mouse_button(WinitButton::Back), Some(MouseButton::X1));
        assert_eq!(mouse_button(WinitButton::Forward), Some(MouseButton::X2));
    }
}
