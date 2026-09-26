//! A shelf-packed texture atlas, and the reason the GPU path has one.
//!
//! Text is the expensive half of a GPU backend, and it is expensive for
//! one reason: a glyph is a bitmap, and uploading it per frame is worse
//! than the software renderer's problem, not better. The atlas is what
//! turns "rasterise every glyph every frame" into "rasterise every glyph
//! once" — `FontStack::place_run` shapes each frame, `FontStack::rasterize`
//! runs only on a miss.
//!
//! **Shelves, not a general packer.** Glyphs at one size are all about
//! one height, so a row-based packer wastes almost nothing on the input
//! this actually gets, and it is thirty lines instead of three hundred.
//! An application whose atlas fills is handled by [`Atlas::reset`] and a
//! rebuild of the frame, not by a better packer — and one whose frame
//! does not fit an *empty* atlas by a larger one, which the renderer
//! makes (M51/T51.1; the packer is not what ran out, `docs/results-gpu.md`
//! §4).

use std::collections::HashMap;
use std::hash::Hash;

/// Where one entry sits, in texels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Slot {
    /// The entry's uv rect in an atlas of `size` texels square.
    pub fn uv(&self, size: u32) -> [f32; 4] {
        let s = size as f32;
        [self.x as f32 / s, self.y as f32 / s, (self.x + self.w) as f32 / s, (self.y + self.h) as f32 / s]
    }
}

/// A square atlas of `size` texels, packed in shelves.
pub struct Atlas<K> {
    size: u32,
    /// Top of the shelf being filled, and its height.
    shelf_y: u32,
    shelf_h: u32,
    /// Next free x on that shelf.
    pen: u32,
    map: HashMap<K, Slot>,
}

/// One texel of gutter between entries.
///
/// The sampler is `Nearest` and every uv lands on a texel centre, so
/// nothing should bleed — but a glyph bitmap abutting its neighbour is
/// the kind of thing that goes wrong on one driver and not another, and
/// a texel per glyph is not a cost worth arguing about.
const GUTTER: u32 = 1;

impl<K: Eq + Hash> Atlas<K> {
    pub fn new(size: u32) -> Self {
        Atlas { size, shelf_y: 0, shelf_h: 0, pen: 0, map: HashMap::new() }
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn get(&self, key: &K) -> Option<Slot> {
        self.map.get(key).copied()
    }

    /// Forget everything. The caller must re-upload whatever it still
    /// needs; nothing here holds pixels.
    pub fn reset(&mut self) {
        self.map.clear();
        self.shelf_y = 0;
        self.shelf_h = 0;
        self.pen = 0;
    }

    /// Reserve `w` x `h` for `key`. `None` means the atlas is full —
    /// which the caller answers with [`Atlas::reset`] and a retry, then
    /// with a larger atlas, and drops a glyph only at the device's texture
    /// limit, where it counts the drop (M51/T51.1; until then this said a
    /// glyph was never dropped, and one was, silently).
    pub fn insert(&mut self, key: K, w: u32, h: u32) -> Option<Slot> {
        if w == 0 || h == 0 || w > self.size || h > self.size {
            return None;
        }
        if self.pen + w > self.size {
            // Next shelf.
            self.shelf_y += self.shelf_h + GUTTER;
            self.shelf_h = 0;
            self.pen = 0;
        }
        if self.shelf_y + h > self.size {
            return None;
        }
        let slot = Slot { x: self.pen, y: self.shelf_y, w, h };
        self.pen += w + GUTTER;
        self.shelf_h = self.shelf_h.max(h);
        self.map.insert(key, slot);
        Some(slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_do_not_overlap_and_stay_inside() {
        let mut a: Atlas<u32> = Atlas::new(64);
        let mut slots = Vec::new();
        let mut k = 0;
        // Ragged sizes, so shelves are not all the same height.
        while let Some(s) = a.insert(k, 7 + (k % 5), 9 + (k % 3)) {
            assert!(s.x + s.w <= 64 && s.y + s.h <= 64, "escaped the atlas: {s:?}");
            slots.push(s);
            k += 1;
            if k > 100 {
                break;
            }
        }
        assert!(slots.len() > 20, "packed only {}", slots.len());
        for (i, a) in slots.iter().enumerate() {
            for b in &slots[i + 1..] {
                let disjoint = a.x + a.w <= b.x
                    || b.x + b.w <= a.x
                    || a.y + a.h <= b.y
                    || b.y + b.h <= a.y;
                assert!(disjoint, "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn a_full_atlas_refuses_rather_than_wrapping() {
        let mut a: Atlas<u32> = Atlas::new(16);
        assert!(a.insert(0, 16, 16).is_some());
        assert!(a.insert(1, 4, 4).is_none(), "there is no room and none must be invented");
        a.reset();
        assert!(a.insert(2, 4, 4).is_some(), "a reset atlas takes entries again");
        assert!(a.get(&0).is_none(), "and has forgotten the old ones");
    }

    #[test]
    fn an_entry_larger_than_the_atlas_is_refused_not_clamped() {
        let mut a: Atlas<u32> = Atlas::new(16);
        assert!(a.insert(0, 17, 4).is_none());
        assert!(a.insert(1, 4, 0).is_none(), "a zero dimension has nothing to store");
    }
}
