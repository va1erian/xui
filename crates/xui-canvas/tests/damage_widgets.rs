//! Damage-driven repainting with the real widgets: scripted sessions (the
//! pointer sweeping the window, clicks, typing, focus moving between fields,
//! the wheel, theme changes, values changed from code) are run twice, once
//! repainting from damage only and once repainting the whole window before
//! every frame, and the frames must be identical pixel for pixel.
//!
//! The windows are small and the sessions short so each test stays under a few
//! seconds in a debug build; the themes and scales are what vary: the flat
//! light and dark themes, Midnight (whose rounded cards, gradients and accent
//! glows are what overdraw and clip masks look like in practice) and
//! 120, 144 and 192 DPI (where a ring drawn in design units is not a whole
//! number of pixels).

use std::cell::RefCell;
use std::rc::Rc;

use xui_canvas::OffscreenBackend;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, Event, PlatformSpec, WidgetId, WindowId};
use xui_core::message::{Key, Modifiers, MouseButton};
use xui_core::prelude::*;
use xui_core::widget::Glyph;
use xui_core::{Dip, Theme};

struct Demo;

impl App for Demo {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

/// How a session is set up.
#[derive(Clone, Copy)]
struct Setup {
    theme: fn() -> Theme,
    dpi: u32,
    /// How many columns and rows the pointer visits in the gallery sweep.
    grid: (i32, i32),
    /// The window's size in design units.
    size: (f32, f32),
}

/// What a script drives: the window, with the frames it has taken so far.
struct Driver<'a> {
    backend: &'a OffscreenBackend,
    ui: &'a Ui<()>,
    window: WindowId,
    reference: bool,
    frames: Vec<Vec<u8>>,
    /// The window's size in pixels.
    size: (i32, i32),
    /// The sweep grid of the setup.
    grid: (i32, i32),
}

impl Driver<'_> {
    /// Takes a frame; the reference repaints the whole window first.
    fn snap(&mut self) {
        self.backend.pump(self.window);
        if self.reference {
            self.backend.invalidate_window(self.window);
        }
        let image = self.backend.render(self.window).expect("a frame");
        self.frames.push(image.pixels);
    }

    fn event(&mut self, event: Event) {
        self.backend.inject(self.window, event);
        self.snap();
    }

    fn move_to(&mut self, x: i32, y: i32) {
        self.event(Event::MouseMove {
            x,
            y,
            modifiers: Modifiers::NONE,
        });
    }

    fn click(&mut self, x: i32, y: i32) {
        for event in [
            Event::MouseDown {
                x,
                y,
                button: MouseButton::Left,
                modifiers: Modifiers::NONE,
            },
            Event::MouseUp {
                x,
                y,
                button: MouseButton::Left,
                modifiers: Modifiers::NONE,
            },
        ] {
            self.backend.inject(self.window, event);
            self.backend.pump(self.window);
        }
        self.snap();
    }

    fn key(&mut self, key: Key) {
        self.event(Event::KeyDown {
            key,
            modifiers: Modifiers::NONE,
            repeat: 1,
            system: false,
        });
    }

    fn wheel(&mut self, x: i32, y: i32, delta: i16) {
        self.event(Event::MouseWheel {
            delta,
            horizontal: false,
            x,
            y,
            modifiers: Modifiers::NONE,
        });
    }

    /// The centre of a node, in window pixels.
    fn centre(&self, id: WidgetId) -> (i32, i32) {
        let bounds = self.ui.bounds(id);
        (
            (bounds.left + bounds.right) / 2,
            (bounds.top + bounds.bottom) / 2,
        )
    }
}

/// The window of a session and the handles its layout gave the script.
type Launched<T> = (Ui<()>, T);

