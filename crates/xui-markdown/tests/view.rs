//! The view driven headlessly: clicks on links, scrolling by wheel, keys and
//! API, and a long document.

#[path = "view/rig.rs"]
mod rig;

use rig::*;
use xui_core::backend::Event;
use xui_core::message::{Key, Modifiers};

#[test]
fn a_click_on_a_link_delivers_its_destination() {
    run(SAMPLE, |stage, rig| {
        let at = find(stage, || clicked_next(rig)).expect("the link is on screen");
        assert_eq!(rig.log.borrow().last().map(String::as_str), Some("next.md"));
        // The same spot clicked again delivers it again.
        let before = rig.log.borrow().len();
        stage.click(at.0, at.1);
        assert_eq!(rig.log.borrow().len(), before + 1);
    });
}

#[test]
fn a_click_on_plain_text_delivers_nothing() {
    run(SAMPLE, |stage, rig| {
        stage.click(8, 8);
        stage.click(WIDTH - 100, HEIGHT - 20);
        assert!(rig.log.borrow().is_empty());
    });
}

#[test]
fn pressing_a_link_and_releasing_elsewhere_delivers_nothing() {
    run(SAMPLE, |stage, rig| {
        let at = find(stage, || clicked_next(rig)).expect("the link is on screen");
        rig.log.borrow_mut().clear();
        mouse(stage, "down", at.0, at.1);
        mouse(stage, "up", 300, HEIGHT - 10);
        assert!(rig.log.borrow().is_empty(), "{:?}", rig.log.borrow());
        // The press was dropped: a release over the link alone is not a click.
        mouse(stage, "up", at.0, at.1);
        assert!(rig.log.borrow().is_empty());
    });
}

#[test]
fn a_link_to_a_heading_scrolls_and_is_not_delivered() {
    run(SAMPLE, |stage, rig| {
        assert_eq!(rig.view.scroll(), 0.0);
        let jumped = || rig.view.scroll() > 0.0;
        find(stage, jumped).expect("the jump link is on screen");
        assert!(rig.view.scroll() > 0.0);
        assert!(
            rig.log.borrow().iter().all(|href| !href.starts_with('#')),
            "{:?}",
            rig.log.borrow()
        );
    });
}

#[test]
fn a_link_to_an_unknown_heading_is_delivered_as_written() {
    run("[x](#nowhere) and\n\n# Somewhere", |stage, rig| {
        stage.click(18, 24);
        assert_eq!(rig.log.borrow().as_slice(), ["#nowhere"]);
        assert_eq!(rig.view.scroll(), 0.0);
    });
}

#[test]
fn scroll_to_anchor_scrolls_to_a_known_heading_only() {
    run(SAMPLE, |_stage, rig| {
        assert!(rig.view.scroll_to_anchor("lists"));
        let at = rig.view.scroll();
        assert!(at > 0.0);
        assert!(!rig.view.scroll_to_anchor("nope"));
        assert_eq!(rig.view.scroll(), at, "a miss does not move the view");
        assert!(rig.view.scroll_to_anchor("a-table"));
        assert!(rig.view.scroll() > at);
    });
}

#[test]
fn the_wheel_scrolls_and_clamps_at_both_ends() {
    run(SAMPLE, |stage, rig| {
        wheel(stage, 1);
        assert_eq!(rig.view.scroll(), 0.0, "up from the top");
        wheel(stage, -1);
        let one = rig.view.scroll();
        assert!((one - 48.0).abs() < 0.5, "a notch is 48 dip, got {one}");
        wheel(stage, 1);
        assert_eq!(rig.view.scroll(), 0.0);
        for _ in 0..60 {
            wheel(stage, -1);
        }
        let end = rig.view.scroll();
        assert!(end > one);
        // Estimates above settle as the end is reached, so it can still move.
        let mut end = end;
        for _ in 0..20 {
            wheel(stage, -1);
            if rig.view.scroll() == end {
                break;
            }
            end = rig.view.scroll();
        }
        wheel(stage, -1);
        assert_eq!(rig.view.scroll(), end, "down at the end");
        wheel(stage, 1);
        assert!(rig.view.scroll() < end);
    });
}

