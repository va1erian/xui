//! Wraps, overlays, absolute layouts, scrolling, frames hidden as a unit,
//! shared widgets, automatic relayout and the layout report.

use std::rc::Rc;

use super::{bounds, settle, setup};
use crate::arrange::{
    Align, Anchor, Handle, LayoutExt, absolute, build, button, column, group, label, overlay, row,
    scroll, stack, wrap,
};
use crate::geometry::Rect;
use crate::widget::{Button, GroupBox, HasText, Label, ScrollView};

#[test]
fn a_wrap_breaks_its_buttons_into_lines() {
    let (_backend, _window, ui, _runtime) = setup();
    let (a, b, c) = (Handle::new(), Handle::new(), Handle::new());
    let _mounted = ui
        .mount(column().child(wrap().gap(10).children((
            button("a").bind(&a).width(150),
            button("b").bind(&b).width(150),
            button("c").bind(&c).width(150),
        ))))
        .unwrap();
    assert_eq!(bounds(&ui, &a), Rect::new(0, 0, 150, 28));
    assert_eq!(bounds(&ui, &b), Rect::new(160, 0, 310, 28));
    assert_eq!(
        bounds(&ui, &c),
        Rect::new(0, 38, 150, 66),
        "the third wraps"
    );
}

#[test]
fn an_overlay_centres_its_entries_over_a_stretched_base() {
    let (_backend, _window, ui, _runtime) = setup();
    let (base, badge): (Handle<Button<u32>>, Handle<Button<u32>>) = (Handle::new(), Handle::new());
    let _mounted = ui
        .mount(
            column().child(
                overlay()
                    .children((
                        button("base").bind(&base).align(Align::Stretch),
                        button("badge").bind(&badge).width(100),
                    ))
                    .fill(1),
            ),
        )
        .unwrap();
    assert_eq!(bounds(&ui, &base), Rect::new(0, 0, 400, 300));
    assert_eq!(bounds(&ui, &badge), Rect::new(150, 136, 250, 164));
}

#[test]
fn a_stack_fills_its_area_with_every_entry() {
    let (_backend, _window, ui, _runtime) = setup();
    let (one, two) = (Handle::<Button<u32>>::new(), Handle::<Button<u32>>::new());
    let _mounted = ui
        .mount(
            column().child(
                stack()
                    .padding(4)
                    .children((button("1").bind(&one), button("2").bind(&two)))
                    .fill(1),
            ),
        )
        .unwrap();
    assert_eq!(bounds(&ui, &one), Rect::new(4, 4, 396, 296));
    assert_eq!(bounds(&ui, &two), bounds(&ui, &one));
}

#[test]
fn an_absolute_layout_places_entries_and_follows_their_anchors() {
    let (backend, window, ui, _runtime) = setup();
    let (fixed, pinned) = (Handle::<Button<u32>>::new(), Handle::<Button<u32>>::new());
    let _mounted = ui
        .mount(
            absolute().design_size(400, 300).children((
                button("fixed").bind(&fixed).at(10, 10, 80, 28),
                button("ok")
                    .bind(&pinned)
                    .at(310, 262, 80, 28)
                    .anchor(Anchor::BottomRight),
            )),
        )
        .unwrap();
    assert_eq!(bounds(&ui, &fixed), Rect::new(10, 10, 90, 38));
    assert_eq!(bounds(&ui, &pinned), Rect::new(310, 262, 390, 290));

    backend.resize_window(window, 500, 400);
    assert_eq!(bounds(&ui, &fixed), Rect::new(10, 10, 90, 38));
    assert_eq!(bounds(&ui, &pinned), Rect::new(410, 362, 490, 390));
}

#[test]
fn a_scroll_measures_its_content_unbounded_and_scrolls_it() {
    let (backend, window, ui, _runtime) = setup();
    let view = Handle::<ScrollView<u32>>::new();
    let (first, last) = (Handle::<Button<u32>>::new(), Handle::<Button<u32>>::new());
    let mut content = column().child(button("first").bind(&first));
    for n in 0..18 {
        content = content.child(button(format!("row {n}")));
    }
    let _mounted = ui
        .mount(
            column().child(
                scroll(content.child(button("last").bind(&last)))
                    .bind(&view)
                    .fill(1),
            ),
        )
        .unwrap();

    let view = view.get();
    assert_eq!(view.content_height().value(), 20 * 28);
    assert_eq!(
        backend.parent_of(first.get().id()),
        Some(view.id()),
        "the content is the view's child"
    );
    let first_rect = bounds(&ui, &first);
    assert_eq!((first_rect.left, first_rect.top), (0, 0));
    assert!(first_rect.right < 400, "the bar takes the right edge");

    view.scroll_to(crate::units::Px(100));
    assert_eq!(bounds(&ui, &first).top, -100);
    assert_eq!(bounds(&ui, &last).bottom, 20 * 28 - 100);

    backend.resize_window(window, 400, 1000);
    assert_eq!(
        bounds(&ui, &first),
        Rect::new(0, 0, 400, 28),
        "nothing to scroll"
    );
}

