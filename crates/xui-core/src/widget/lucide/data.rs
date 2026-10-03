// Generated from the vendored Lucide SVGs in `assets/lucide/` (see
// `assets/lucide/README.md`) by `assets/lucide/generate.py`. Do not edit.

use crate::backend::PathSeg;

#[path = "data_a.rs"]
mod data_a;
pub(crate) use data_a::*;

#[path = "data_b.rs"]
mod data_b;
pub(crate) use data_b::*;

#[path = "data_c.rs"]
mod data_c;
pub(crate) use data_c::*;

#[path = "data_d.rs"]
mod data_d;
pub(crate) use data_d::*;

#[path = "data_e.rs"]
mod data_e;
pub(crate) use data_e::*;

#[path = "data_f.rs"]
mod data_f;
pub(crate) use data_f::*;

#[path = "data_g.rs"]
mod data_g;
pub(crate) use data_g::*;

#[path = "data_h.rs"]
mod data_h;
pub(crate) use data_h::*;

#[path = "data_i.rs"]
mod data_i;
pub(crate) use data_i::*;

#[path = "data_l.rs"]
mod data_l;
pub(crate) use data_l::*;

#[path = "data_m.rs"]
mod data_m;
pub(crate) use data_m::*;

#[path = "data_p.rs"]
mod data_p;
pub(crate) use data_p::*;

#[path = "data_r.rs"]
mod data_r;
pub(crate) use data_r::*;

#[path = "data_s.rs"]
mod data_s;
pub(crate) use data_s::*;

#[path = "data_t.rs"]
mod data_t;
pub(crate) use data_t::*;

#[path = "data_u.rs"]
mod data_u;
pub(crate) use data_u::*;

#[path = "data_v.rs"]
mod data_v;
pub(crate) use data_v::*;

#[path = "data_w.rs"]
mod data_w;
pub(crate) use data_w::*;

#[path = "data_x.rs"]
mod data_x;
pub(crate) use data_x::*;

#[path = "data_z.rs"]
mod data_z;
pub(crate) use data_z::*;

