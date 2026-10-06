//! What Wikipedia needs drawn (va1erian/lazyos#632): CSS `opacity`, SVG
//! pictures (`<img>` and `background-image`), the text of inline
//! `::after` pseudo-elements, and spacing around a no-break space and an
//! italic run. Each page is a `data:` URL loaded into an offscreen
//! `NetSurfView` and judged by its pixels.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use xui_canvas::snapshot::{Snapshot, render_with};
use xui_core::Dip;
use xui_core::app::{App, Ui};
use xui_core::image::Image;
use xui_netsurf::{NetSurfView, NetSurfViewEvent};

/// How long a page may take before the test fails instead of hanging.
const WATCHDOG: Duration = Duration::from_secs(30);

struct Page {
    view: NetSurfView<()>,
    events: Rc<RefCell<Vec<NetSurfViewEvent>>>,
}

impl App for Page {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {
        let events = self.view.update();
        self.events.borrow_mut().extend(events);
    }
}

/// `html` as a `data:` URL (NetSurf reads those itself).
fn data_url(html: &str) -> String {
    let mut url = String::from("data:text/html;charset=utf-8,");
    for byte in html.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~ /:=<>\"'!;(){}".contains(&byte) {
            url.push(byte as char);
        } else {
            url.push_str(&format!("%{byte:02X}"));
        }
    }
    url
}

