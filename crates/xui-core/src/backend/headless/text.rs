#![forbid(unsafe_code)]

//! The headless backend's text shaper: a deterministic approximation of a
//! monospace font, so a test can assert hit-test and selection geometry
//! without a platform font stack.

use crate::backend::text::{FontSpec, TextHit, TextLayout, TextShaper};
use crate::geometry::Rect;

/// Shapes text with a fixed advance and line height.
pub(crate) struct HeadlessShaper;

impl TextShaper for HeadlessShaper {
    fn layout(&self, text: &str, spec: &FontSpec, max_width: f32, dpi: u32) -> Box<dyn TextLayout> {
        let size_px = spec.size.to_px(dpi).value().max(1) as f32;
        Box::new(HeadlessLayout {
            text: text.to_owned(),
            char_width: size_px * 0.5,
            line_height: (size_px * 1.25).round().max(1.0),
            max_width,
        })
    }
}

/// A fixed-advance layout over the shaped text.
struct HeadlessLayout {
    text: String,
    char_width: f32,
    line_height: f32,
    max_width: f32,
}

impl HeadlessLayout {
    /// The byte ranges of the lines, a `\n` excluded and a character that
    /// would overflow `max_width` starting the next line.
    fn lines(&self) -> Vec<(usize, usize)> {
        let mut lines = Vec::new();
        let (mut start, mut width) = (0, 0.0);
        for (at, character) in self.text.char_indices() {
            if character == '\n' {
                lines.push((start, at));
                start = at + 1;
                width = 0.0;
                continue;
            }
            if width + self.char_width > self.max_width && at > start {
                lines.push((start, at));
                start = at;
                width = 0.0;
            }
            width += self.char_width;
        }
        lines.push((start, self.text.len()));
        lines
    }

    /// The chars on the line starting at `start` and ending before `end`.
    fn chars_in(&self, start: usize, end: usize) -> usize {
        self.text[start..end].chars().count()
    }

    /// The byte offset at `chars` into the line starting at `start`.
    fn byte_at(&self, start: usize, chars: usize) -> usize {
        self.text[start..]
            .char_indices()
            .nth(chars)
            .map_or(self.text.len(), |(at, _)| start + at)
    }
}

impl TextLayout for HeadlessLayout {
    fn width(&self) -> f32 {
        self.lines()
            .iter()
            .map(|(start, end)| self.chars_in(*start, *end) as f32 * self.char_width)
            .fold(0.0, f32::max)
    }

    fn height(&self) -> f32 {
        self.lines().len() as f32 * self.line_height
    }

    fn hit_test_point(&self, x: f32, y: f32) -> TextHit {
        let lines = self.lines();
        let width = self.width();
        let height = self.height();
        if lines.is_empty() {
            return TextHit {
                byte_index: 0,
                inside: false,
            };
        }
        let line =
            ((y / self.line_height).floor() as isize).clamp(0, lines.len() as isize - 1) as usize;
        let (start, end) = lines[line];
        let count = self.chars_in(start, end);
        let chars = (x / self.char_width).round().clamp(0.0, count as f32) as usize;
        TextHit {
            byte_index: self.byte_at(start, chars),
            inside: x >= 0.0 && y >= 0.0 && x <= width && y <= height,
        }
    }

    fn selection_rects(&self, byte_start: usize, byte_end: usize) -> Vec<Rect> {
        if byte_end <= byte_start {
            return Vec::new();
        }
        let mut rects = Vec::new();
        for (line, (start, end)) in self.lines().iter().enumerate() {
            let from = byte_start.max(*start);
            let to = byte_end.min(*end);
            if to <= from {
                continue;
            }
            let left = self.chars_in(*start, from) as f32 * self.char_width;
            let right = self.chars_in(*start, to) as f32 * self.char_width;
            let top = line as f32 * self.line_height;
            rects.push(Rect::new(
                left.round() as i32,
                top.round() as i32,
                right.round() as i32,
                (top + self.line_height).round() as i32,
            ));
        }
        rects
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
