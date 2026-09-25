#![forbid(unsafe_code)]

//! Input vocabulary shared by the message enum: virtual keys, modifier state,
//! mouse buttons and hit-test results.

/// A virtual-key code (`VK_*`), the identifier carried by keyboard messages.
///
/// Named constants cover the common keys and [`Key::from_code`] accepts any raw
/// code. The type is small and `Hash`able so accelerators can use it as a table
/// key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key(u16);

impl Key {
    /// Wraps a raw virtual-key code.
    pub const fn from_code(code: u16) -> Key {
        Key(code)
    }

    /// The raw virtual-key code.
    pub const fn code(self) -> u16 {
        self.0
    }
}

/// Defines [`Key`]'s named constants.
///
/// The codes are the `VK_*` values from `Winuser.h`, mirrored as literals so
/// the core has no platform dependency; a backend decodes to the same values.
macro_rules! keys {
    ($($name:ident => $code:expr),+ $(,)?) => {
        impl Key {
            $(
                #[doc = concat!("The ", stringify!($name), " key.")]
                pub const $name: Key = Key($code);
            )+
        }
    };
}

keys! {
    BACK => 0x08,
    TAB => 0x09,
    RETURN => 0x0D,
    SHIFT => 0x10,
    CONTROL => 0x11,
    MENU => 0x12,
    CAPITAL => 0x14,
    ESCAPE => 0x1B,
    SPACE => 0x20,
    PAGE_UP => 0x21,
    PAGE_DOWN => 0x22,
    END => 0x23,
    HOME => 0x24,
    LEFT => 0x25,
    UP => 0x26,
    RIGHT => 0x27,
    DOWN => 0x28,
    INSERT => 0x2D,
    DELETE => 0x2E,
    F1 => 0x70,
    F2 => 0x71,
    F3 => 0x72,
    F4 => 0x73,
    F5 => 0x74,
    F6 => 0x75,
    F7 => 0x76,
    F8 => 0x77,
    F9 => 0x78,
    F10 => 0x79,
    F11 => 0x7A,
    F12 => 0x7B,
    A => 0x41,
    B => 0x42,
    C => 0x43,
    D => 0x44,
    E => 0x45,
    F => 0x46,
    G => 0x47,
    H => 0x48,
    I => 0x49,
    J => 0x4A,
    K => 0x4B,
    L => 0x4C,
    M => 0x4D,
    N => 0x4E,
    O => 0x4F,
    P => 0x50,
    Q => 0x51,
    R => 0x52,
    S => 0x53,
    T => 0x54,
    U => 0x55,
    V => 0x56,
    W => 0x57,
    X => 0x58,
    Y => 0x59,
    Z => 0x5A,
    DIGIT0 => 0x30,
    DIGIT1 => 0x31,
    DIGIT2 => 0x32,
    DIGIT3 => 0x33,
    DIGIT4 => 0x34,
    DIGIT5 => 0x35,
    DIGIT6 => 0x36,
    DIGIT7 => 0x37,
    DIGIT8 => 0x38,
    DIGIT9 => 0x39,
}

/// Which modifier keys were held when an input message was produced.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    /// Ctrl.
    pub ctrl: bool,
    /// Shift.
    pub shift: bool,
    /// Alt (`VK_MENU`).
    pub alt: bool,
    /// The Windows key (left or right).
    pub win: bool,
}

impl Modifiers {
    /// No modifiers held.
    pub const NONE: Modifiers = Modifiers {
        ctrl: false,
        shift: false,
        alt: false,
        win: false,
    };

    /// Whether no modifier is held.
    pub const fn is_empty(self) -> bool {
        !self.ctrl && !self.shift && !self.alt && !self.win
    }
}

/// A mouse button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    /// Left button.
    Left,
    /// Right button.
    Right,
    /// Middle (wheel) button.
    Middle,
    /// First extra (thumb) button.
    X1,
    /// Second extra (thumb) button.
    X2,
}

/// The result of hit-testing the mouse against a window (`WM_SETCURSOR`'s
/// `LOWORD(lparam)`). Codes come from `winuser.h` (`HT*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitTest {
    /// `HTNOWHERE`: not over any window.
    Nowhere,
    /// `HTCLIENT`: over the client area.
    Client,
    /// `HTCAPTION`: over the title bar.
    Caption,
    /// `HTSYSMENU`: over the system menu.
    SysMenu,
    /// `HTGROWBOX`: over the size box.
    GrowBox,
    /// `HTMENU`: over the menu.
    Menu,
    /// `HTHSCROLL`: over the horizontal scroll bar.
    HScroll,
    /// `HTVSCROLL`: over the vertical scroll bar.
    VScroll,
    /// `HTMINBUTTON`: over the minimise button.
    MinButton,
    /// `HTMAXBUTTON`: over the maximise button.
    MaxButton,
    /// `HTLEFT`: over the left border.
    Left,
    /// `HTRIGHT`: over the right border.
    Right,
    /// `HTTOP`: over the top border.
    Top,
    /// `HTTOPLEFT`: over the top-left corner.
    TopLeft,
    /// `HTTOPRIGHT`: over the top-right corner.
    TopRight,
    /// `HTBOTTOM`: over the bottom border.
    Bottom,
    /// `HTBOTTOMLEFT`: over the bottom-left corner.
    BottomLeft,
    /// `HTBOTTOMRIGHT`: over the bottom-right corner.
    BottomRight,
    /// `HTBORDER`: over an inactive border.
    Border,
    /// `HTOBJECT`: over an object.
    Object,
    /// `HTCLOSE`: over the close button.
    Close,
    /// `HTHELP`: over the help button.
    Help,
    /// Any other code, passed through unchanged (`HTERROR`, `HTTRANSPARENT`…).
    Other(u16),
}

impl HitTest {
    /// Maps a raw hit-test code.
    pub const fn from_code(code: u16) -> HitTest {
        match code {
            0 => HitTest::Nowhere,
            1 => HitTest::Client,
            2 => HitTest::Caption,
            3 => HitTest::SysMenu,
            4 => HitTest::GrowBox,
            5 => HitTest::Menu,
            6 => HitTest::HScroll,
            7 => HitTest::VScroll,
            8 => HitTest::MinButton,
            9 => HitTest::MaxButton,
            10 => HitTest::Left,
            11 => HitTest::Right,
            12 => HitTest::Top,
            13 => HitTest::TopLeft,
            14 => HitTest::TopRight,
            15 => HitTest::Bottom,
            16 => HitTest::BottomLeft,
            17 => HitTest::BottomRight,
            18 => HitTest::Border,
            19 => HitTest::Object,
            20 => HitTest::Close,
            21 => HitTest::Help,
            other => HitTest::Other(other),
        }
    }
}
