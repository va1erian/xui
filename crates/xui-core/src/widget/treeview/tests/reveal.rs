use std::time::Duration;

use super::super::reveal::Active;
use super::*;

const ROW: i32 = 22;

#[test]
fn the_band_grows_with_progress_and_the_rows_below_follow_its_edge() {
    let at = |p| Active::new(1, 5, p, 3, 10);
    assert_eq!(at(0.0).band_height(ROW), 0);
    assert_eq!(at(0.5).band_height(ROW), 33);
    assert_eq!(at(1.0).band_height(ROW), 66);
    assert_eq!(
        at(0.0).shift(ROW),
        66,
        "closed: the rows below sit on the node"
    );
    assert_eq!(at(0.5).shift(ROW), 33);
    assert_eq!(
        at(1.0).shift(ROW),
        0,
        "open: they are where they always were"
    );
    assert_eq!(at(2.0).band_height(ROW), 66, "progress is clamped");
}

#[test]
fn a_huge_subtree_animates_as_a_viewport_worth() {
    let huge = Active::new(1, 1001, 1.0, 1000, 5);
    assert_eq!(
        huge.band_height(ROW),
        5 * ROW,
        "the band stops at the viewport"
    );
    let start = Active::new(1, 1001, 0.0, 1000, 5);
    assert_eq!(
        start.shift(ROW),
        1000 * ROW,
        "the rows below start on the node"
    );
    assert!(huge.contains(2) && huge.contains(1000));
    assert!(!huge.contains(1) && !huge.contains(1001));
    assert!(huge.after(1001) && !huge.after(1000));
}

fn rows() -> [TreeRow; 5] {
    [
        TreeRow::new("top", 0),
        TreeRow::new("folder", 0).expandable(true),
        TreeRow::new("c1", 1),
        TreeRow::new("c2", 1),
        TreeRow::new("after", 0),
    ]
}

struct Rig {
    backend: Rc<HeadlessBackend>,
    runtime: Rc<Runtime<TestApp>>,
    tree: TreeView<u32>,
}

fn rig(clock: bool, rows: &[TreeRow]) -> Rig {
    LOG.with(|log| log.borrow_mut().clear());
    let backend = Rc::new(HeadlessBackend::new());
    if clock {
        backend.set_now(Duration::ZERO);
    }
    let window = backend.open_window(&PlatformSpec::new("t")).unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    let tree = TreeView::new(&ui, Rect::new(0, 0, 160, 220), rows).unwrap();
    Rig {
        backend,
        runtime: Runtime::primary(core, TestApp),
        tree,
    }
}

impl Rig {
    /// Clicks the chevron of the visible slot `slot`.
    fn toggle(&self, slot: i32) {
        self.runtime
            .deliver(self.tree.id(), &down(8, slot * ROW + ROW / 2));
    }

    fn frame(&self, after_ms: u64) {
        self.backend.advance(Duration::from_millis(after_ms));
        self.runtime.deliver(WidgetId::NONE, &Event::Frame);
    }

    fn ops(&self) -> Vec<DrawOp> {
        self.backend.render(self.tree.id());
        self.backend.ops(self.tree.id())
    }

    /// The top of the row labelled `label`, if it is drawn.
    fn top_of(&self, label: &str) -> Option<i32> {
        self.ops().iter().find_map(|op| match op {
            DrawOp::Text(rect, text, _) if text == label => Some(rect.top),
            _ => None,
        })
    }
}

/// The picture of `rows` with no clock, after toggling `slot` if given.
fn plain(rows: &[TreeRow], slot: Option<i32>) -> Vec<DrawOp> {
    let rig = rig(false, rows);
    if let Some(slot) = slot {
        rig.toggle(slot);
    }
    assert_eq!(rig.backend.frame_requests(), 0);
    rig.ops()
}