/// `html` drawn in a 400x200 view at 96 dpi, once it finished loading.
fn render(html: &str) -> Image {
    let url = data_url(html);
    let events = Rc::new(RefCell::new(Vec::new()));
    let settled = Rc::new(RefCell::new(false));
    let image = {
        let (events, settled) = (Rc::clone(&events), Rc::clone(&settled));
        let watched = Rc::clone(&events);
        render_with(
            Snapshot::new(Dip(400.0), Dip(200.0)).dpi(96),
            move |ui| {
                let view = NetSurfView::new(ui, ui.client_rect(), &url, || ())?;
                Ok(Page { view, events })
            },
            move |stage| {
                let start = Instant::now();
                while start.elapsed() < WATCHDOG {
                    let _ = stage.ui().capture();
                    stage.emit(());
                    let idle = watched.borrow().iter().rev().find_map(|e| match e {
                        NetSurfViewEvent::LoadingChanged(on) => Some(!on),
                        _ => None,
                    });
                    if idle == Some(true) {
                        // One more round so the newest frame is on screen.
                        std::thread::sleep(Duration::from_millis(300));
                        stage.emit(());
                        let _ = stage.ui().capture();
                        *settled.borrow_mut() = true;
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            },
        )
        .expect("render offscreen")
    };
    assert!(
        *settled.borrow(),
        "the page did not settle: {:?}",
        events.borrow()
    );
    image
}

fn pixel(image: &Image, x: u32, y: u32) -> [u8; 3] {
    let p = image.pixel(x, y).expect("a pixel inside the snapshot");
    [p[0], p[1], p[2]]
}

fn near(px: [u8; 3], rgb: [u8; 3], tolerance: u8) -> bool {
    px.iter().zip(rgb).all(|(&a, b)| a.abs_diff(b) <= tolerance)
}

const WHITE: [u8; 3] = [255, 255, 255];

/// The columns of row band `y0..y1` holding ink (anything not white).
fn inked_columns(image: &Image, y0: u32, y1: u32) -> Vec<bool> {
    (0..image.width())
        .map(|x| (y0..y1).any(|y| !near(pixel(image, x, y), WHITE, 60)))
        .collect()
}

/// The first and last inked column of a band, if any.
fn ink_extent(image: &Image, y0: u32, y1: u32) -> Option<(u32, u32)> {
    let cols = inked_columns(image, y0, y1);
    let first = cols.iter().position(|&c| c)?;
    let last = cols.iter().rposition(|&c| c)?;
    Some((first as u32, last as u32))
}

/// The runs of at least `min` blank columns strictly between the first and
/// last inked column of a band: the gaps between words.
fn gaps(image: &Image, y0: u32, y1: u32, min: usize) -> usize {
    let cols = inked_columns(image, y0, y1);
    let (Some(first), Some(last)) = (cols.iter().position(|&c| c), cols.iter().rposition(|&c| c))
    else {
        return 0;
    };
    let mut count = 0;
    let mut run = 0;
    for &inked in &cols[first..=last] {
        if inked {
            if run >= min {
                count += 1;
            }
            run = 0;
        } else {
            run += 1;
        }
    }
    count
}

const PAGE_STYLE: &str = "<style>body { margin: 0; background: #fff; font-size: 16px }
.box { position: absolute; width: 100px; height: 40px }</style>";

#[test]
fn opacity_hides_and_fades_boxes() {
    let image = render(&format!(
        r#"<!DOCTYPE html><html><head>{PAGE_STYLE}</head><body>
<div class="box" style="left: 0; top: 0; background: #00f"></div>
<div class="box" style="left: 0; top: 0; background: #f00; opacity: 0"></div>
<div class="box" style="left: 0; top: 50px; background: #000; opacity: 0.5"></div>
<div class="box" style="left: 0; top: 100px; opacity: 0.5"><div style="opacity: 0.5; background: #000; height: 40px"></div></div>
<div style="position: absolute; left: 150px; top: 0; opacity: 0"><span style="background: #0f0">hidden text</span></div>
<input type="checkbox" style="position: absolute; left: 150px; top: 50px; width: 40px; height: 40px; opacity: 0">
</body></html>"#
    ));
    assert!(
        near(pixel(&image, 10, 10), [0, 0, 255], 10),
        "opacity 0 drew: {:?}",
        pixel(&image, 10, 10)
    );
    assert!(
        near(pixel(&image, 10, 70), [128, 128, 128], 12),
        "half black: {:?}",
        pixel(&image, 10, 70)
    );
    // Opacity multiplies down the tree: 0.5 x 0.5 of black.
    assert!(
        near(pixel(&image, 10, 120), [191, 191, 191], 12),
        "quarter black: {:?}",
        pixel(&image, 10, 120)
    );
    assert_eq!(
        ink_extent(&image, 0, 30).map(|(_, last)| last < 140),
        Some(true),
        "hidden text drew"
    );
    for (x, y) in [(150, 50), (170, 70), (189, 89)] {
        assert!(
            near(pixel(&image, x, y), WHITE, 0),
            "the hidden checkbox drew at {x},{y}"
        );
    }
}

const SQUARE_SVG: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='40' height='20' viewBox='0 0 4 2'%3E%3Crect width='2' height='2' fill='%2300ff00'/%3E%3Crect x='2' width='2' height='2' fill='%23ff00ff'/%3E%3C/svg%3E";

#[test]
fn svg_pictures_draw_at_their_size() {
    let image = render(&format!(
        r#"<!DOCTYPE html><html><head>{PAGE_STYLE}</head><body>
<img src="{SQUARE_SVG}" style="position: absolute; left: 0; top: 0">
<style>.tiles {{ background: url("{SQUARE_SVG}") repeat-x }}</style>
<div class="tiles" style="position: absolute; left: 0; top: 40px; width: 80px; height: 20px"></div>
<img src="{SQUARE_SVG}" style="position: absolute; left: 0; top: 80px; width: 80px; height: 40px">
</body></html>"#
    ));
    // <img>: 40x20 from the SVG's width and height, left half green.
    assert!(
        near(pixel(&image, 5, 5), [0, 255, 0], 10),
        "{:?}",
        pixel(&image, 5, 5)
    );
    assert!(
        near(pixel(&image, 35, 15), [255, 0, 255], 10),
        "{:?}",
        pixel(&image, 35, 15)
    );
    assert!(
        near(pixel(&image, 45, 5), WHITE, 10),
        "wider than 40px: {:?}",
        pixel(&image, 45, 5)
    );
    // background-image, tiled: the second tile starts green at x = 40.
    assert!(
        near(pixel(&image, 45, 50), [0, 255, 0], 10),
        "{:?}",
        pixel(&image, 45, 50)
    );
    assert!(
        near(pixel(&image, 75, 50), [255, 0, 255], 10),
        "{:?}",
        pixel(&image, 75, 50)
    );
    // Scaled up twice, the edge between the halves stays sharp.
    assert!(
        near(pixel(&image, 38, 100), [0, 255, 0], 10),
        "{:?}",
        pixel(&image, 38, 100)
    );
    assert!(
        near(pixel(&image, 41, 100), [255, 0, 255], 10),
        "{:?}",
        pixel(&image, 41, 100)
    );
}

/// The ink extent of `body`, one line placed at 10,10.
fn line_extent(body: &str) -> (u32, u32) {
    let image = render(&format!(
        r#"<!DOCTYPE html><html><head>{PAGE_STYLE}<style>
.line {{ position: absolute; left: 10px; top: 10px; white-space: nowrap }}
.hlist ul {{ margin: 0; padding: 0 }}
.hlist li {{ display: inline }}
.hlist li::after {{ content: " · "; font-weight: bold }}
.hlist li:last-child::after {{ content: none }}
.tag::before {{ content: "[" attr(data-n) "] " }}
</style></head><body><div class="line">{body}</div></body></html>"#
    ));
    ink_extent(&image, 5, 35).expect("the line drew")
}

#[test]
fn inline_after_text_separates_list_items() {
    let generated =
        line_extent(r#"<div class="hlist"><ul><li>aa</li><li>bb</li><li>cc</li></ul></div>"#);
    let written = line_extent("aa <b>·</b> bb <b>·</b> cc");
    let bare = line_extent("aabbcc");
    assert!(
        generated.1.abs_diff(written.1) <= 3,
        "generated {generated:?} vs written {written:?}"
    );
    assert!(
        generated.1 > bare.1 + 20,
        "no separators: {generated:?} vs {bare:?}"
    );
}

#[test]
fn before_text_with_attr_and_none_is_respected() {
    let generated = line_extent(r#"<span class="tag" data-n="12">x</span>"#);
    let written = line_extent("[12] x");
    assert!(
        generated.1.abs_diff(written.1) <= 3,
        "{generated:?} vs {written:?}"
    );
    // `content: none` on the last item: nothing after "cc".
    let last = line_extent(r#"<div class="hlist"><ul><li>cc</li></ul></div>"#);
    let plain = line_extent("cc");
    assert!(last.1.abs_diff(plain.1) <= 1, "{last:?} vs {plain:?}");
}

#[test]
fn a_no_break_space_is_as_wide_as_a_space() {
    let nbsp = line_extent("11&nbsp;days");
    let space = line_extent("11 days");
    assert!(
        nbsp.1.abs_diff(space.1) <= 1,
        "11&nbsp;days {nbsp:?} vs 11 days {space:?}"
    );
}

#[test]
fn a_space_after_an_italic_run_stays() {
    let image = render(&format!(
        r#"<!DOCTYPE html><html><head>{PAGE_STYLE}<style>p {{ margin: 0; position: absolute; left: 10px; top: 10px }}</style></head>
<body><p><i>Anno Domini</i> (AD)</p></body></html>"#
    ));
    // Two gaps between three words: after "Anno" and after "Domini".
    assert_eq!(
        gaps(&image, 5, 35, 3),
        2,
        "the space after the italic run is lost"
    );
}