#[test]
fn hiding_a_group_hides_its_content_and_frees_its_space() {
    let (backend, window, ui, _runtime) = setup();
    let frame = Handle::<GroupBox<u32>>::new();
    let (inside, below) = (Handle::<Button<u32>>::new(), Handle::<Button<u32>>::new());
    let _mounted = ui
        .mount(column().children((
            group("Options", column().child(button("inside").bind(&inside))).bind(&frame),
            button("below").bind(&below),
        )))
        .unwrap();
    let shown = |handle: &Handle<Button<u32>>| backend.node(handle.get().id()).unwrap().3;
    assert!(bounds(&ui, &below).top > 28);

    ui.set_visible(frame.get().id(), false);
    settle(&backend, window);
    assert_eq!(bounds(&ui, &below).top, 0, "the frame gave up its space");
    assert!(!shown(&inside), "the content hid with the frame");
    assert!(
        ui.is_visible(inside.get().id()),
        "but the app did not hide it"
    );

    ui.set_visible(frame.get().id(), true);
    settle(&backend, window);
    assert!(shown(&inside), "and shows with it");
    assert!(bounds(&ui, &below).top > 28);
}

#[test]
fn a_widget_the_app_hides_inside_a_hidden_group_stays_hidden() {
    let (backend, window, ui, _runtime) = setup();
    let frame = Handle::<GroupBox<u32>>::new();
    let inside = Handle::<Button<u32>>::new();
    let _mounted = ui
        .mount(
            column().child(
                group("Options", column().child(button("inside").bind(&inside))).bind(&frame),
            ),
        )
        .unwrap();
    let shown = || backend.node(inside.get().id()).unwrap().3;

    ui.set_visible(frame.get().id(), false);
    settle(&backend, window);
    ui.set_visible(inside.get().id(), false);
    settle(&backend, window);
    assert!(!shown(), "the app hid it while its frame was collapsed");

    ui.set_visible(frame.get().id(), true);
    settle(&backend, window);
    assert!(!shown(), "showing the frame does not override the app");
}

#[test]
fn a_widget_shared_through_an_rc_is_placed() {
    let (_backend, _window, ui, _runtime) = setup();
    let shared = Rc::new(Button::new(&ui, Rect::default(), "shared").unwrap());
    let placed = Rc::clone(&shared);
    let _mounted = ui
        .mount(column().padding(5).child(build(move |_| Ok(placed))))
        .unwrap();
    assert_eq!(ui.bounds(shared.id()), Rect::new(5, 5, 395, 33));
}

#[test]
fn a_layout_takes_more_than_twelve_children_in_a_tuple() {
    let (_backend, _window, ui, _runtime) = setup();
    let last = Handle::<Label<u32>>::new();
    let _mounted = ui
        .mount(column().children((
            label("1"),
            label("2"),
            label("3"),
            label("4"),
            label("5"),
            label("6"),
            label("7"),
            label("8"),
            label("9"),
            label("10"),
            label("11"),
            label("12"),
            label("13"),
            label("14"),
            label("15"),
            label("16"),
            label("17"),
            label("18"),
            label("19"),
            label("20"),
            label("21"),
            label("22"),
            label("23"),
            label("24").bind(&last),
        )))
        .unwrap();
    assert!(bounds(&ui, &last).top > 0);
}

#[test]
fn a_text_change_reflows_the_layout_once_the_event_is_handled() {
    let (backend, window, ui, _runtime) = setup();
    let (text, after) = (Handle::<Label<u32>>::new(), Handle::<Button<u32>>::new());
    let _mounted = ui
        .mount(row().children((label("a").bind(&text), button("after").bind(&after))))
        .unwrap();
    let left = bounds(&ui, &after).left;

    text.get().set_text("a much longer text than before");
    settle(&backend, window);
    assert!(bounds(&ui, &after).left > left, "the label grew");
}