#[test]
fn the_horizontal_wheel_does_nothing() {
    run(SAMPLE, |stage, rig| {
        stage.inject(Event::MouseWheel {
            delta: -120,
            horizontal: true,
            x: 100,
            y: 100,
            modifiers: Modifiers::NONE,
        });
        assert_eq!(rig.view.scroll(), 0.0);
    });
}

#[test]
fn home_end_and_page_down_scroll_a_focused_view() {
    run(SAMPLE, |stage, rig| {
        focus(stage, rig);
        key(stage, Key::PAGE_DOWN);
        let page = rig.view.scroll();
        assert!((page - 0.9 * HEIGHT as f32).abs() < 1.0, "got {page}");
        let end = to_end(stage, rig);
        assert!(end > page);
        key(stage, Key::PAGE_DOWN);
        assert_eq!(rig.view.scroll(), end);
        key(stage, Key::PAGE_UP);
        assert!(rig.view.scroll() < end);
        key(stage, Key::HOME);
        assert_eq!(rig.view.scroll(), 0.0);
        key(stage, Key::DOWN);
        assert!(rig.view.scroll() > 0.0);
        key(stage, Key::UP);
        assert_eq!(rig.view.scroll(), 0.0);
        key(stage, Key::SPACE);
        assert!(rig.view.scroll() > 0.0);
    });
}

#[test]
fn keys_do_nothing_until_the_view_has_the_focus() {
    run(SAMPLE, |stage, rig| {
        stage.inject(Event::KillFocus);
        key(stage, Key::PAGE_DOWN);
        key(stage, Key::END);
        assert_eq!(rig.view.scroll(), 0.0);
    });
}

#[test]
fn clicking_the_view_focuses_it_for_the_keyboard() {
    run(SAMPLE, |stage, rig| {
        stage.click(8, 8);
        key(stage, Key::END);
        assert!(rig.view.scroll() > 0.0);
    });
}

#[test]
fn set_scroll_clamps_to_the_content() {
    run(SAMPLE, |stage, rig| {
        focus(stage, rig);
        let end = to_end(stage, rig);
        rig.view.set_scroll(1.0e9);
        assert_eq!(rig.view.scroll(), end);
        rig.view.set_scroll(-50.0);
        assert_eq!(rig.view.scroll(), 0.0);
    });
}

#[test]
fn clicking_the_scrollbar_track_pages_down() {
    run(SAMPLE, |stage, rig| {
        mouse(stage, "down", WIDTH - 3, HEIGHT - 10);
        mouse(stage, "up", WIDTH - 3, HEIGHT - 10);
        assert!(rig.view.scroll() > 0.0);
        assert!(rig.log.borrow().is_empty());
    });
}

#[test]
fn the_thumb_drags_on_the_second_press_of_a_quick_pair() {
    run(SAMPLE, |stage, rig| {
        // The thumb starts at the top of the bar: a click, then a press the
        // platform reports as a double-click, held and dragged down.
        stage.click(WIDTH - 3, 10);
        assert_eq!(rig.view.scroll(), 0.0, "a click on the thumb does not page");
        stage.inject(xui_core::backend::Event::MouseDoubleClick {
            x: WIDTH - 3,
            y: 10,
            button: xui_core::message::MouseButton::Left,
            modifiers: Modifiers::NONE,
        });
        mouse(stage, "move", WIDTH - 3, 70);
        mouse(stage, "up", WIDTH - 3, 70);
        assert!(rig.view.scroll() > 0.0, "the drag moved the page");
        assert!(rig.log.borrow().is_empty());
    });
}