/// One of the vendored [Lucide](https://lucide.dev) outline icons.
///
/// Adding an icon is dropping its SVG into `assets/lucide/` and re-running
/// `assets/lucide/generate.py`; the variant is generated from the file name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lucide {
    /// The `app-window` outline.
    AppWindow,
    /// The `arrow-down-a-z` outline.
    ArrowDownAZ,
    /// The `bold` outline.
    Bold,
    /// The `book-open` outline.
    BookOpen,
    /// The `box` outline.
    Box,
    /// The `bug` outline.
    Bug,
    /// The `check` outline.
    Check,
    /// The `chevron-down` outline.
    ChevronDown,
    /// The `chevron-left` outline.
    ChevronLeft,
    /// The `chevron-right` outline.
    ChevronRight,
    /// The `chevron-up` outline.
    ChevronUp,
    /// The `chevrons-up-down` outline.
    ChevronsUpDown,
    /// The `circle-dot` outline.
    CircleDot,
    /// The `circle-help` outline.
    CircleHelp,
    /// The `circle-x` outline.
    CircleX,
    /// The `clipboard-paste` outline.
    ClipboardPaste,
    /// The `code` outline.
    Code,
    /// The `copy` outline.
    Copy,
    /// The `disc` outline.
    Disc,
    /// The `download` outline.
    Download,
    /// The `ellipsis` outline.
    Ellipsis,
    /// The `external-link` outline.
    ExternalLink,
    /// The `eye-off` outline.
    EyeOff,
    /// The `eye` outline.
    Eye,
    /// The `file-code` outline.
    FileCode,
    /// The `file-plus` outline.
    FilePlus,
    /// The `file-text` outline.
    FileText,
    /// The `file` outline.
    File,
    /// The `folder-open` outline.
    FolderOpen,
    /// The `folder` outline.
    Folder,
    /// The `group` outline.
    Group,
    /// The `heading-1` outline.
    Heading1,
    /// The `heading-2` outline.
    Heading2,
    /// The `heading-3` outline.
    Heading3,
    /// The `history` outline.
    History,
    /// The `home` outline.
    Home,
    /// The `image` outline.
    Image,
    /// The `indent-decrease` outline.
    IndentDecrease,
    /// The `indent-increase` outline.
    IndentIncrease,
    /// The `info` outline.
    Info,
    /// The `italic` outline.
    Italic,
    /// The `layout-grid` outline.
    LayoutGrid,
    /// The `link` outline.
    Link,
    /// The `list-ordered` outline.
    ListOrdered,
    /// The `list-tree` outline.
    ListTree,
    /// The `list` outline.
    List,
    /// The `lock` outline.
    Lock,
    /// The `log-out` outline.
    LogOut,
    /// The `menu` outline.
    Menu,
    /// The `minus` outline.
    Minus,
    /// The `monitor` outline.
    Monitor,
    /// The `mouse-pointer-2` outline.
    MousePointer2,
    /// The `package` outline.
    Package,
    /// The `pause` outline.
    Pause,
    /// The `pencil` outline.
    Pencil,
    /// The `play` outline.
    Play,
    /// The `plus` outline.
    Plus,
    /// The `rectangle-horizontal` outline.
    RectangleHorizontal,
    /// The `redo-2` outline.
    Redo2,
    /// The `refresh-cw` outline.
    RefreshCw,
    /// The `repeat` outline.
    Repeat,
    /// The `ruler` outline.
    Ruler,
    /// The `save-all` outline.
    SaveAll,
    /// The `save` outline.
    Save,
    /// The `scissors` outline.
    Scissors,
    /// The `search` outline.
    Search,
    /// The `separator-horizontal` outline.
    SeparatorHorizontal,
    /// The `settings` outline.
    Settings,
    /// The `shuffle` outline.
    Shuffle,
    /// The `skip-back` outline.
    SkipBack,
    /// The `skip-forward` outline.
    SkipForward,
    /// The `square-check` outline.
    SquareCheck,
    /// The `square-mouse-pointer` outline.
    SquareMousePointer,
    /// The `square` outline.
    Square,
    /// The `star` outline.
    Star,
    /// The `strikethrough` outline.
    Strikethrough,
    /// The `tag` outline.
    Tag,
    /// The `terminal` outline.
    Terminal,
    /// The `text-align-center` outline.
    TextAlignCenter,
    /// The `text-align-end` outline.
    TextAlignEnd,
    /// The `text-align-justify` outline.
    TextAlignJustify,
    /// The `text-align-start` outline.
    TextAlignStart,
    /// The `text-cursor-input` outline.
    TextCursorInput,
    /// The `text-quote` outline.
    TextQuote,
    /// The `trash-2` outline.
    Trash2,
    /// The `triangle-alert` outline.
    TriangleAlert,
    /// The `type` outline.
    Type,
    /// The `underline` outline.
    Underline,
    /// The `undo-2` outline.
    Undo2,
    /// The `unlock` outline.
    Unlock,
    /// The `upload` outline.
    Upload,
    /// The `users` outline.
    Users,
    /// The `volume-2` outline.
    Volume2,
    /// The `wrap-text` outline.
    WrapText,
    /// The `x` outline.
    X,
    /// The `zap` outline.
    Zap,
}

