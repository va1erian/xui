//! A `BlitzView` end to end on the offscreen backend: layout reaches the
//! pixels, links are reported or followed, selection copies, and the theme
//! reaches `prefers-color-scheme`.

mod common;

use common::{Msg, builder, rgb, run};
use xui_blitz::BlitzViewEvent;
use xui_core::backend::Event;
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
                x: 395,
                y: 20,
                modifiers: mods,
            });
            p.stage.inject(Event::MouseUp {
                x: 395,
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