#[test]
fn expanding_reveals_the_rows_under_the_node_and_slides_the_rest_down() {
    let rig = rig(true, &rows());
    assert_eq!(rig.top_of("after"), Some(2 * ROW));
    rig.toggle(1);
    assert!(rig.backend.frame_requests() > 0, "frames are asked for");

    assert_eq!(rig.top_of("after"), Some(2 * ROW), "starts where it was");
    assert_eq!(rig.top_of("c1"), None, "nothing is revealed yet");

    rig.frame(30);
    let mid = rig.top_of("after").unwrap();
    assert!(mid > 2 * ROW && mid < 4 * ROW, "between the two: {mid}");
    assert!(rig.top_of("c1").is_some(), "the first child shows");
    assert!(
        rig.ops().iter().any(|op| matches!(op, DrawOp::Clip(_))),
        "the children are clipped to the band"
    );

    rig.frame(400);
    assert_eq!(
        rig.ops(),
        plain(&rows(), Some(1)),
        "the last frame is today's"
    );
    let requests = rig.backend.frame_requests();
    rig.frame(100);
    assert_eq!(
        rig.backend.frame_requests(),
        requests,
        "no frame after it lands"
    );
}

#[test]
fn collapsing_keeps_painting_the_rows_until_they_are_gone() {
    let mut open = rows();
    open[1] = TreeRow::new("folder", 0).expandable(true).expanded(true);
    let rig = rig(true, &open);
    assert_eq!(rig.top_of("after"), Some(4 * ROW));
    rig.toggle(1);

    assert_eq!(rig.top_of("after"), Some(4 * ROW), "starts where it was");
    rig.frame(30);
    let mid = rig.top_of("after").unwrap();
    assert!(mid > 2 * ROW && mid < 4 * ROW, "between the two: {mid}");
    assert!(rig.top_of("c1").is_some(), "the collapsing rows still show");

    rig.frame(400);
    assert_eq!(rig.ops(), plain(&open, Some(1)));
    assert_eq!(rig.top_of("c1"), None);
    let requests = rig.backend.frame_requests();
    rig.frame(100);
    assert_eq!(rig.backend.frame_requests(), requests);
}

#[test]
fn without_a_clock_the_change_paints_the_final_picture_at_once() {
    let rig = rig(false, &rows());
    rig.toggle(1);
    assert_eq!(rig.backend.frame_requests(), 0);
    assert_eq!(rig.top_of("after"), Some(4 * ROW));
    assert!(rig.top_of("c2").is_some());
}

#[test]
fn toggling_again_mid_flight_turns_round() {
    let rig = rig(true, &rows());
    rig.toggle(1);
    rig.frame(30);
    rig.toggle(1);
    rig.frame(30);
    let mid = rig.top_of("after").unwrap();
    assert!(mid > 2 * ROW && mid < 4 * ROW, "still in between: {mid}");
    rig.frame(400);
    assert_eq!(rig.ops(), plain(&rows(), None));
}

#[test]
fn toggling_another_node_mid_flight_ends_the_first_at_once() {
    let two = [
        TreeRow::new("a", 0).expandable(true),
        TreeRow::new("a1", 1),
        TreeRow::new("b", 0).expandable(true),
        TreeRow::new("b1", 1),
    ];
    let rig = rig(true, &two);
    rig.toggle(0);
    rig.frame(30);
    // "a" is open in the model, so "b" now sits in slot 2.
    rig.toggle(2);
    rig.frame(400);
    let done = [
        two[0].clone().expanded(true),
        two[1].clone(),
        two[2].clone().expanded(true),
        two[3].clone(),
    ];
    assert_eq!(rig.ops(), plain(&done, None));
}

#[test]
fn changing_the_model_mid_flight_ends_on_the_new_picture() {
    let rig = rig(true, &rows());
    rig.toggle(1);
    rig.frame(30);
    rig.tree.set_rows(&rows()[..2]);
    rig.frame(30);
    rig.frame(400);
    assert_eq!(rig.ops(), plain(&rows()[..2], None));

    rig.tree.set_rows(&rows());
    rig.toggle(1);
    rig.frame(30);
    rig.tree.refresh_rows(&rows());
    rig.frame(400);
    assert_eq!(rig.ops(), plain(&rows(), None));
}
