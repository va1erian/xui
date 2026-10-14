#![forbid(unsafe_code)]

//! The limits of a form.
//!
//! A form is read from untrusted text and may be edited in memory, so what it
//! asks a build to make is bounded: its tree nests at most [`MAX_DEPTH`]
//! levels, a control array has at most [`MAX_ARRAY`] elements and a length is
//! within [`MAX_LENGTH`]. Each serves the reader (`load` rejects what exceeds
//! it, at its position) and the writer (`Form::to_ron` refuses it) alike, so a
//! form that is written can always be read back; the scan in
//! [`breaches`](super::breach) is what `Form::validate` reports and
//! `describe` refuses to build.

use std::cell::Cell;

use serde::de;

use super::Node;

/// The deepest a form's tree may nest: the root node is level 1, a node
/// directly inside it level 2, and so on. A `Group`'s content, a `Panel`'s
/// content and a `Tabs` page's content are each one level below the
/// container.
///
/// A form nested more deeply is not read (`load` reports the first node one
/// level past this), not written (`Form::to_ron` returns an error), not built,
/// and `Form::validate` reports it.
///
/// The value keeps the reader's stack use at what RON's default recursion
/// limit allowed (a debug build needs about 5 KiB of stack for each level of
/// RON nesting, and a `Tabs` chain nests eight per level).
pub const MAX_DEPTH: usize = 16;

/// The most elements a control array has: `array` is at most this, and an
/// element's `index` is below it.
///
/// An array of `n` elements builds `n` widgets, so the count of a file would
/// otherwise be a way to exhaust memory and time (a `Button(array:
/// 100000000)` took 770 MB to check). 1024 is far beyond a form's real
/// controls; more rows belong in a `ListView`.
pub const MAX_ARRAY: u32 = 1024;

/// The largest length, in design units, either way: a width, a gap, a
/// position. A length is finite and at most this many units from zero.
///
/// Layout adds lengths in 32-bit device pixels, so a length the file may write
/// as `1e38` would overflow it (a panic in a debug build); 32768 is the range
/// classic window coordinates had and is far beyond any form.
pub const MAX_LENGTH: f32 = 32_768.0;

/// The recursion limit the form's RON is read and written with. Eight is how
/// many RON levels the most nested node kind (a `Tabs` page) takes, plus a few
/// for the form itself, so the limit never trips before [`MAX_DEPTH`] does:
/// it only stops text no valid form can be.
pub(crate) const RECURSION_LIMIT: usize = 8 * MAX_DEPTH + 8;

/// What the reader says of a node past [`MAX_DEPTH`].
pub(crate) fn depth_message() -> String {
    format!("a form nests at most {MAX_DEPTH} levels deep")
}

thread_local! {
    /// How many nodes the reader on this thread is inside.
    static READING: Cell<usize> = const { Cell::new(0) };
}

/// One node being read: it counts towards [`MAX_DEPTH`] until dropped.
pub(super) struct Level;

impl Level {
    /// Enters one level, or fails with the position-less error the reader
    /// attaches its position to.
    pub(super) fn enter<E: de::Error>() -> Result<Level, E> {
        READING.with(|reading| {
            if reading.get() >= MAX_DEPTH {
                Err(E::custom(depth_message()))
            } else {
                reading.set(reading.get() + 1);
                Ok(Level)
            }
        })
    }
}

impl Drop for Level {
    fn drop(&mut self) {
        READING.with(|reading| reading.set(reading.get().saturating_sub(1)));
    }
}

impl Node {
    /// How deep the tree nests from this node: 1 for a node with nothing
    /// inside it. Walks the tree without recursing, so a tree of any depth is
    /// measured.
    pub fn depth(&self) -> usize {
        let mut deepest = 0;
        let mut pending = vec![(self, 1usize)];
        while let Some((node, level)) = pending.pop() {
            deepest = deepest.max(level);
            pending.extend(node.children().into_iter().map(|child| (child, level + 1)));
        }
        deepest
    }
}
