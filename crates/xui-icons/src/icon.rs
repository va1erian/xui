#![forbid(unsafe_code)]

//! The icon vocabulary, shared by every visual style.

use crate::shape::Shape;

/// The groups the set is organised into.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    /// Computer hardware and peripherals.
    Hardware,
    /// Folders and file types.
    Files,
    /// Everyday operating-system icons.
    System,
    /// Concepts specific to the xui toolkit.
    Toolkit,
}

/// One icon of the set. Which artwork it draws is chosen at compile time; see
/// [`STYLE`](crate::STYLE).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    /// The `computer` icon.
    Computer,
    /// The `monitor` icon.
    Monitor,
    /// The `keyboard` icon.
    Keyboard,
    /// The `mouse` icon.
    Mouse,
    /// The `hard-disk` icon.
    HardDisk,
    /// The `floppy` icon.
    Floppy,
    /// The `cd-rom` icon.
    CdRom,
    /// The `printer` icon.
    Printer,
    /// The `modem` icon.
    Modem,
    /// The `network` icon.
    Network,
    /// The `server` icon.
    Server,
    /// The `speaker` icon.
    Speaker,
    /// The `folder` icon.
    Folder,
    /// The `folder-open` icon.
    FolderOpen,
    /// The `document` icon.
    Document,
    /// The `image` icon.
    Image,
    /// The `music` icon.
    Music,
    /// The `archive` icon.
    Archive,
    /// The `trash` icon.
    Trash,
    /// The `trash-full` icon.
    TrashFull,
    /// The `settings` icon.
    Settings,
    /// The `search` icon.
    Search,
    /// The `home` icon.
    Home,
    /// The `clock` icon.
    Clock,
    /// The `mail` icon.
    Mail,
    /// The `lock` icon.
    Lock,
    /// The `help` icon.
    Help,
    /// The `warning` icon.
    Warning,
    /// The `info` icon.
    Info,
    /// The `error` icon.
    Error,
    /// The `rpc` icon.
    Rpc,
    /// The `pub-sub` icon.
    PubSub,
    /// The `theme` icon.
    Theme,
    /// The `widget` icon.
    Widget,
    /// The `terminal` icon.
    Terminal,
    /// The `window` icon.
    Window,
}

impl Icon {
    /// Every icon, in gallery order.
    pub const ALL: &'static [Icon] = &[
        Icon::Computer,
        Icon::Monitor,
        Icon::Keyboard,
        Icon::Mouse,
        Icon::HardDisk,
        Icon::Floppy,
        Icon::CdRom,
        Icon::Printer,
        Icon::Modem,
        Icon::Network,
        Icon::Server,
        Icon::Speaker,
        Icon::Folder,
        Icon::FolderOpen,
        Icon::Document,
        Icon::Image,
        Icon::Music,
        Icon::Archive,
        Icon::Trash,
        Icon::TrashFull,
        Icon::Settings,
        Icon::Search,
        Icon::Home,
        Icon::Clock,
        Icon::Mail,
        Icon::Lock,
        Icon::Help,
        Icon::Warning,
        Icon::Info,
        Icon::Error,
        Icon::Rpc,
        Icon::PubSub,
        Icon::Theme,
        Icon::Widget,
        Icon::Terminal,
        Icon::Window,
    ];

    /// The icon's file stem: `"hard-disk"`, `"pub-sub"`.
    pub fn name(self) -> &'static str {
        match self {
            Icon::Computer => "computer",
            Icon::Monitor => "monitor",
            Icon::Keyboard => "keyboard",
            Icon::Mouse => "mouse",
            Icon::HardDisk => "hard-disk",
            Icon::Floppy => "floppy",
            Icon::CdRom => "cd-rom",
            Icon::Printer => "printer",
            Icon::Modem => "modem",
            Icon::Network => "network",
            Icon::Server => "server",
            Icon::Speaker => "speaker",
            Icon::Folder => "folder",
            Icon::FolderOpen => "folder-open",
            Icon::Document => "document",
            Icon::Image => "image",
            Icon::Music => "music",
            Icon::Archive => "archive",
            Icon::Trash => "trash",
            Icon::TrashFull => "trash-full",
            Icon::Settings => "settings",
            Icon::Search => "search",
            Icon::Home => "home",
            Icon::Clock => "clock",
            Icon::Mail => "mail",
            Icon::Lock => "lock",
            Icon::Help => "help",
            Icon::Warning => "warning",
            Icon::Info => "info",
            Icon::Error => "error",
            Icon::Rpc => "rpc",
            Icon::PubSub => "pub-sub",
            Icon::Theme => "theme",
            Icon::Widget => "widget",
            Icon::Terminal => "terminal",
            Icon::Window => "window",
        }
    }

    /// The group the icon belongs to.
    pub fn category(self) -> Category {
        match self {
            Icon::Computer => Category::Hardware,
            Icon::Monitor => Category::Hardware,
            Icon::Keyboard => Category::Hardware,
            Icon::Mouse => Category::Hardware,
            Icon::HardDisk => Category::Hardware,
            Icon::Floppy => Category::Hardware,
            Icon::CdRom => Category::Hardware,
            Icon::Printer => Category::Hardware,
            Icon::Modem => Category::Hardware,
            Icon::Network => Category::Hardware,
            Icon::Server => Category::Hardware,
            Icon::Speaker => Category::Hardware,
            Icon::Folder => Category::Files,
            Icon::FolderOpen => Category::Files,
            Icon::Document => Category::Files,
            Icon::Image => Category::Files,
            Icon::Music => Category::Files,
            Icon::Archive => Category::Files,
            Icon::Trash => Category::System,
            Icon::TrashFull => Category::System,
            Icon::Settings => Category::System,
            Icon::Search => Category::System,
            Icon::Home => Category::System,
            Icon::Clock => Category::System,
            Icon::Mail => Category::System,
            Icon::Lock => Category::System,
            Icon::Help => Category::System,
            Icon::Warning => Category::System,
            Icon::Info => Category::System,
            Icon::Error => Category::System,
            Icon::Rpc => Category::Toolkit,
            Icon::PubSub => Category::Toolkit,
            Icon::Theme => Category::Toolkit,
            Icon::Widget => Category::Toolkit,
            Icon::Terminal => Category::Toolkit,
            Icon::Window => Category::Toolkit,
        }
    }

    /// The icon's shapes, back to front, on a 32-unit grid.
    pub fn shapes(self) -> &'static [Shape] {
        crate::style::shapes(self)
    }
}
