//! Differential tests of damage-driven repainting: after every step of a random
//! sequence of node operations, the frame the backend repaints from its
//! damage list is pixel-for-pixel the frame a whole-window repaint draws.
//!
//! Two offscreen backends receive the same operations. Backend `A` is only
//! ever asked for the next frame, so it keeps repainting its persistent
//! surface from damage alone, and any damage it loses or underestimates
//! accumulates as stale pixels. Backend `B` is told to repaint the whole
//! window before each frame, which is the reference.
//!
//! The sequences are seeded and deterministic. `XUI_DAMAGE_SEEDS` raises the
//! number of seeds (the defaults are 25, of 36 operations each) for a longer
//! run.

mod model;

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use model::{H, KINDS, Look, Looks, Rng, W, checker, draw};
use xui_canvas::OffscreenBackend;
use xui_core::backend::{
    Backend, NodeKind, NodeSpec, Painter, ParentRef, PlatformSpec, WidgetId, WindowId,
};
use xui_core::geometry::Rect;
use xui_core::image::Image;
use xui_core::{Dip, Theme};

struct Node {
    id: WidgetId,
    /// The index of its look in the shared looks.
    index: usize,
    /// The node it was created in.
    parent: Option<usize>,
    alive: bool,
    container: bool,
}

struct Twin {
    a: Rc<OffscreenBackend>,
    b: Rc<OffscreenBackend>,
    wa: WindowId,
    wb: WindowId,
    looks: Looks,
    image: Rc<Image>,
    /// In creation order; ids are the same in both backends.
    nodes: Vec<Node>,
}

fn spec() -> PlatformSpec {
    PlatformSpec::new("differential").size(Dip(W as f32), Dip(H as f32))
}

impl Twin {
    /// Two windows at `dpi`, laid out in device pixels.
    fn new(dpi: u32) -> Twin {
        let a = Rc::new(OffscreenBackend::with_dpi(dpi));
        let b = Rc::new(OffscreenBackend::with_dpi(dpi));
        let wa = a.open_window(&spec()).unwrap();
        let wb = b.open_window(&spec()).unwrap();
        Twin {
            a,
            b,
            wa,
            wb,
            looks: Rc::new(RefCell::new(Vec::new())),
            image: Rc::new(checker()),
            nodes: Vec::new(),
        }
    }

    /// A painter for look `index` on `id`, reading the shared looks and its
    /// own backend's text.
    fn painter(&self, backend: &Rc<OffscreenBackend>, index: usize, id: WidgetId) -> Painter {
        let looks = Rc::clone(&self.looks);
        let image = Rc::clone(&self.image);
        let backend: Weak<OffscreenBackend> = Rc::downgrade(backend);
        Rc::new(move |canvas| {
            let text = backend.upgrade().map(|b| b.text(id)).unwrap_or_default();
            let look = looks.borrow()[index].clone();
            let bounds = canvas.bounds();
            draw(canvas, bounds, &look, &text, &image);
        })
    }

    fn live(&self) -> Vec<usize> {
        (0..self.nodes.len())
            .filter(|at| self.nodes[*at].alive)
            .collect()
    }

    fn containers(&self) -> Vec<usize> {
        self.live()
            .into_iter()
            .filter(|at| self.nodes[*at].container)
            .collect()
    }

    /// A random live node, if any.
    fn pick(&self, rng: &mut Rng) -> Option<usize> {
        let live = self.live();
        (!live.is_empty()).then(|| live[rng.below(live.len())])
    }

    fn create(&mut self, rng: &mut Rng, container: bool) -> String {
        let parent = {
            let containers = self.containers();
            (!containers.is_empty() && rng.below(3) != 0)
                .then(|| containers[rng.below(containers.len())])
        };
        let left = rng.range(-30, W);
        let top = rng.range(-30, H);
        let size = if container {
            (rng.range(40, 140), rng.range(30, 110))
        } else {
            (rng.range(6, 70), rng.range(6, 50))
        };
        let rect = Rect::new(left, top, left + size.0, top + size.1);
        let look = Look {
            kind: rng.below(KINDS),
            color: rng.color(),
            reach: rng.range(1, 6),
            caret: 0,
        };
        let index = {
            let mut looks = self.looks.borrow_mut();
            looks.push(look.clone());
            looks.len() - 1
        };
        let mut created = None;
        for (backend, window) in [(&self.a, self.wa), (&self.b, self.wb)] {
            let parent_ref = match parent {
                Some(at) => ParentRef::Widget(self.nodes[at].id),
                None => ParentRef::Window(window),
            };
            let kind = if container {
                NodeKind::Container
            } else {
                NodeKind::Custom
            };
            let id = backend
                .create(parent_ref, &NodeSpec::new(kind, rect))
                .expect("a node");
            // A container paints as well half the time; plain nodes always.
            if !container || index % 2 == 0 {
                backend.set_painter(id, self.painter(backend, index, id));
            }
            created = Some(id);
        }
        let id = created.unwrap();
        self.nodes.push(Node {
            id,
            index,
            parent,
            alive: true,
            container,
        });
        format!("create {id:?} {rect:?} kind {} in {parent:?}", look.kind)
    }