/// Runs `script` on a window that holds `layout`, and returns the frame after
/// every step. With `reference`, the whole window is marked stale before each
/// frame.
fn session<T: 'static>(
    setup: Setup,
    reference: bool,
    layout: impl FnOnce() -> (Layout<()>, T) + 'static,
    script: impl FnOnce(&mut Driver<'_>, T) + 'static,
) -> Vec<Vec<u8>> {
    let backend = Rc::new(OffscreenBackend::with_dpi(setup.dpi));
    let frames: Rc<RefCell<Vec<Vec<u8>>>> = Rc::new(RefCell::new(Vec::new()));
    let slot: Rc<RefCell<Option<Launched<T>>>> = Rc::new(RefCell::new(None));

    let weak = Rc::downgrade(&backend);
    let hook_frames = Rc::clone(&frames);
    let hook_slot = Rc::clone(&slot);
    backend.set_run_hook(move || {
        let (Some(backend), Some((ui, handles))) = (weak.upgrade(), hook_slot.borrow_mut().take())
        else {
            return;
        };
        let window = ui.window();
        let client = backend.client_rect(window);
        let mut driver = Driver {
            backend: &backend,
            ui: &ui,
            window,
            reference,
            frames: Vec::new(),
            size: (client.width(), client.height()),
            grid: setup.grid,
        };
        driver.snap();
        script(&mut driver, handles);
        *hook_frames.borrow_mut() = driver.frames;
    });

    let make_slot = Rc::clone(&slot);
    let run: Rc<dyn Backend> = backend.clone();
    run_app(
        run,
        PlatformSpec::new("damage widgets").size(Dip(setup.size.0), Dip(setup.size.1)),
        move |ui| {
            ui.set_theme((setup.theme)());
            let (layout, handles) = layout();
            ui.root(layout).expect("the layout mounts");
            *make_slot.borrow_mut() = Some((ui.clone(), handles));
            Demo
        },
    )
    .expect("the session ran");
    slot.borrow_mut().take();
    frames.take()
}

/// Runs the same script with damage and as a reference, and asserts every
/// frame is equal; returns how many distinct frames there were.
fn assert_identical<T: 'static>(
    name: &str,
    setup: Setup,
    layout: impl Fn() -> (Layout<()>, T) + Clone + 'static,
    script: impl Fn(&mut Driver<'_>, T) + Clone + 'static,
) -> usize {
    let region = session(setup, false, layout.clone(), script.clone());
    let full = session(setup, true, layout, script);
    assert_eq!(region.len(), full.len(), "{name}: the same steps ran");
    let width = (setup.size.0 * setup.dpi as f32 / 96.0) as usize;
    for (step, (got, want)) in region.iter().zip(&full).enumerate() {
        if got != want {
            let at = got.iter().zip(want).position(|(a, b)| a != b).unwrap() / 4;
            panic!(
                "{name}: step {step} differs from a full repaint at ({}, {})",
                at % width.max(1),
                at / width.max(1)
            );
        }
    }
    let mut distinct: Vec<&Vec<u8>> = Vec::new();
    for frame in &region {
        if !distinct.contains(&frame) {
            distinct.push(frame);
        }
    }
    distinct.len()
}

fn light() -> Theme {
    Theme::light()
}

fn dark() -> Theme {
    Theme::dark()
}

fn midnight() -> Theme {
    Theme::midnight()
}

/// What the gallery script changes from code.
type GalleryHandles = (Handle<ProgressBar<()>>, Handle<CheckBox<()>>);

/// The widgets of the gallery sessions: a spread of every kind that paints
/// differently on hover, press, focus and selection.
fn gallery() -> (Layout<()>, GalleryHandles) {
    let progress_handle = Handle::new();
    let check = Handle::new();
    let layout = column().padding(10).gap(6).children((
        label("Damage differential").title(),
        row().gap(8).children((
            column().gap(6).children((
                edit().text("type here"),
                checkbox("Enabled"),
                toggle_button("Bold"),
                radio_group(&["Small", "Medium"]),
                combo_box(&["Alpha", "Beta", "Gamma"]),
                slider(0.0, 100.0),
                progress(100).bind(&progress_handle).height(8),
                row()
                    .gap(6)
                    .children((button("OK").primary(), button("Cancel"))),
            )),
            column().gap(6).children((
                list().height(56),
                panel(column().padding(8).children((
                    label("In a panel"),
                    button("Inside"),
                    checkbox("Driven from code").bind(&check),
                )))
                .height(96),
                tabs()
                    .page("General", column().child(label("General page")))
                    .page("More", column().child(button("More")))
                    .height(70),
                scroll(column().children((
                    label("Row 1").fixed(30),
                    label("Row 2").fixed(30),
                    label("Row 3").fixed(30),
                    label("Row 4").fixed(30),
                )))
                .height(70),
            )),
        )),
        top_bar().then(|bar| {
            bar.icon(TopBarId::new(1), Glyph::Menu)
                .toggle(TopBarId::new(2), Glyph::Star)
                .label(TopBarId::new(3), "bar")
        }),
        status_bar(&["Ready", ""]),
    ));
    (layout, (progress_handle, check))
}

