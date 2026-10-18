//! A `BlitzView` end to end on the offscreen backend: layout reaches the
//! pixels, links are reported or followed, selection copies, and the theme
//! reaches `prefers-color-scheme`.

mod common;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use common::{Msg, builder, rgb, run};
use xui_blitz::{BlitzView, BlitzViewEvent};
use xui_canvas::snapshot::{Snapshot, render_with};
use xui_core::Dip;
use xui_core::app::{App, Ui};
use xui_core::arrange::{Handle, LayoutExt, build, column};
use xui_core::backend::Event;
use xui_core::geometry::Rect;
use xui_core::message::{Key, Modifiers, MouseButton};
use xui_core::theme::Theme;

const RED: [u8; 3] = [255, 0, 0];
const BLUE: [u8; 3] = [0, 0, 255];
const GREEN: [u8; 3] = [0, 128, 0];

#[test]
fn grid_and_custom_properties_reach_the_pixels() {
    let html = r#"<style>
        :root { --left: #f00; }
        body { margin: 0 }
        .g { display: grid; grid-template-columns: 100px 100px; height: 100px }
    </style>
    <div class="g"><div style="background: var(--left)"></div><div style="background: #00f"></div></div>"#;
    let image = run(
        Theme::light(),
        move || builder().html(html),
        |p| p.wait_loaded(),
    );
    assert_eq!(rgb(&image, 50, 50), RED);
    assert_eq!(rgb(&image, 150, 50), BLUE);
    assert_eq!(rgb(&image, 250, 50), [255, 255, 255]);
}

#[test]
fn the_title_and_loading_are_reported() {
    run(
        Theme::light(),
        || builder().html("<title> A  page </title><p>x"),
        |p| {
            p.wait_loaded();
            let events = p.take_events();
            assert!(events.contains(&BlitzViewEvent::TitleChanged("A page".into())));
            assert!(events.contains(&BlitzViewEvent::LoadingChanged(false)));
        },
    );
}

const LINK_PAGE: &str = r#"<body style="margin: 0">
    <a href="LINK" style="display: block; height: 100px; background: #0f0">go</a>"#;

#[test]
fn a_link_is_reported_when_the_view_does_not_follow_links() {
    let html = LINK_PAGE.replace("LINK", "/docs/next.html");
    run(
        Theme::light(),
        move || builder().html(html).base_url("https://example.com/docs/"),
        |p| {
            p.wait_loaded();
            p.stage.hover(50, 50);
            p.wait("the status line", |e| {
                e.contains(&BlitzViewEvent::StatusChanged(
                    "https://example.com/docs/next.html".into(),
                ))
            });
            p.stage.click(50, 50);
            p.wait("the link", |e| {
                e.contains(&BlitzViewEvent::LinkClicked(
                    "https://example.com/docs/next.html".into(),
                ))
            });
        },
    );
}

#[test]
fn a_followed_link_loads_its_page() {
    let target = "data:text/html,<title>Two</title><body style='background:%2300f'>";
    let html = LINK_PAGE.replace("LINK", target);
    let image = run(
        Theme::light(),
        move || builder().html(html).follow_links(true),
        |p| {
            p.wait_loaded();
            p.stage.click(50, 50);
            p.wait("the second page", |e| {
                e.contains(&BlitzViewEvent::TitleChanged("Two".into()))
            });
            p.wait_loaded();
        },
    );
    assert_eq!(rgb(&image, 200, 200), BLUE);
}

#[test]
fn a_mailto_link_is_handed_to_the_host() {
    let html = LINK_PAGE.replace("LINK", "mailto:someone@example.com");
    run(
        Theme::light(),
        move || builder().html(html).follow_links(true),
        |p| {
            p.wait_loaded();
            p.stage.click(50, 50);
            p.wait("the launch", |e| {
                e.contains(&BlitzViewEvent::LaunchUrl {
                    url: "mailto:someone@example.com".into(),
                    by_user: true,
                })
            });
        },
    );
}

#[test]
fn selected_text_is_copied_on_ctrl_c() {
    run(
        Theme::light(),
        || {
            builder()
                .html("<body style='margin: 0; font-size: 40px'><p style='margin: 0'>Hello there")
        },
        |p| {
            p.wait_loaded();
            let mods = Modifiers::NONE;
            p.stage.inject(Event::MouseDown {
                x: 2,
                y: 20,
                button: MouseButton::Left,
                modifiers: mods,
            });
            p.stage.inject(Event::MouseMove {
                x: 370,
                y: 20,
                modifiers: mods,
            });
            p.stage.inject(Event::MouseUp {
                x: 370,
                y: 20,
                button: MouseButton::Left,
                modifiers: mods,
            });
            p.stage.inject(Event::KeyDown {
                key: Key::C,
                modifiers: Modifiers {
                    ctrl: true,
                    ..Modifiers::NONE
                },
                repeat: 1,
                system: false,
            });
            p.wait("the copy", |e| {
                e.iter().any(
                    |e| matches!(e, BlitzViewEvent::CopyRequested(t) if t.starts_with("Hello")),
                )
            });
        },
    );
}

#[test]
fn the_dark_theme_reaches_prefers_color_scheme() {
    let html = r#"<style>
        body { margin: 0; background: #f00 }
        @media (prefers-color-scheme: dark) { body { background: #008000 } }
    </style>"#;
    let dark = run(
        Theme::dark(),
        move || builder().html(html),
        |p| p.wait_loaded(),
    );
    assert_eq!(rgb(&dark, 200, 150), GREEN);
    let light = run(
        Theme::light(),
        move || builder().html(html),
        |p| p.wait_loaded(),
    );
    assert_eq!(rgb(&light, 200, 150), RED);
}

#[test]
fn the_wheel_scrolls_the_page() {
    let html = r#"<body style="margin: 0">
        <div style="height: 300px; background: #f00"></div>
        <div style="height: 2000px; background: #00f"></div>"#;
    let image = run(
        Theme::light(),
        move || builder().html(html),
        |p| {
            p.wait_loaded();
            p.stage.inject(Event::MouseWheel {
                delta: -120 * 5,
                horizontal: false,
                x: 100,
                y: 100,
                modifiers: Modifiers::NONE,
            });
            for _ in 0..20 {
                let _ = p.stage.ui().capture();
                p.stage.emit(Msg::Frame);
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        },
    );
    // Five notches down is past the red block.
    assert_eq!(rgb(&image, 200, 10), BLUE);
}

#[test]
fn typing_into_a_field_and_pressing_enter_submits_the_form() {
    let html = r#"<body style="margin: 0">
        <form action="https://example.com/search">
        <input name="q" style="display: block; width: 300px; height: 40px; font-size: 20px">
        </form>"#;
    run(
        Theme::light(),
        move || builder().html(html),
        |p| {
            p.wait_loaded();
            p.stage.click(50, 20);
            for c in "hi there".chars() {
                p.stage.inject(Event::Char(c));
            }
            p.stage.inject(Event::KeyDown {
                key: Key::RETURN,
                modifiers: Modifiers::NONE,
                repeat: 1,
                system: false,
            });
            p.wait("the submission", |e| {
                e.contains(&BlitzViewEvent::LinkClicked(
                    "https://example.com/search?q=hi+there".into(),
                ))
            });
        },
    );
}

#[test]
fn legacy_attributes_and_xhtml_doctypes_style_html_mail() {
    // An XHTML doctype on HTML still parses as HTML, and body `text`, `<font
    // color>` and `cellpadding` reach the style (`currentColor` shows it).
    let html = r##"<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Transitional//EN" "http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd">
<html xmlns="http://www.w3.org/1999/xhtml"><body text="#ff0000" style="margin: 0">
<div style="height: 50px; background: currentColor"></div>
<font color="#0000FF"><div style="height: 50px; background: currentColor"></div></font>
<table cellpadding="30" cellspacing="0" style="background: #008000"><tr><td></td></tr></table>"##;
    let image = run(
        Theme::light(),
        move || builder().html(html),
        |p| p.wait_loaded(),
    );
    assert_eq!(rgb(&image, 20, 25), RED);
    assert_eq!(rgb(&image, 20, 75), BLUE);
    // The empty cell is 60px square only with its padding.
    assert_eq!(rgb(&image, 50, 150), GREEN);
}

#[test]
fn a_scrollbar_thumb_shows_for_a_tall_page_and_dragging_it_scrolls() {
    let html = r#"<body style="margin: 0">
        <div style="height: 300px; background: #f00"></div>
        <div style="height: 2000px; background: #00f"></div>"#;
    let image = run(
        Theme::light(),
        move || builder().html(html),
        |p| {
            p.wait_loaded();
            // Press the thumb at the top of the right edge and drag it down.
            let x = 400 - 8;
            p.stage.inject(Event::MouseDown {
                x,
                y: 10,
                button: MouseButton::Left,
                modifiers: Modifiers::NONE,
            });
            p.stage.inject(Event::MouseMove {
                x,
                y: 200,
                modifiers: Modifiers::NONE,
            });
            p.stage.inject(Event::MouseUp {
                x,
                y: 200,
                button: MouseButton::Left,
                modifiers: Modifiers::NONE,
            });
            for _ in 0..20 {
                let _ = p.stage.ui().capture();
                p.stage.emit(Msg::Frame);
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        },
    );
    // The page moved past the red block; the gutter is the bar, not the page.
    assert_eq!(rgb(&image, 100, 10), BLUE);
    assert_ne!(rgb(&image, 400 - 8, 290), BLUE);
}

#[test]
fn the_scrollbar_is_drawn_in_the_gutter_beside_the_page() {
    let html = r#"<body style="margin: 0; background: #fff">
        <div style="height: 2000px; background: #fff"></div>"#;
    let image = run(
        Theme::light(),
        move || builder().html(html),
        |p| p.wait_loaded(),
    );
    // The thumb sits at the top of the right-hand gutter, and nothing of the
    // bar is drawn at the view's top-left corner.
    let gutter: Vec<[u8; 3]> = (0..60).map(|y| rgb(&image, 400 - 8, y)).collect();
    assert!(
        gutter.iter().any(|px| *px != [255, 255, 255]),
        "no thumb in the gutter"
    );
    assert_eq!(rgb(&image, 8, 10), [255, 255, 255]);
}

#[test]
fn a_right_click_reports_the_picture_and_link_under_the_pointer() {
    let png = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='80' height='80'%3E%3Crect width='80' height='80' fill='%23f00'/%3E%3C/svg%3E";
    let html = format!(
        r#"<body style="margin: 0"><a href="/next"><img src="{png}" width="80" height="80"></a>"#
    );
    run(
        Theme::light(),
        move || builder().html(html).base_url("https://example.com/dir/"),
        |p| {
            p.wait_loaded();
            let at = |button, up: bool| {
                if up {
                    Event::MouseUp {
                        x: 40,
                        y: 40,
                        button,
                        modifiers: Modifiers::NONE,
                    }
                } else {
                    Event::MouseDown {
                        x: 40,
                        y: 40,
                        button,
                        modifiers: Modifiers::NONE,
                    }
                }
            };
            p.stage.inject(at(MouseButton::Right, false));
            p.stage.inject(at(MouseButton::Right, true));
            p.wait("the context menu event", |e| {
                e.iter().any(|e| {
                    matches!(e, BlitzViewEvent::ContextMenu { link: Some(l), image: Some(i), .. }
                        if l == "https://example.com/next" && i.starts_with("data:image/svg"))
                })
            });
        },
    );
}

/// A view a layout built, kept so the app can pump its frames.
struct Placed {
    view: Rc<BlitzView<Msg>>,
    events: Rc<RefCell<Vec<BlitzViewEvent>>>,
}

impl App for Placed {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Frame => self.view.update(),
            Msg::Event(event) => self.events.borrow_mut().push(event),
            Msg::Navigate(url) => self.view.navigate(&url),
        }
    }
}

#[test]
fn a_layout_places_the_view_and_its_scrollbar() {
    let html = r#"<body style="margin: 0; background: #00f">
        <div style="height: 2000px"></div>"#;
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&events);
    let image = render_with(
        Snapshot::new(Dip(400.0), Dip(300.0)).theme(Theme::light()),
        move |ui| {
            let view: Handle<BlitzView<Msg>> = Handle::new();
            ui.root(
                column().padding(40).child(
                    build(move |ui| builder().html(html).build(ui, Rect::default()))
                        .bind(&view)
                        .fill(1),
                ),
            )?;
            Ok(Placed {
                view: view.get(),
                events,
            })
        },
        move |stage| {
            let start = Instant::now();
            let loaded = |events: &[BlitzViewEvent]| {
                events
                    .iter()
                    .rev()
                    .any(|e| matches!(e, BlitzViewEvent::LoadingChanged(false)))
            };
            while !loaded(&seen.borrow()) {
                stage.emit(Msg::Frame);
                let _ = stage.ui().capture();
                assert!(
                    start.elapsed() < Duration::from_secs(30),
                    "timed out waiting for the page"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            for _ in 0..20 {
                stage.emit(Msg::Frame);
                let _ = stage.ui().capture();
                std::thread::sleep(Duration::from_millis(10));
            }
        },
    )
    .expect("render");

    // The view fills the padded slot (40..360 × 40..260), not the window.
    assert_eq!(rgb(&image, 200, 150), BLUE);
    let window = rgb(&image, 10, 10);
    assert_ne!(window, BLUE, "the padding is not the view");
    // Its scrollbar was re-laid along the slot's right edge, where the page
    // does not paint: 400 - 40 - 8 + 4.
    let gutter: Vec<[u8; 3]> = (45..255).map(|y| rgb(&image, 356, y)).collect();
    assert!(
        gutter.iter().any(|px| *px != BLUE && *px != window),
        "no scrollbar in the slot's gutter: {gutter:?}"
    );
}