    fn destroy(&mut self, rng: &mut Rng) -> String {
        let Some(at) = self.pick(rng) else {
            return "destroy nothing".into();
        };
        let id = self.nodes[at].id;
        for backend in [&self.a, &self.b] {
            backend.destroy(id);
        }
        // The node and everything created inside it, directly or not.
        let mut gone = vec![at];
        loop {
            let before = gone.len();
            for candidate in 0..self.nodes.len() {
                if !gone.contains(&candidate)
                    && self.nodes[candidate]
                        .parent
                        .is_some_and(|parent| gone.contains(&parent))
                {
                    gone.push(candidate);
                }
            }
            if gone.len() == before {
                break;
            }
        }
        for at in gone {
            self.nodes[at].alive = false;
        }
        format!("destroy {id:?}")
    }

    fn moves(&mut self, rng: &mut Rng) -> String {
        let mut batch = Vec::new();
        for _ in 0..1 + rng.below(3) {
            let Some(at) = self.pick(rng) else {
                break;
            };
            let id = self.nodes[at].id;
            let current = self.a.bounds(id);
            let rect = match rng.below(3) {
                // A nudge, the commonest animation step.
                0 => current.offset(rng.range(-3, 4), rng.range(-3, 4)),
                // A resize, sometimes to nothing along one axis or both.
                1 => {
                    let mut size = (rng.range(4, 90), rng.range(4, 70));
                    match rng.below(5) {
                        0 => size.0 = 0,
                        1 => size.1 = 0,
                        2 => size = (0, 0),
                        _ => {}
                    }
                    Rect::new(
                        current.left,
                        current.top,
                        current.left + size.0,
                        current.top + size.1,
                    )
                }
                // Anywhere, partly or wholly outside the window.
                _ => {
                    let left = rng.range(-60, W + 20);
                    let top = rng.range(-60, H + 20);
                    Rect::new(left, top, left + rng.range(4, 90), top + rng.range(4, 70))
                }
            };
            batch.push((id, rect));
        }
        self.a.apply_moves(self.wa, &batch);
        self.b.apply_moves(self.wb, &batch);
        format!("move {batch:?}")
    }

    fn visibility(&mut self, rng: &mut Rng) -> String {
        let Some(at) = self.pick(rng) else {
            return "show nothing".into();
        };
        let id = self.nodes[at].id;
        let visible = rng.below(2) == 0;
        for backend in [&self.a, &self.b] {
            backend.set_visible(id, visible);
        }
        format!("visible {id:?} {visible}")
    }

    fn raise(&mut self, rng: &mut Rng) -> String {
        let Some(at) = self.pick(rng) else {
            return "raise nothing".into();
        };
        let id = self.nodes[at].id;
        for backend in [&self.a, &self.b] {
            backend.raise(id);
        }
        format!("raise {id:?}")
    }

    /// Changes what a node draws and tells the backend, the way a widget does:
    /// a whole-node invalidate, or the rectangle that changed.
    fn restyle(&mut self, rng: &mut Rng) -> String {
        let Some(at) = self.pick(rng) else {
            return "restyle nothing".into();
        };
        let (id, index) = (self.nodes[at].id, self.nodes[at].index);
        let bounds = self.a.bounds(id);
        let local_width = bounds.width();
        let local_height = bounds.height();
        let kind = self.looks.borrow()[index].kind;
        let change = rng.below(3);
        if kind == 8 && change != 0 {
            // The caret moves: only the old and new caret rectangles change.
            let old = self.looks.borrow()[index].caret;
            let new = rng.range(0, local_width.max(4));
            self.looks.borrow_mut()[index].caret = new;
            // The painter keeps the caret inside the node.
            let around = |at: i32| {
                let at = at.min(local_width - 3).max(0);
                Rect::new(at, 0, at + 3, local_height)
            };
            for backend in [&self.a, &self.b] {
                backend.invalidate_rect(id, around(old));
                backend.invalidate_rect(id, around(new));
            }
            return format!("caret {id:?} {old} -> {new}");
        }
        {
            let mut looks = self.looks.borrow_mut();
            looks[index].color = rng.color();
            looks[index].reach = rng.range(1, 6);
        }
        for backend in [&self.a, &self.b] {
            backend.invalidate(id);
        }
        format!("restyle {id:?}")
    }

    fn text(&mut self, rng: &mut Rng) -> String {
        let Some(at) = self.pick(rng) else {
            return "text nothing".into();
        };
        let id = self.nodes[at].id;
        let text = ["Alpha", "Be", "Gamma ray", "", "xui Ag"][rng.below(5)];
        for backend in [&self.a, &self.b] {
            backend.set_text(id, text);
        }
        format!("text {id:?} {text:?}")
    }