#[test]
fn set_markdown_resets_the_scroll() {
    run(SAMPLE, |stage, rig| {
        wheel(stage, -5);
        assert!(rig.view.scroll() > 0.0);
        rig.view.set_markdown("# Another\n\nShort.");
        assert_eq!(rig.view.scroll(), 0.0);
        wheel(stage, -5);
        assert_eq!(rig.view.scroll(), 0.0, "too short to scroll");
        rig.view.set_markdown(SAMPLE);
        assert_eq!(rig.view.scroll(), 0.0);
        assert_eq!(rig.view.current_document().text(), {
            let doc = xui_markdown::Document::parse(SAMPLE);
            doc.text().to_owned()
        });
    });
}

#[test]
fn an_empty_document_does_not_scroll_or_panic() {
    run("", |stage, rig| {
        focus(stage, rig);
        wheel(stage, -3);
        key(stage, Key::END);
        key(stage, Key::PAGE_DOWN);
        stage.click(20, 20);
        assert_eq!(rig.view.scroll(), 0.0);
        assert!(!rig.view.scroll_to_anchor("x"));
    });
}

fn long() -> String {
    (0..2000)
        .map(|i| {
            format!(
                "Paragraph {i} has **bold** words and a [link](l{i}.md) so that it wraps \
                 onto a second or third line in a window this narrow.\n\n"
            )
        })
        .collect()
}

#[test]
fn a_long_document_opens_scrolls_and_jumps_to_its_end_at_once() {
    let markdown = long();
    let image = run(&markdown, |stage, rig| {
        focus(stage, rig);
        let end = to_end(stage, rig);
        assert!(end > 50_000.0, "2,000 paragraphs are tall, got {end}");
        key(stage, Key::HOME);
        assert_eq!(rig.view.scroll(), 0.0);
        rig.view.set_scroll(end / 2.0);
        wheel(stage, -3);
        assert!(rig.view.scroll() > end / 2.0);
        // A link far down the document is clickable once it is shown.
        to_end(stage, rig);
        let mut hit = false;
        for y in (8..HEIGHT - 8).step_by(6) {
            for x in (16..WIDTH - 40).step_by(8) {
                stage.click(x, y);
                hit |= !rig.log.borrow().is_empty();
            }
        }
        assert!(hit, "a link at the end of the document");
        let last = rig.log.borrow().last().cloned().expect("a link");
        assert!(last.starts_with("l19"), "{last}");
    });
    assert_eq!(image.size(), (WIDTH as u32, HEIGHT as u32));
}

#[test]
fn the_first_paint_shows_the_top_padding() {
    // Scroll 0 is the very top, padding included: Home and the first paint
    // must agree on it.
    run_raw(SAMPLE, |stage, rig| {
        let first = rig.view.scroll();
        focus(stage, rig);
        key(stage, Key::END);
        key(stage, Key::HOME);
        assert_eq!(first, rig.view.scroll(), "the first paint is scrolled down");
    });
}

#[test]
fn end_lands_on_the_real_end_at_once() {
    run(SAMPLE, |stage, rig| {
        focus(stage, rig);
        key(stage, Key::END);
        let first = rig.view.scroll();
        key(stage, Key::END);
        assert_eq!(first, rig.view.scroll(), "scrolled past the end");
    });
}

#[test]
fn end_stays_on_the_end_when_the_last_blocks_are_taller_than_estimated() {
    // Table rows are estimated at one line, and these cells wrap to many in
    // a narrow column: the real end lies well past the estimated one.
    let cell = "word ".repeat(80);
    let mut text = String::from(
        "| a | b |
| --- | --- |
",
    );
    for _ in 0..30 {
        text.push_str(&format!(
            "| {cell} | {cell} |
"
        ));
    }
    run(&text, |stage, rig| {
        focus(stage, rig);
        key(stage, Key::END);
        let end = rig.view.scroll();
        key(stage, Key::DOWN);
        assert_eq!(rig.view.scroll(), end, "End stopped short of the end");
        key(stage, Key::END);
        assert_eq!(rig.view.scroll(), end, "a second End moved");
    });
}
