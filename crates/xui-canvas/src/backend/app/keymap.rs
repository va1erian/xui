#![forbid(unsafe_code)]

//! Pure translations from `winit`'s input vocabulary to the portable one:
//! pointer shapes, mouse buttons and logical keys.

use winit::event::MouseButton as WinitButton;
use winit::keyboard::{Key as WinitKey, NamedKey};
use winit::window::{CursorIcon, ResizeDirection};

use xui_core::backend::Cursor;
use xui_core::message::{Key, MouseButton};

/// The frame thickness, in device pixels, within which a borderless resizable
/// window treats a pointer as being on a resize edge.
pub(super) const RESIZE_BORDER_PX: i32 = 5;

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

/// The resize direction for a pointer at `(x, y)` in a `width`×`height` client
/// area, when it is within `border` device pixels of an edge.
///
/// Returns `None` in the interior, when `resizable` is false, when `border` is
/// not positive, or when an axis is too short for its two edges to be distinct.
pub(super) fn resize_direction(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    border: i32,
    resizable: bool,
) -> Option<ResizeDirection> {
    if !resizable || border <= 0 {
        return None;
    }
    let (width, height) = (width as i32, height as i32);
    let horizontal = width >= 2 * border;
    let vertical = height >= 2 * border;
    let west = horizontal && x < border;
    let east = horizontal && x >= width - border;
    let north = vertical && y < border;
    let south = vertical && y >= height - border;
    Some(match (north, south, west, east) {
        (true, _, true, _) => ResizeDirection::NorthWest,
        (true, _, _, true) => ResizeDirection::NorthEast,
        (_, true, true, _) => ResizeDirection::SouthWest,
        (_, true, _, true) => ResizeDirection::SouthEast,
        (true, _, _, _) => ResizeDirection::North,
        (_, true, _, _) => ResizeDirection::South,
        (_, _, true, _) => ResizeDirection::West,
        (_, _, _, true) => ResizeDirection::East,
        _ => return None,
    })
}

/// The pointer icon shown over the frame for a resize direction. The edges use
/// the bidirectional arrows a border draws; the corners use diagonals.
pub(super) fn resize_cursor(direction: ResizeDirection) -> CursorIcon {
    match direction {
        ResizeDirection::East | ResizeDirection::West => CursorIcon::EwResize,
        ResizeDirection::North | ResizeDirection::South => CursorIcon::NsResize,
        ResizeDirection::NorthEast | ResizeDirection::SouthWest => CursorIcon::NeswResize,
        ResizeDirection::NorthWest | ResizeDirection::SouthEast => CursorIcon::NwseResize,
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

    #[test]
    fn each_edge_of_the_frame_maps_to_a_resize_direction() {
        // A 100x80 window with a 4px frame, probed at the middle of each edge.
        assert_eq!(
            resize_direction(0, 40, 100, 80, 4, true),
            Some(ResizeDirection::West)
        );
        assert_eq!(
            resize_direction(99, 40, 100, 80, 4, true),
            Some(ResizeDirection::East)
        );
        assert_eq!(
            resize_direction(50, 0, 100, 80, 4, true),
            Some(ResizeDirection::North)
        );
        assert_eq!(
            resize_direction(50, 79, 100, 80, 4, true),
            Some(ResizeDirection::South)
        );
    }

    #[test]
    fn each_corner_of_the_frame_maps_to_a_diagonal_direction() {
        assert_eq!(
            resize_direction(0, 0, 100, 80, 4, true),
            Some(ResizeDirection::NorthWest)
        );
        assert_eq!(
            resize_direction(99, 0, 100, 80, 4, true),
            Some(ResizeDirection::NorthEast)
        );
        assert_eq!(
            resize_direction(0, 79, 100, 80, 4, true),
            Some(ResizeDirection::SouthWest)
        );
        assert_eq!(
            resize_direction(99, 79, 100, 80, 4, true),
            Some(ResizeDirection::SouthEast)
        );
    }

    #[test]
    fn the_interior_is_not_a_resize_edge() {
        assert_eq!(resize_direction(50, 40, 100, 80, 4, true), None);
        assert_eq!(resize_direction(4, 40, 100, 80, 4, true), None);
        assert_eq!(resize_direction(50, 4, 100, 80, 4, true), None);
    }

    #[test]
    fn a_non_resizable_window_has_no_resize_edges() {
        assert_eq!(resize_direction(0, 0, 100, 80, 4, false), None);
        assert_eq!(resize_direction(50, 40, 100, 80, 4, false), None);
    }

    #[test]
    fn a_window_too_small_for_two_edges_has_no_resize_edges() {
        // Narrower than twice the frame, so no horizontal edge is distinct.
        assert_eq!(resize_direction(0, 40, 6, 80, 4, true), None);
        assert_eq!(
            resize_direction(50, 0, 6, 80, 4, true),
            Some(ResizeDirection::North)
        );
    }

    #[test]
    fn the_resize_cursors_follow_the_direction() {
        assert_eq!(resize_cursor(ResizeDirection::East), CursorIcon::EwResize);
        assert_eq!(resize_cursor(ResizeDirection::West), CursorIcon::EwResize);
        assert_eq!(resize_cursor(ResizeDirection::North), CursorIcon::NsResize);
        assert_eq!(resize_cursor(ResizeDirection::South), CursorIcon::NsResize);
        assert_eq!(
            resize_cursor(ResizeDirection::NorthEast),
            CursorIcon::NeswResize
        );
        assert_eq!(
            resize_cursor(ResizeDirection::NorthWest),
            CursorIcon::NwseResize
        );
    }
}