/// The gallery's script: the pointer sweeps the window with a click now and
/// then, typing, keys, values set from code, the wheel, and theme changes.
fn gallery_script(driver: &mut Driver<'_>, (progress, check): GalleryHandles) {
    let (w, h) = driver.size;
    let (step_x, step_y) = ((w / driver.grid.0).max(20), (h / driver.grid.1).max(20));
    let mut count = 0;
    for y in (step_y / 2..h).step_by(step_y as usize) {
        for x in (step_x / 3..w).step_by(step_x as usize) {
            driver.move_to(x, y);
            count += 1;
            if count % 9 == 4 {
                driver.click(x, y);
            }
        }
    }
    // Typing into the edit, after focusing it.
    driver.click(w / 8, h / 6);
    for ch in "dmg".chars() {
        driver.event(Event::Char(ch));
    }
    for key in [Key::LEFT, Key::HOME, Key::END] {
        driver.key(key);
    }
    for value in [10, 100] {
        progress.get().set_value(value);
        driver.snap();
    }
    check.get().set_checked(true);
    driver.snap();
    for delta in [-3, 3] {
        driver.wheel(w * 3 / 4, h * 3 / 4, delta);
    }
    for next in [midnight, dark, light] {
        driver.ui.set_theme(next());
        driver.snap();
        driver.move_to(w / 3, h / 3);
    }
}

fn gallery_session(name: &str, setup: Setup) {
    let distinct = assert_identical(name, setup, gallery, gallery_script);
    assert!(
        distinct >= 10,
        "{name}: the session changed the window ({distinct} distinct frames)"
    );
}

#[test]
fn a_widget_session_repaints_identically_on_the_light_theme() {
    gallery_session(
        "light 96",
        Setup {
            theme: light,
            dpi: 96,
            grid: (6, 8),
            size: (520.0, 400.0),
        },
    );
}

#[test]
fn a_widget_session_repaints_identically_on_the_dark_theme_at_120_dpi() {
    gallery_session(
        "dark 120",
        Setup {
            theme: dark,
            dpi: 120,
            grid: (6, 8),
            size: (440.0, 340.0),
        },
    );
}

#[test]
fn a_widget_session_repaints_identically_on_midnight_at_144_dpi() {
    gallery_session(
        "midnight 144",
        Setup {
            theme: midnight,
            dpi: 144,
            grid: (4, 5),
            size: (320.0, 240.0),
        },
    );
}

#[test]
fn a_widget_session_repaints_identically_on_midnight_at_192_dpi() {
    gallery_session(
        "midnight 192",
        Setup {
            theme: midnight,
            dpi: 192,
            grid: (4, 4),
            size: (280.0, 210.0),
        },
    );
}

const SMALL: Setup = Setup {
    theme: light,
    dpi: 96,
    grid: (6, 8),
    size: (300.0, 160.0),
};

/// The two fields and the first button of [`fields`].
type FieldHandles = (Handle<Edit<()>>, Handle<Edit<()>>, Handle<Button<()>>);

/// Two fields and a button, for the focus and hover sessions.
fn fields() -> (Layout<()>, FieldHandles) {
    let (first, second, button_handle) = (Handle::new(), Handle::new(), Handle::new());
    let layout = column().padding(16).gap(8).children((
        edit().text("first").bind(&first),
        edit().text("second").bind(&second),
        row().gap(6).children((
            button("One").bind(&button_handle),
            button("Two"),
            number_field(0.0, 100.0, 5.0),
        )),
    ));
    (layout, (first, second, button_handle))
}

/// Focus moves between two fields by click, by `Backend::focus` (what Tab
/// does), by a key and by the window system's focus events; each field that
/// loses it must stop showing its ring and caret.
#[test]
fn moving_the_focus_between_fields_repaints_both() {
    for theme in [light as fn() -> Theme, midnight] {
        let setup = Setup { theme, ..SMALL };
        let distinct = assert_identical("focus", setup, fields, |driver, (first, second, one)| {
            let (first, second, one) = (first.get().id(), second.get().id(), one.get().id());
            let at = driver.centre(first);
            driver.click(at.0, at.1);
            let at = driver.centre(second);
            driver.click(at.0, at.1);
            driver.key(Key::TAB);
            driver.event(Event::Char('x'));
            let at = driver.centre(first);
            driver.click(at.0, at.1);
            // What Tab navigation does: ask the backend for the focus.
            driver.backend.focus(second);
            driver.snap();
            driver.backend.focus(one);
            driver.snap();
            driver.backend.focus(first);
            driver.snap();
            driver.event(Event::KillFocus);
            driver.event(Event::SetFocus);
            let at = driver.centre(one);
            driver.click(at.0, at.1);
        });
        assert!(distinct >= 6, "the focus moves were visible: {distinct}");
    }
}