#[test]
fn the_layout_report_lists_the_tree_and_warns() {
    let (_backend, _window, ui, _runtime) = setup();
    ui.root(column().padding(8).children((
        label("Title"),
        row().children((
            label("a label much too long for its fixed width").width(40),
            button("OK").width(0),
        )),
    )))
    .unwrap();
    let report = ui.layout_report();
    assert!(
        report.starts_with("layout in the window, at 0,0 size 400x300"),
        "{report}"
    );
    assert!(report.contains("  column at 0,0 size 400x300"), "{report}");
    assert!(report.contains("    Label \"Title\" at 8,8"), "{report}");
    assert!(report.contains("! text truncated"), "{report}");
    assert!(report.contains("Button \"OK\""), "{report}");
    assert!(report.contains("! zero size"), "{report}");
    assert_eq!(report.matches('!').count(), 3, "{report}");
}

#[test]
fn button_builders_set_an_icon_and_a_tooltip() {
    let (_backend, _window, ui, _runtime) = setup();
    let save = Handle::<Button<u32>>::new();
    let _mounted = ui
        .mount(
            column().child(
                button("Save")
                    .icon(crate::icon::Lucide::Save)
                    .tooltip("Save the file")
                    .bind(&save),
            ),
        )
        .unwrap();
    assert_eq!(save.get().text(), "Save");
}

#[test]
fn the_layout_report_warns_of_a_check_box_too_narrow_for_its_text() {
    let (_backend, _window, ui, _runtime) = setup();
    ui.root(column().child(crate::arrange::checkbox("A long check box caption").width(40)))
        .unwrap();
    let report = ui.layout_report();
    assert!(
        report.contains("CheckBox \"A long check box caption\""),
        "{report}"
    );
    assert!(report.contains("! text truncated"), "{report}");
}

#[test]
fn an_entry_aligns_per_axis_and_takes_a_size() {
    let (_backend, _window, ui, _runtime) = setup();
    let (caption, fixed) = (Handle::<Label<u32>>::new(), Handle::<Button<u32>>::new());
    let _mounted = ui
        .mount(
            column().child(
                crate::arrange::grid([crate::arrange::Track::Fill(1)])
                    .child(
                        label("Caption")
                            .bind(&caption)
                            .align_x(Align::Start)
                            .align_y(Align::Center)
                            .fill(1),
                    )
                    .child(button("fixed").bind(&fixed).size(100, 40))
                    .fill(1),
            ),
        )
        .unwrap();
    let rect = bounds(&ui, &caption);
    assert_eq!(rect.left, 0);
    assert!(rect.top > 0 && rect.width() < 400, "{rect:?}");
    assert_eq!(
        bounds(&ui, &fixed).size(),
        crate::geometry::Size::new(100, 40)
    );

/// A widget whose `placed` dirties the layout again every time.
struct Restless(crate::backend::WidgetId);

impl crate::widget::Placeable<u32> for Restless {
    fn id(&self) -> crate::backend::WidgetId {
        self.0
    }

    fn measure(
        &self,
        _ui: &crate::app::Ui<u32>,
        _constraints: crate::layout::Constraints,
    ) -> crate::geometry::Size {
        crate::geometry::Size::new(10, 10)
    }

    fn placed(&self, ui: &crate::app::Ui<u32>, _rect: Rect) {
        ui.invalidate_layout();
    }
}

#[test]
fn a_layout_that_never_settles_gets_a_bounded_number_of_deferred_passes() {
    let (backend, window, ui, _runtime) = setup();
    let node = crate::widget::Control::new(
        &ui,
        &crate::backend::NodeSpec::new(crate::backend::NodeKind::Custom, Rect::default()),
    )
    .unwrap();
    let id = node.id();
    let _mounted = ui
        .mount(column().child(build(move |_| Ok(Restless(id)))))
        .unwrap();
    let wakes = backend.wakes(window);

    // Each delivered event (the wake included) flushes; a layout still dirty
    // after its passes asks for one more wake, three in a row at most.
    for _ in 0..6 {
        ui.invalidate_layout();
        settle(&backend, window);
    }
    assert_eq!(backend.wakes(window) - wakes, 3, "bounded retries");
    drop(node);
}
