//! The scroll between two pages.

use super::*;

/// The backend, the runtime, the container and its pages' widgets.
type Paged = (
    Rc<HeadlessBackend>,
    Rc<Runtime<TestApp>>,
    Tabs<usize>,
    [Label<usize>; 3],
);

/// [`animated`], keeping the pages' widgets alive: a dropped widget destroys
/// its node, and these tests read the nodes.
fn paged() -> Paged {
    let (backend, core, ui) = setup();
    backend.set_now(Duration::ZERO);
    let tabs = Tabs::new(&ui, Rect::new(0, 0, 300, 120)).unwrap();
    let pages = [label(&tabs, "a"), label(&tabs, "b"), label(&tabs, "c")];
    let tabs = tabs
        .page("A", &[pages[0].id()])
        .page("B", &[pages[1].id()])
        .page("C", &[pages[2].id()]);
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));
    (backend, runtime, tabs, pages)
}

/// The page area of the 300x120 container `paged` builds.
const PAGE: Rect = Rect::new(0, 32, 300, 120);

/// The bounds and visibility of page `index`'s only child.
fn page(backend: &HeadlessBackend, tabs: &Tabs<usize>, index: usize) -> (Rect, bool) {
    let child = tabs.shared.pages.borrow()[index][0];
    let node = backend.node(child).unwrap();
    (node.1, node.3)
}

#[test]
fn selecting_a_tab_scrolls_the_new_page_in_and_the_old_one_out() {
    let (backend, runtime, tabs, _pages) = paged();
    tabs.select(1);
    assert_eq!(page(&backend, &tabs, 1), (PAGE.offset(300, 0), true));
    assert_eq!(page(&backend, &tabs, 0), (PAGE, true), "still shown");
    assert_eq!(
        backend.clip(tabs.id()),
        Some(Rect::new(0, 0, 300, 120)),
        "a page partly outside is not drawn over the neighbours"
    );

    frame_after(&runtime, &backend, 90);
    let ((new, _), (old, shown)) = (page(&backend, &tabs, 1), page(&backend, &tabs, 0));
    assert!(new.left > 0 && new.left < 300, "{new:?}");
    assert_eq!(old.right, new.left, "the pages move as one strip");
    assert!(shown);

    frame_after(&runtime, &backend, 200);
    assert_eq!(page(&backend, &tabs, 1), (PAGE, true));
    assert!(!page(&backend, &tabs, 0).1, "the old page is hidden");
    assert_eq!(backend.clip(tabs.id()), None);
    let asked = backend.frame_requests();
    frame_after(&runtime, &backend, 16);
    assert_eq!(backend.frame_requests(), asked, "nothing moves any more");
}

#[test]
fn a_tab_on_the_left_scrolls_in_from_the_left() {
    let (backend, runtime, tabs, _pages) = paged();
    tabs.select(2);
    frame_after(&runtime, &backend, 400);
    tabs.select(0);
    assert_eq!(page(&backend, &tabs, 0).0, PAGE.offset(-300, 0));
    frame_after(&runtime, &backend, 400);
    assert_eq!(page(&backend, &tabs, 0), (PAGE, true));
    assert!(!page(&backend, &tabs, 2).1);
}

#[test]
fn a_layout_during_the_scroll_keeps_it_going() {
    let (backend, runtime, tabs, _pages) = paged();
    tabs.select(1);
    frame_after(&runtime, &backend, 90);
    let before = (page(&backend, &tabs, 0), page(&backend, &tabs, 1));

    tabs.relayout();
    assert_eq!((page(&backend, &tabs, 0), page(&backend, &tabs, 1)), before);

    frame_after(&runtime, &backend, 200);
    assert_eq!(page(&backend, &tabs, 1), (PAGE, true));
    assert!(!page(&backend, &tabs, 0).1);
}

#[test]
fn selecting_again_mid_scroll_lands_the_first_and_starts_the_next() {
    let (backend, runtime, tabs, _pages) = paged();
    tabs.select(1);
    frame_after(&runtime, &backend, 90);
    tabs.select(2);
    assert!(!page(&backend, &tabs, 0).1, "the first scroll is over");
    assert_eq!(page(&backend, &tabs, 1), (PAGE, true), "its page landed");
    assert_eq!(page(&backend, &tabs, 2).0, PAGE.offset(300, 0));

    frame_after(&runtime, &backend, 400);
    assert_eq!(page(&backend, &tabs, 2), (PAGE, true));
    assert!(!page(&backend, &tabs, 1).1);
    assert_eq!(backend.clip(tabs.id()), None);
}

#[test]
fn removing_a_page_mid_scroll_ends_it() {
    let (backend, runtime, tabs, _pages) = paged();
    tabs.select(2);
    frame_after(&runtime, &backend, 90);
    tabs.remove_page(0);
    assert_eq!(backend.clip(tabs.id()), None);
    assert_eq!(tabs.selected(), 1);
    assert_eq!(page(&backend, &tabs, 1), (PAGE, true));
    frame_after(&runtime, &backend, 400);
    assert_eq!(page(&backend, &tabs, 1), (PAGE, true));
}

#[test]
fn without_animation_the_page_changes_at_once() {
    let (backend, _core, ui) = setup();
    let tabs = Tabs::new(&ui, Rect::new(0, 0, 300, 120)).unwrap();
    let (a, b) = (label(&tabs, "a"), label(&tabs, "b"));
    let tabs = tabs.page("A", &[a.id()]).page("B", &[b.id()]);
    tabs.select(1);
    assert_eq!(page(&backend, &tabs, 1), (PAGE, true));
    assert!(!page(&backend, &tabs, 0).1);
    assert_eq!(backend.clip(tabs.id()), None);
    assert_eq!(backend.frame_requests(), 0);
}

#[test]
fn dropping_the_tabs_mid_scroll_leaves_nothing_behind() {
    let (backend, runtime, tabs, pages) = paged();
    tabs.select(1);
    frame_after(&runtime, &backend, 60);
    drop(tabs);
    drop(pages);
    let (asked, painted) = (backend.frame_requests(), backend.invalidations());
    frame_after(&runtime, &backend, 60);
    frame_after(&runtime, &backend, 60);
    assert_eq!(backend.invalidations(), painted, "nothing is repainted");
    assert!(
        backend.frame_requests() <= asked + 1,
        "and the frames stop at once"
    );
}