/// A field that loses focus is repainted: the pixels of the unfocused field
/// are those of a window that never focused it.
#[test]
fn a_field_that_loses_the_focus_looks_unfocused() {
    let never = session(SMALL, false, fields, |driver, _| driver.snap());
    let after = session(SMALL, false, fields, |driver, (first, second, _)| {
        let at = driver.centre(first.get().id());
        driver.click(at.0, at.1);
        driver.snap();
        let at = driver.centre(second.get().id());
        driver.click(at.0, at.1);
    });
    // The first field's rows, outside the second field.
    let width = (SMALL.size.0) as usize;
    let first_rows = 16..16 + 28;
    let at_rest = &never[0];
    let last = after.last().expect("a frame");
    for row in first_rows {
        let line = row * width * 4..(row + 1) * width * 4;
        assert!(
            at_rest[line.clone()] == last[line],
            "row {row}: the first field shows no focus after the second took it"
        );
    }
}

/// The pointer moves between buttons and out: each button that it left stops
/// showing the hover.
#[test]
fn hover_leaves_the_node_the_pointer_left() {
    for theme in [light as fn() -> Theme, midnight] {
        let setup = Setup { theme, ..SMALL };
        let distinct = assert_identical("hover", setup, fields, |driver, (_, _, one)| {
            let at = driver.centre(one.get().id());
            driver.move_to(at.0, at.1);
            driver.move_to(at.0 + 60, at.1);
            driver.move_to(at.0 + 120, at.1);
            driver.move_to(4, 4);
            driver.move_to(at.0, at.1);
            driver.event(Event::MouseLeave);
            driver.move_to(at.0 + 2, at.1);
        });
        assert!(distinct >= 3, "the hover was visible: {distinct}");
    }
}

/// A label with a fixed width whose text overflows it, then gets shorter: the
/// overflow is clipped to the label's reach and what is left of it is
/// repainted away.
#[test]
fn an_overflowing_label_that_gets_shorter_leaves_nothing_behind() {
    let layout = || {
        let handle: Handle<Label<()>> = Handle::new();
        let layout = column().padding(8).gap(4).children((
            label("OK").bind(&handle).width(80),
            label("a neighbour that is not touched"),
        ));
        (layout, handle)
    };
    let script = |driver: &mut Driver<'_>, handle: Handle<Label<()>>| {
        handle
            .get()
            .set_text("A very long status message that overflows its fixed width box by far");
        driver.snap();
        handle.get().set_text("OK");
        driver.snap();
        handle.get().set_text("Medium length text, still too wide");
        driver.snap();
    };
    for theme in [light as fn() -> Theme, midnight] {
        let setup = Setup { theme, ..SMALL };
        let distinct = assert_identical("label", setup, layout, script);
        assert!(distinct >= 3, "the text changes were visible: {distinct}");
    }
}

/// Where an overflowing label's text can reach: its bounds and the overdraw
/// margin, no further, in a frame of damage and in a frame of everything.
#[test]
fn a_labels_text_is_clipped_to_its_reach() {
    let layout = || {
        let handle: Handle<Label<()>> = Handle::new();
        let layout = column()
            .padding(8)
            .children((label("x").bind(&handle).width(60),));
        (layout, handle)
    };
    let frames = session(SMALL, false, layout, |driver, handle| {
        handle
            .get()
            .set_text("A very long status message that overflows its box by far, far, far");
        driver.snap();
    });
    let image = frames.last().expect("a frame");
    let width = SMALL.size.0 as usize;
    let background = Theme::light().background;
    // 8 padding + 60 wide + 8 margin = 76: everything right of it is plain.
    let mut inked = 0;
    for y in 0..40usize {
        for x in 80..width {
            let at = (y * width + x) * 4;
            if image[at..at + 3] != [background.r, background.g, background.b] {
                inked += 1;
            }
        }
    }
    assert_eq!(inked, 0, "no text past the reach of its label");
}
