//! View coordinates: the text margin and scroll applied to carets and hits.

mod common;

use common::sample_document;
use xui_canvas::snapshot::{Snapshot, try_render};
use xui_core::app::{App, Ui};
use xui_core::geometry::Point;
use xui_core::{Dip, Rect, Theme};
use xui_rich_text::model::Affinity;
use xui_rich_text::{DocPos, RichTextEditor};

struct Host {
    _editor: RichTextEditor<()>,
}

impl App for Host {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

/// Runs `check` against a 640 x 300 editor on the sample document, before any
/// paint, so the methods must lay the document out themselves.
fn with_editor(check: impl FnOnce(&RichTextEditor<()>)) {
    try_render(
        Snapshot::new(Dip(640.0), Dip(300.0)).theme(Theme::light()),
        |ui| {
            let editor =
                RichTextEditor::new(ui, Rect::new(0, 0, 640, 300))?.document(sample_document());
            check(&editor);
            Ok(Host { _editor: editor })
        },
    )
    .expect("render");
}

#[test]
fn carets_include_the_text_margin_and_the_scroll() {
    with_editor(|editor| {
        let start = editor.caret_rect(DocPos::new(0, 0), Affinity::Downstream);
        assert_eq!(start.left, 8, "the 8 dip margin at 96 dpi");
        assert_eq!(start.top, 22, "the heading's space before");

        let low = DocPos::new(4, 0);
        let before = editor.caret_rect(low, Affinity::Downstream);
        editor.set_scroll(60.0);
        let after = editor.caret_rect(low, Affinity::Downstream);
        assert_eq!(after.top, before.top - 60);
        assert_eq!(after.left, before.left);
    });
}

#[test]
fn view_points_hit_back_to_their_carets() {
    with_editor(|editor| {
        for scroll in [0.0, 80.0] {
            editor.set_scroll(scroll);
            for pos in [DocPos::new(0, 8), DocPos::new(3, 14), DocPos::new(5, 2)] {
                let r = editor.caret_rect(pos, Affinity::Downstream);
                if r.top < 0 || r.bottom > 300 {
                    continue;
                }
                let back = editor.pos_at(Point::new(r.left, (r.top + r.bottom) / 2));
                assert_eq!(back, pos, "scroll {scroll}");
            }
        }
    });
}

#[test]
fn ensuring_a_caret_visible_scrolls_the_least() {
    with_editor(|editor| {
        let last = DocPos::new(10, 3);
        editor.ensure_caret_visible(last);
        let r = editor.caret_rect(last, Affinity::Downstream);
        assert!(r.top >= 0 && r.bottom <= 300, "visible: {r:?}");
        assert_eq!(r.bottom, 300, "scrolled only as far as needed");

        // Already visible: nothing moves.
        editor.ensure_caret_visible(last);
        assert_eq!(editor.caret_rect(last, Affinity::Downstream), r);

        // Back up to the first line.
        editor.ensure_caret_visible(DocPos::new(0, 0));
        assert_eq!(
            editor
                .caret_rect(DocPos::new(0, 0), Affinity::Downstream)
                .top,
            0
        );
    });
}