    fn clip(&mut self, rng: &mut Rng) -> String {
        let containers = self.containers();
        if containers.is_empty() {
            return "clip nothing".into();
        }
        let id = self.nodes[containers[rng.below(containers.len())]].id;
        let clip = (rng.below(4) != 0).then(|| {
            let left = rng.range(-10, 50);
            let top = rng.range(-10, 40);
            Rect::new(left, top, left + rng.range(0, 90), top + rng.range(0, 70))
        });
        for backend in [&self.a, &self.b] {
            backend.set_clip(id, clip);
        }
        format!("clip {id:?} {clip:?}")
    }

    fn repaint_with(&mut self, rng: &mut Rng) -> String {
        let Some(at) = self.pick(rng) else {
            return "painter nothing".into();
        };
        let (id, index) = (self.nodes[at].id, self.nodes[at].index);
        self.looks.borrow_mut()[index].kind = rng.below(KINDS);
        self.looks.borrow_mut()[index].color = rng.color();
        let painter_a = self.painter(&self.a, index, id);
        let painter_b = self.painter(&self.b, index, id);
        self.a.set_painter(id, painter_a);
        self.b.set_painter(id, painter_b);
        format!("painter {id:?}")
    }

    fn theme(&mut self, rng: &mut Rng) -> String {
        let theme = match rng.below(3) {
            0 => Theme::light(),
            1 => Theme::dark(),
            _ => Theme::midnight(),
        };
        self.a.set_theme(self.wa, &theme);
        self.b.set_theme(self.wb, &theme);
        "theme".into()
    }

    /// One random operation.
    fn step(&mut self, rng: &mut Rng) -> String {
        match rng.below(20) {
            0..=3 => self.create(rng, false),
            4 => self.create(rng, true),
            5 => self.destroy(rng),
            6..=8 => self.moves(rng),
            9 => self.visibility(rng),
            10 => self.raise(rng),
            11..=13 => self.restyle(rng),
            14 => self.text(rng),
            15 => self.clip(rng),
            16 => self.repaint_with(rng),
            17 => self.theme(rng),
            // A frame in which nothing changed.
            _ => "idle".into(),
        }
    }

    /// Asserts the region-repainted frame equals the whole-window reference.
    fn check(&self, seed: u64, step: usize, what: &str) {
        let (got, damage) = self
            .a
            .present(self.wa, |frame| {
                (frame.pixels.to_vec(), frame.damage.to_vec())
            })
            .expect("a frame");
        self.b.invalidate_window(self.wb);
        let want = self.b.render(self.wb).expect("a reference frame");
        if got != want.pixels {
            let at = got
                .iter()
                .zip(&want.pixels)
                .position(|(a, b)| a != b)
                .unwrap();
            let (x, y) = ((at / 4) as i32 % W, (at / 4) as i32 / W);
            let mut nodes = String::new();
            for node in self.live() {
                let node = &self.nodes[node];
                let look = self.looks.borrow()[node.index].clone();
                nodes.push_str(&format!(
                    "
  {:?} kind {} {:?} parent {:?} visible-container {}",
                    node.id,
                    look.kind,
                    self.a.bounds(node.id),
                    node.parent,
                    node.container
                ));
            }
            panic!(
                "seed {seed}, step {step} ({what}): region repaint differs from a full repaint 
                 at ({x}, {y}); damage was {damage:?}; nodes:{nodes}",
            );
        }
    }
}

/// How many seeds a test runs: `default`, or `XUI_DAMAGE_SEEDS` for a longer
/// run. The defaults keep each test to a few seconds in a debug build.
fn seeds(default: u64) -> u64 {
    std::env::var("XUI_DAMAGE_SEEDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// Random sequences of every operation at `dpi`: region repaint equals full
/// repaint after every step.
fn random_sequences(dpi: u32, seeds: u64) {
    for seed in 1..=seeds {
        let mut rng = Rng::new(seed.wrapping_add(u64::from(dpi) << 32));
        let mut twin = Twin::new(dpi);
        for step in 0..36 {
            let what = twin.step(&mut rng);
            twin.check(seed, step, &what);
        }
    }
}

#[test]
fn region_repaint_equals_full_repaint_over_random_operation_sequences() {
    random_sequences(96, seeds(25));
}

/// The same at the DPIs where a ring drawn in design units is not a whole
/// number of pixels and the overdraw margin has to follow the scale.
#[test]
fn region_repaint_equals_full_repaint_at_120_144_and_192_dpi() {
    for dpi in [120, 144, 192] {
        random_sequences(dpi, seeds(25) / 5);
    }
}

/// The same, with many overlapping siblings in a few nested containers and
/// mostly small moves: the animation workload.
#[test]
fn region_repaint_equals_full_repaint_for_overlapping_siblings_moving_a_little() {
    for seed in 1..=seeds(25) {
        let mut rng = Rng::new(seed ^ 0xA5A5);
        let mut twin = Twin::new(96);
        for _ in 0..3 {
            twin.create(&mut rng, true);
        }
        for _ in 0..14 {
            twin.create(&mut rng, false);
        }
        twin.check(seed, 0, "setup");
        for step in 1..=30 {
            let what = match rng.below(4) {
                0 | 1 => twin.moves(&mut rng),
                2 => twin.restyle(&mut rng),
                _ => twin.raise(&mut rng),
            };
            twin.check(seed, step, &what);
        }
    }
}
