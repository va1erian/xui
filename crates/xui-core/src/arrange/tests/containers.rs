//! Frames and tabs that hold layouts.

use super::{bounds, setup};
use crate::arrange::{Handle, LayoutExt, button, column, group, tabs};
use crate::geometry::Rect;
use crate::widget::{Button, Tabs};

#[test]
fn a_group_lays_its_content_out_inside_its_frame() {
    let (backend, _window, ui, _runtime) = setup();
    let inner = Handle::<Button<u32>>::new();
    let _mounted = ui
        .mount(
            column()
                .padding(10)
                .child(group("Memory", column().child(button("inside").bind(&inner))).fill(1)),
        )
        .unwrap();

    let frame = backend.parent_of(inner.get().id());
    assert_eq!(frame, None, "the content is the frame's sibling");
    let placed = bounds(&ui, &inner);
    assert!(placed.left > 10 && placed.right < 390, "inside the frame");
    assert!(placed.top > 10 + 8, "below the title band");
}

#[test]
fn tab_pages_lay_out_inside_the_container_and_switch() {
    let (_backend, _window, ui, _runtime) = setup();
    let (container, first, second) = (
        Handle::<Tabs<u32>>::new(),
        Handle::<Button<u32>>::new(),
        Handle::<Button<u32>>::new(),
    );
    let _mounted = ui
        .mount(
            column().child(
                tabs()
                    .page(
                        "One",
                        column().padding(4).child(button("1").bind(&first).fill(1)),
                    )
                    .page("Two", column().child(button("2").bind(&second)))
                    .bind(&container)
                    .fill(1),
            ),
        )
        .unwrap();

    // The page panel fills the area under the 32-dip strip; the button is
    // placed in the panel's own coordinates.
    assert_eq!(bounds(&ui, &first), Rect::new(4, 4, 396, 264));
    assert!(ui.is_visible(first.get().id()) || container.get().selected() == 0);

    container.get().select(1);
    assert_eq!(bounds(&ui, &second), Rect::new(0, 0, 400, 28));
}