impl Lucide {
    /// Every vendored icon, in file-name order.
    pub const ALL: &'static [Lucide] = &[
        Lucide::AppWindow,
        Lucide::ArrowDownAZ,
        Lucide::Bold,
        Lucide::BookOpen,
        Lucide::Box,
        Lucide::Bug,
        Lucide::Check,
        Lucide::ChevronDown,
        Lucide::ChevronLeft,
        Lucide::ChevronRight,
        Lucide::ChevronUp,
        Lucide::ChevronsUpDown,
        Lucide::CircleDot,
        Lucide::CircleHelp,
        Lucide::CircleX,
        Lucide::ClipboardPaste,
        Lucide::Code,
        Lucide::Copy,
        Lucide::Disc,
        Lucide::Download,
        Lucide::Ellipsis,
        Lucide::ExternalLink,
        Lucide::EyeOff,
        Lucide::Eye,
        Lucide::FileCode,
        Lucide::FilePlus,
        Lucide::FileText,
        Lucide::File,
        Lucide::FolderOpen,
        Lucide::Folder,
        Lucide::Group,
        Lucide::Heading1,
        Lucide::Heading2,
        Lucide::Heading3,
        Lucide::History,
        Lucide::Home,
        Lucide::Image,
        Lucide::IndentDecrease,
        Lucide::IndentIncrease,
        Lucide::Info,
        Lucide::Italic,
        Lucide::LayoutGrid,
        Lucide::Link,
        Lucide::ListOrdered,
        Lucide::ListTree,
        Lucide::List,
        Lucide::Lock,
        Lucide::LogOut,
        Lucide::Menu,
        Lucide::Minus,
        Lucide::Monitor,
        Lucide::MousePointer2,
        Lucide::Package,
        Lucide::Pause,
        Lucide::Pencil,
        Lucide::Play,
        Lucide::Plus,
        Lucide::RectangleHorizontal,
        Lucide::Redo2,
        Lucide::RefreshCw,
        Lucide::Repeat,
        Lucide::Ruler,
        Lucide::SaveAll,
        Lucide::Save,
        Lucide::Scissors,
        Lucide::Search,
        Lucide::SeparatorHorizontal,
        Lucide::Settings,
        Lucide::Shuffle,
        Lucide::SkipBack,
        Lucide::SkipForward,
        Lucide::SquareCheck,
        Lucide::SquareMousePointer,
        Lucide::Square,
        Lucide::Star,
        Lucide::Strikethrough,
        Lucide::Tag,
        Lucide::Terminal,
        Lucide::TextAlignCenter,
        Lucide::TextAlignEnd,
        Lucide::TextAlignJustify,
        Lucide::TextAlignStart,
        Lucide::TextCursorInput,
        Lucide::TextQuote,
        Lucide::Trash2,
        Lucide::TriangleAlert,
        Lucide::Type,
        Lucide::Underline,
        Lucide::Undo2,
        Lucide::Unlock,
        Lucide::Upload,
        Lucide::Users,
        Lucide::Volume2,
        Lucide::WrapText,
        Lucide::X,
        Lucide::Zap,
    ];
}

