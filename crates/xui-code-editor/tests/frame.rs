//! The editor's frame is stroked on the edge of its node, so it is two pixels
//! wide: half inside the node and half outside. It must not be drawn under a
//! clip of the node's own rectangle, which would cut the outer half (and leave
//! nothing at all on the side the scrollbar covers).

use xui_canvas::snapshot::{Snapshot, try_render};
use xui_code_editor::{Editor, Options};
use xui_core::app::{App, Ui};
use xui_core::arrange::{Handle, LayoutExt, absolute, build};
use xui_core::geometry::Rect;
use xui_core::units::Dip;
use xui_core::{Image, Theme};

struct Empty;

impl App for Empty {
    type Msg = ();
    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

fn render(theme: Theme, view: Rect, lines: usize) -> Image {
    try_render(
        Snapshot::new(Dip(260.0), Dip(160.0)).theme(theme),
        move |ui| {
            let editor = Handle::new();
            ui.root(
                absolute().child(
                    build(move |ui| Editor::with_options(ui, Rect::default(), Options::default()))
                        .bind(&editor)
                        .at(view.left, view.top, view.width(), view.height()),
                ),
            )?;
            let text: String = (0..lines).map(|line| format!("line {line}\n")).collect();
            editor.get().set_text(&text);
            Ok(Empty)
        },
    )
    .expect("the editor renders")
}

fn pixel(image: &Image, x: i32, y: i32) -> [u8; 4] {
    image
        .pixel(x as u32, y as u32)
        .unwrap_or_else(|| panic!("pixel ({x}, {y}) is inside the image"))
}

#[test]
fn the_frame_keeps_both_halves_on_every_side_with_and_without_a_scrollbar() {
    let view = Rect::new(20, 20, 220, 130);
    let (mx, my) = ((view.left + view.right) / 2, (view.top + view.bottom) / 2);
    // Three lines fit; forty overflow, so the vertical bar is on the right.
    for lines in [3, 40] {
        for theme in [Theme::light(), Theme::dark()] {
            let image = render(theme, view, lines);
            // Each outer half pixel against the window two pixels further out.
            let outer = [
                ("top", (mx, view.top - 1), (mx, view.top - 3)),
                ("bottom", (mx, view.bottom), (mx, view.bottom + 2)),
                ("left", (view.left - 1, my), (view.left - 3, my)),
                ("right", (view.right, my), (view.right + 2, my)),
            ];
            for (side, (fx, fy), (bx, by)) in outer {
                assert_ne!(
                    pixel(&image, fx, fy),
                    pixel(&image, bx, by),
                    "{lines} lines: the outer half of the frame on the {side} side is drawn"
                );
            }
        }
    }
}
