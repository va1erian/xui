//! A scrolled `Edit` paints its selection over the text it covers: the scroll
//! offset is applied once, not twice.

use xui_canvas::snapshot::{Snapshot, render_with};
use xui_core::app::{App, Ui};
use xui_core::backend::Event;
use xui_core::message::{Key, Modifiers};
use xui_core::widget::Edit;
use xui_core::{Dip, Rect, Theme};

struct Field {
    _edit: Edit<()>,
}

impl App for Field {
    type Msg = ();
    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

const FIELD: Rect = Rect::new(10, 10, 210, 38);

#[test]
fn select_all_on_a_scrolled_field_highlights_up_to_its_right_edge() {
    let theme = Theme::light();
    let long = "the quick brown fox jumps over the lazy dog ".repeat(4);
    let image = render_with(
        Snapshot::new(Dip(240.0), Dip(60.0)).theme(theme),
        move |ui| {
            Ok(Field {
                _edit: Edit::new(ui, FIELD, &long)?,
            })
        },
        |stage| {
            // Focus the field, move to the end (scrolling it), then select all.
            stage.click(FIELD.left + 20, FIELD.top + 14);
            let ctrl = Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            };
            for (key, modifiers) in [(Key::END, Modifiers::NONE), (Key::A, ctrl)] {
                stage.inject(Event::KeyDown {
                    key,
                    modifiers,
                    repeat: 1,
                    system: false,
                });
            }
        },
    )
    .expect("the field renders");

    let selection = theme.selection;
    let is_selection = |x: u32, y: u32| {
        image.pixel(x, y).is_some_and(|[r, g, b, _]| {
            r.abs_diff(selection.r) <= 8
                && g.abs_diff(selection.g) <= 8
                && b.abs_diff(selection.b) <= 8
        })
    };
    // The whole text is selected and the field is scrolled to its end, so the
    // highlight reaches the field's right edge. With the scroll applied twice
    // it stopped `scroll` pixels short of it.
    let right_band = (FIELD.right - 12) as u32..(FIELD.right - 4) as u32;
    let rows = (FIELD.top + 4) as u32..(FIELD.bottom - 4) as u32;
    let highlighted = right_band
        .clone()
        .any(|x| rows.clone().any(|y| is_selection(x, y)));
    assert!(
        highlighted,
        "the selection reaches the scrolled field's right edge"
    );
}