/// The path segments that draw `icon` on the 24x24 Lucide grid.
pub(crate) fn path(icon: Lucide) -> &'static [PathSeg] {
    match icon {
        Lucide::AppWindow => APP_WINDOW,
        Lucide::ArrowDownAZ => ARROW_DOWN_A_Z,
        Lucide::Bold => BOLD,
        Lucide::BookOpen => BOOK_OPEN,
        Lucide::Box => BOX,
        Lucide::Bug => BUG,
        Lucide::Check => CHECK,
        Lucide::ChevronDown => CHEVRON_DOWN,
        Lucide::ChevronLeft => CHEVRON_LEFT,
        Lucide::ChevronRight => CHEVRON_RIGHT,
        Lucide::ChevronUp => CHEVRON_UP,
        Lucide::ChevronsUpDown => CHEVRONS_UP_DOWN,
        Lucide::CircleDot => CIRCLE_DOT,
        Lucide::CircleHelp => CIRCLE_HELP,
        Lucide::CircleX => CIRCLE_X,
        Lucide::ClipboardPaste => CLIPBOARD_PASTE,
        Lucide::Code => CODE,
        Lucide::Copy => COPY,
        Lucide::Disc => DISC,
        Lucide::Download => DOWNLOAD,
        Lucide::Ellipsis => ELLIPSIS,
        Lucide::ExternalLink => EXTERNAL_LINK,
        Lucide::EyeOff => EYE_OFF,
        Lucide::Eye => EYE,
        Lucide::FileCode => FILE_CODE,
        Lucide::FilePlus => FILE_PLUS,
        Lucide::FileText => FILE_TEXT,
        Lucide::File => FILE,
        Lucide::FolderOpen => FOLDER_OPEN,
        Lucide::Folder => FOLDER,
        Lucide::Group => GROUP,
        Lucide::Heading1 => HEADING_1,
        Lucide::Heading2 => HEADING_2,
        Lucide::Heading3 => HEADING_3,
        Lucide::History => HISTORY,
        Lucide::Home => HOME,
        Lucide::Image => IMAGE,
        Lucide::IndentDecrease => INDENT_DECREASE,
        Lucide::IndentIncrease => INDENT_INCREASE,
        Lucide::Info => INFO,
        Lucide::Italic => ITALIC,
        Lucide::LayoutGrid => LAYOUT_GRID,
        Lucide::Link => LINK,
        Lucide::ListOrdered => LIST_ORDERED,
        Lucide::ListTree => LIST_TREE,
        Lucide::List => LIST,
        Lucide::Lock => LOCK,
        Lucide::LogOut => LOG_OUT,
        Lucide::Menu => MENU,
        Lucide::Minus => MINUS,
        Lucide::Monitor => MONITOR,
        Lucide::MousePointer2 => MOUSE_POINTER_2,
        Lucide::Package => PACKAGE,
        Lucide::Pause => PAUSE,
        Lucide::Pencil => PENCIL,
        Lucide::Play => PLAY,
        Lucide::Plus => PLUS,
        Lucide::RectangleHorizontal => RECTANGLE_HORIZONTAL,
        Lucide::Redo2 => REDO_2,
        Lucide::RefreshCw => REFRESH_CW,
        Lucide::Repeat => REPEAT,
        Lucide::Ruler => RULER,
        Lucide::SaveAll => SAVE_ALL,
        Lucide::Save => SAVE,
        Lucide::Scissors => SCISSORS,
        Lucide::Search => SEARCH,
        Lucide::SeparatorHorizontal => SEPARATOR_HORIZONTAL,
        Lucide::Settings => SETTINGS,
        Lucide::Shuffle => SHUFFLE,
        Lucide::SkipBack => SKIP_BACK,
        Lucide::SkipForward => SKIP_FORWARD,
        Lucide::SquareCheck => SQUARE_CHECK,
        Lucide::SquareMousePointer => SQUARE_MOUSE_POINTER,
        Lucide::Square => SQUARE,
        Lucide::Star => STAR,
        Lucide::Strikethrough => STRIKETHROUGH,
        Lucide::Tag => TAG,
        Lucide::Terminal => TERMINAL,
        Lucide::TextAlignCenter => TEXT_ALIGN_CENTER,
        Lucide::TextAlignEnd => TEXT_ALIGN_END,
        Lucide::TextAlignJustify => TEXT_ALIGN_JUSTIFY,
        Lucide::TextAlignStart => TEXT_ALIGN_START,
        Lucide::TextCursorInput => TEXT_CURSOR_INPUT,
        Lucide::TextQuote => TEXT_QUOTE,
        Lucide::Trash2 => TRASH_2,
        Lucide::TriangleAlert => TRIANGLE_ALERT,
        Lucide::Type => TYPE,
        Lucide::Underline => UNDERLINE,
        Lucide::Undo2 => UNDO_2,
        Lucide::Unlock => UNLOCK,
        Lucide::Upload => UPLOAD,
        Lucide::Users => USERS,
        Lucide::Volume2 => VOLUME_2,
        Lucide::WrapText => WRAP_TEXT,
        Lucide::X => X,
        Lucide::Zap => ZAP,
    }
}
