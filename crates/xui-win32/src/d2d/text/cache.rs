#![forbid(unsafe_code)]

//! Bounded least-recently-used caches of measured widths and laid-out text.

use std::collections::HashMap;

use super::layout::Layout;

/// Widths keyed by string, evicting the least recently used half in one sweep
/// when full, so a miss costs O(1) amortised and a hit allocates nothing.
pub(super) struct WidthCache {
    entries: HashMap<Box<str>, Entry>,
    clock: u64,
    capacity: usize,
}

struct Entry {
    width: f32,
    last_used: u64,
}

impl WidthCache {
    pub(super) fn new(capacity: usize) -> WidthCache {
        WidthCache {
            entries: HashMap::with_capacity(capacity),
            clock: 0,
            capacity,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn get(&mut self, text: &str) -> Option<f32> {
        self.clock += 1;
        let entry = self.entries.get_mut(text)?;
        entry.last_used = self.clock;
        Some(entry.width)
    }

    pub(super) fn insert(&mut self, text: &str, width: f32) {
        if self.entries.len() >= self.capacity {
            self.evict_older_half();
        }
        self.clock += 1;
        self.entries.insert(
            text.into(),
            Entry {
                width,
                last_used: self.clock,
            },
        );
    }

    fn evict_older_half(&mut self) {
        let mut stamps: Vec<u64> = self.entries.values().map(|e| e.last_used).collect();
        let middle = stamps.len() / 2;
        let (_, &mut cutoff, _) = stamps.select_nth_unstable(middle);
        self.entries.retain(|_, entry| entry.last_used >= cutoff);
    }
}

struct LayoutEntry {
    layout: Layout,
    last_used: u64,
}

/// Laid-out text keyed by `(text, wrap width)`, so a virtualized list of
/// repeated cell text lays each one out once instead of rebuilding a
/// DirectWrite layout per cell, per frame. Bounded like [`WidthCache`].
///
/// The text is the outer key, so a hit looks up by `&str` and allocates
/// nothing; the width is a `f32`'s bits inside, so two widths that are
/// bit-identical (`f32::INFINITY` is the single-line case) share an entry.
pub(super) struct LayoutCache {
    entries: HashMap<Box<str>, HashMap<u32, LayoutEntry>>,
    clock: u64,
    count: usize,
    capacity: usize,
}

impl LayoutCache {
    pub(super) fn new(capacity: usize) -> LayoutCache {
        LayoutCache {
            entries: HashMap::new(),
            clock: 0,
            count: 0,
            capacity,
        }
    }

    pub(super) fn get(&mut self, text: &str, width: f32) -> Option<&Layout> {
        self.clock += 1;
        let entry = self.entries.get_mut(text)?.get_mut(&width.to_bits())?;
        entry.last_used = self.clock;
        Some(&entry.layout)
    }

    pub(super) fn insert(&mut self, text: &str, width: f32, layout: Layout) {
        if self.count >= self.capacity {
            self.evict_older_half();
        }
        self.clock += 1;
        let replaced = self
            .entries
            .entry(text.into())
            .or_default()
            .insert(
                width.to_bits(),
                LayoutEntry {
                    layout,
                    last_used: self.clock,
                },
            )
            .is_none();
        if replaced {
            self.count += 1;
        }
    }

    fn evict_older_half(&mut self) {
        let mut stamps: Vec<u64> = self
            .entries
            .values()
            .flat_map(|by_width| by_width.values().map(|entry| entry.last_used))
            .collect();
        let middle = stamps.len() / 2;
        let (_, &mut cutoff, _) = stamps.select_nth_unstable(middle);
        for by_width in self.entries.values_mut() {
            by_width.retain(|_, entry| entry.last_used >= cutoff);
        }
        self.entries.retain(|_, by_width| !by_width.is_empty());
        self.count = self.entries.values().map(HashMap::len).sum();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_return_the_stored_width() {
        let mut cache = WidthCache::new(4);
        assert_eq!(cache.get("a"), None);
        cache.insert("a", 1.5);
        assert_eq!(cache.get("a"), Some(1.5));
    }

    #[test]
    fn stays_within_capacity() {
        let mut cache = WidthCache::new(8);
        for n in 0..100 {
            cache.insert(&n.to_string(), n as f32);
            assert!(cache.len() <= 8);
        }
    }

    #[test]
    fn evicts_the_least_recently_used() {
        let mut cache = WidthCache::new(4);
        for key in ["a", "b", "c", "d"] {
            cache.insert(key, 1.0);
        }
        // Touch "a" so it is the newest of the old entries.
        cache.get("a");
        cache.insert("e", 1.0);
        assert!(cache.get("a").is_some(), "the recently used entry survives");
        assert!(cache.get("b").is_none(), "the oldest entry is evicted");
        assert!(cache.get("e").is_some());
    }
}
