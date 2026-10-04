//! **A display list as bytes whose meaning does not depend on the build**
//! — M37/T37.0 (`docs/plan-window-reload.md` §2.3).
//!
//! A hot-reloaded library and the loader that drew its frames are two
//! compilations. `repr(Rust)` promises no layout across compilations, so
//! a `Cmd` copied as memory from one would be read as garbage by the
//! other the first time either side's compiler chose differently. This
//! encoding is explicit instead: a version byte, then per command a tag
//! and each field little-endian, then the text. Decoding checks every
//! length and every text range, so a truncated or foreign buffer answers
//! `None` rather than a list that points outside its own text.
//!
//! **The tags are append-only.** A new command takes the next number; a
//! removed one keeps its number unused. That is what lets two builds
//! disagree about the *set* of commands and still agree about every
//! command they share.
//!
//! **The window's reload boundary does not use it.** It was measured
//! as the byte option of `docs/plan-window-reload.md` §2.3 and lost to
//! borrowed pointers behind an ABI stamp: it costs more than the frame
//! arm's spread on three of four showcase entries
//! (`docs/step4-hot-reload.md`, T37.0). It stays because it is that
//! measurement's other arm, and because a list that must outlive or leave
//! its process (a trace, a recording) needs exactly this.

use alloc::vec::Vec;

use crate::display::{Cmd, Color, DisplayList, ImageId, Material, Rect, Stops, MAX_STOPS};
use crate::desc::Point;

/// The encoding's version. Bumped when a field changes meaning or a
/// command gains one, never for an appended tag.
///
/// 2 since M80/T80.1: `Cmd::Text` carries `line_height`, one byte after
/// its weight. A version-1 buffer has no such byte, so it is refused
/// rather than read one byte out of step.
///
/// 3 since M82/T82.1: a gradient stop's offset is one byte in 1/255
/// steps, not an `f32` ([`Stops`]). A version-2 gradient is 12 B longer,
/// and a version-2 buffer with no gradient at all has the same layout, so
/// only this byte refuses it.
pub const VERSION: u8 = 3;

/// Append `list`'s encoding to `out`.
pub fn encode(list: &DisplayList, out: &mut Vec<u8>) {
    let mut w = W(out);
    w.u8(VERSION);
    match list.bounds {
        Some(r) => {
            w.u8(1);
            w.rect(r);
        }
        None => w.u8(0),
    }
    w.u32(list.cmds.len() as u32);
    for c in &list.cmds {
        match *c {
            Cmd::Rect { rect, color } => {
                w.u8(0);
                w.rect(rect);
                w.color(color);
            }
            Cmd::Text { rect, text, size, weight, line_height, color } => {
                w.u8(1);
                w.rect(rect);
                w.u32(text.0);
                w.u32(text.1);
                w.f32(size);
                w.f32(weight);
                w.u8(line_height);
                w.color(color);
            }
            Cmd::RoundedRect { rect, color, radius } => {
                w.u8(2);
                w.rect(rect);
                w.color(color);
                w.f32(radius);
            }
            Cmd::StrokedRect { rect, color, radius, width } => {
                w.u8(3);
                w.rect(rect);
                w.color(color);
                w.f32(radius);
                w.f32(width);
            }
            Cmd::Shadow { rect, color, radius, spread } => {
                w.u8(4);
                w.rect(rect);
                w.color(color);
                w.f32(radius);
                w.f32(spread);
            }
            Cmd::Line { a, b, width, color } => {
                w.u8(5);
                w.f32(a.x);
                w.f32(a.y);
                w.f32(b.x);
                w.f32(b.y);
                w.f32(width);
                w.color(color);
            }
            Cmd::Gradient { rect, stops, angle } => {
                w.u8(6);
                w.rect(rect);
                w.u8(stops.len);
                for i in 0..MAX_STOPS {
                    w.u8(stops.at[i]);
                    w.color(stops.color[i]);
                }
                w.f32(angle);
            }
            Cmd::Image { rect, id, tint } => {
                w.u8(7);
                w.rect(rect);
                w.u32(id.0);
                w.color(tint);
            }
            Cmd::Glass { rect, radius, material: m } => {
                w.u8(8);
                w.rect(rect);
                w.f32(radius);
                w.f32(m.blur);
                w.color(m.tint);
                w.f32(m.rim);
                w.f32(m.refraction);
                w.f32(m.specular);
                w.f32(m.light);
                w.u8(m.shadow);
                w.i16(m.morph_x);
                w.i16(m.morph_y);
                w.u8(m.adapt);
                w.color(m.tint_over_light);
                w.u8(m.dispersion);
                w.u8(m.saturation);
            }
            Cmd::PushClip { rect } => {
                w.u8(9);
                w.rect(rect);
            }
            Cmd::PopClip => w.u8(10),
        }
    }
    w.u32(list.text.len() as u32);
    w.0.extend_from_slice(list.text.as_bytes());
}

/// Decode what [`encode`] wrote, into `list` (cleared first, so its
/// allocations are reused frame to frame). `None` for a buffer of
/// another version, a truncated one, an unknown tag, text that is not
/// UTF-8, or a text range outside the text.
pub fn decode_into(bytes: &[u8], list: &mut DisplayList) -> Option<()> {
    list.cmds.clear();
    list.text.clear();
    let mut r = R(bytes);
    if r.u8()? != VERSION {
        return None;
    }
    list.bounds = match r.u8()? {
        0 => None,
        1 => Some(r.rect()?),
        _ => return None,
    };
    let n = r.u32()? as usize;
    // Every command is at least one byte: a count past the buffer is a
    // lie, and reserving for it would be an allocation a foreign buffer
    // chose.
    if n > r.0.len() {
        return None;
    }
    list.cmds.reserve(n);
    for _ in 0..n {
        let c = match r.u8()? {
            0 => Cmd::Rect { rect: r.rect()?, color: r.color()? },
            1 => Cmd::Text {
                rect: r.rect()?,
                text: (r.u32()?, r.u32()?),
                size: r.f32()?,
                weight: r.f32()?,
                line_height: r.u8()?,
                color: r.color()?,
            },
            2 => Cmd::RoundedRect { rect: r.rect()?, color: r.color()?, radius: r.f32()? },
            3 => Cmd::StrokedRect { rect: r.rect()?, color: r.color()?, radius: r.f32()?, width: r.f32()? },
            4 => Cmd::Shadow { rect: r.rect()?, color: r.color()?, radius: r.f32()?, spread: r.f32()? },
            5 => Cmd::Line {
                a: Point { x: r.f32()?, y: r.f32()? },
                b: Point { x: r.f32()?, y: r.f32()? },
                width: r.f32()?,
                color: r.color()?,
            },
            6 => {
                let rect = r.rect()?;
                let len = r.u8()?;
                if len as usize > MAX_STOPS {
                    return None;
                }
                let mut stops = Stops { at: [0; MAX_STOPS], color: [Color::TRANSPARENT; MAX_STOPS], len };
                for i in 0..MAX_STOPS {
                    stops.at[i] = r.u8()?;
                    stops.color[i] = r.color()?;
                }
                Cmd::Gradient { rect, stops, angle: r.f32()? }
            }
            7 => Cmd::Image { rect: r.rect()?, id: ImageId(r.u32()?), tint: r.color()? },
            8 => Cmd::Glass {
                rect: r.rect()?,
                radius: r.f32()?,
                material: Material {
                    blur: r.f32()?,
                    tint: r.color()?,
                    rim: r.f32()?,
                    refraction: r.f32()?,
                    specular: r.f32()?,
                    light: r.f32()?,
                    shadow: r.u8()?,
                    morph_x: r.i16()?,
                    morph_y: r.i16()?,
                    adapt: r.u8()?,
                    tint_over_light: r.color()?,
                    dispersion: r.u8()?,
                    saturation: r.u8()?,
                },
            },
            9 => Cmd::PushClip { rect: r.rect()? },
            10 => Cmd::PopClip,
            _ => return None,
        };
        list.cmds.push(c);
    }
    let tn = r.u32()? as usize;
    let tb = r.take(tn)?;
    list.text.push_str(core::str::from_utf8(tb).ok()?);
    if !r.0.is_empty() {
        return None;
    }
    // A text range must name text that is there, on a character boundary:
    // `DisplayList::str_of` slices with it.
    for c in &list.cmds {
        if let Cmd::Text { text: (at, len), .. } = *c {
            let (a, b) = (at as usize, at as usize + len as usize);
            if b > list.text.len() || !list.text.is_char_boundary(a) || !list.text.is_char_boundary(b) {
                return None;
            }
        }
    }
    Some(())
}

/// [`decode_into`] a fresh list.
pub fn decode(bytes: &[u8]) -> Option<DisplayList> {
    let mut list = DisplayList::new();
    decode_into(bytes, &mut list)?;
    Some(list)
}

struct W<'a>(&'a mut Vec<u8>);

impl W<'_> {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i16(&mut self, v: i16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn rect(&mut self, r: Rect) {
        for v in r {
            self.f32(v);
        }
    }
    fn color(&mut self, c: Color) {
        self.0.extend_from_slice(&[c.0, c.1, c.2, c.3]);
    }
}

struct R<'a>(&'a [u8]);

impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.0.len() < n {
            return None;
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Some(a)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn i16(&mut self) -> Option<i16> {
        Some(i16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn rect(&mut self) -> Option<Rect> {
        Some([self.f32()?, self.f32()?, self.f32()?, self.f32()?])
    }
    fn color(&mut self) -> Option<Color> {
        let b = self.take(4)?;
        Some(Color(b[0], b[1], b[2], b[3]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_command() -> DisplayList {
        let mut l = DisplayList::new();
        l.bounds = Some([0.0, 0.0, 640.0, 480.0]);
        l.rect([1.0, 2.0, 3.0, 4.0], Color(1, 2, 3, 4));
        l.text_spaced([5.0, 6.0, 70.0, 20.0], "héllo", 15.0, 700.0, 24, Color(9, 8, 7, 6));
        l.cmds.push(Cmd::RoundedRect { rect: [0.0; 4], color: Color::BLACK, radius: 3.5 });
        l.cmds.push(Cmd::StrokedRect { rect: [1.0; 4], color: Color::BLACK, radius: 2.0, width: 1.5 });
        l.cmds.push(Cmd::Shadow { rect: [2.0; 4], color: Color::BLACK, radius: 4.0, spread: 8.0 });
        l.cmds.push(Cmd::Line { a: Point { x: 1.0, y: 2.0 }, b: Point { x: 3.0, y: 4.0 }, width: 1.0, color: Color::BLACK });
        let mut stops = Stops { at: [0, 128, 255, 0], color: [Color(1, 1, 1, 1); MAX_STOPS], len: 3 };
        stops.color[2] = Color(200, 100, 50, 255);
        l.cmds.push(Cmd::Gradient { rect: [3.0; 4], stops, angle: 90.0 });
        l.cmds.push(Cmd::Image { rect: [4.0; 4], id: ImageId(77), tint: Color::BLACK });
        let mut m = Material::FROSTED;
        m.morph_x = -300;
        m.dispersion = 9;
        l.cmds.push(Cmd::Glass { rect: [5.0; 4], radius: 12.0, material: m });
        l.cmds.push(Cmd::PushClip { rect: [6.0; 4] });
        l.cmds.push(Cmd::PopClip);
        l
    }

    #[test]
    fn every_command_round_trips() {
        let l = every_command();
        // Every tag, so a variant added to `Cmd` without a tag here is a
        // compile error in `encode` and a count mismatch here.
        assert_eq!(l.cmds.len(), 11);
        let mut b = Vec::new();
        encode(&l, &mut b);
        let d = decode(&b).expect("decodes");
        assert_eq!(d.cmds, l.cmds);
        assert_eq!(d.text, l.text);
        assert_eq!(d.bounds, l.bounds);
        assert_eq!(d.hash_from(0), l.hash_from(0));
    }

    /// M80/T80.1: the line pitch survives the trip, and a buffer of the
    /// version before it is refused, not read a byte out of step.
    #[test]
    fn a_line_height_round_trips_and_the_old_version_is_refused() {
        let mut l = DisplayList::new();
        l.text_spaced([0.0, 0.0, 80.0, 40.0], "two\nlines", 16.0, 400.0, 24, Color::BLACK);
        assert_eq!(l.cmds.len(), 1, "the fixture must hold the command it round-trips");
        let mut b = Vec::new();
        encode(&l, &mut b);
        let d = decode(&b).expect("decodes");
        let got = match d.cmds[0] {
            Cmd::Text { line_height, .. } => line_height,
            c => panic!("decoded {c:?}, not a text run"),
        };
        println!("line_height: encoded 24, decoded {got} (version {VERSION})");
        assert_eq!(got, 24, "the line pitch did not survive the trip");
        assert_eq!(d.cmds, l.cmds);
        // Version 1 is the encoding without the byte: a reader of it must
        // not be handed this one, nor this reader one of those.
        assert_ne!(VERSION, 1, "Cmd::Text gained a field and VERSION was not bumped");
        let mut old = b.clone();
        old[0] = 1;
        println!("a version-{} buffer decodes: {}", old[0], decode(&old).is_some());
        assert!(decode(&old).is_none(), "a buffer of version {} decoded under {VERSION}", old[0]);
    }

    /// M82/T82.1: a three-stop gradient round-trips with its byte
    /// offsets, and a buffer from the version-2 encoder is refused.
    ///
    /// The refusal fixture is a **literal**, written by the encoder before
    /// T82.1 (version 2): a rect and no gradient, so its layout is the
    /// same under version 3 and only the version byte can refuse it. A
    /// relative `VERSION + 1`, or an old gradient buffer (12 B longer, so
    /// the trailing-bytes check refuses it whatever the version says),
    /// could not see an unbumped `VERSION`.
    #[test]
    fn byte_offsets_round_trip_and_a_version_2_buffer_is_refused() {
        let mut l = DisplayList::new();
        let stops = Stops {
            at: [0, 51, 255, 0],
            color: [Color(1, 2, 3, 255), Color(4, 5, 6, 255), Color(7, 8, 9, 255), Color::TRANSPARENT],
            len: 3,
        };
        l.gradient([0.0, 0.0, 40.0, 20.0], stops, 0.0);
        assert_eq!(l.cmds.len(), 1, "the fixture must hold the gradient it round-trips");
        let mut b = Vec::new();
        encode(&l, &mut b);
        let d = decode(&b).expect("decodes");
        println!("3-stop gradient: {} B encoded at version {VERSION}, round trip equal: {}", b.len(), d.cmds == l.cmds);
        assert_eq!(d.cmds, l.cmds, "the gradient did not survive the trip");

        // `encode` at b873ab0 (version 2) of one rect [1, 2, 30, 40] in
        // Color(10, 20, 30, 255), no bounds.
        const V2_RECT: [u8; 31] = [
            2, 0, 1, 0, 0, 0, 0, 0, 0, 128, 63, 0, 0, 0, 64, 0, 0, 240, 65, 0, 0, 32, 66, 10, 20, 30, 255, 0, 0, 0, 0,
        ];
        let mut same = V2_RECT;
        same[0] = VERSION;
        assert!(decode(&same).is_some(), "setup: the fixture's body must decode under this layout");
        let old = decode(&V2_RECT);
        println!("a version-{} buffer decodes: {}", V2_RECT[0], old.is_some());
        println!("wire::VERSION = {VERSION} (want 3)");
        assert_eq!(VERSION, 3, "Stops' offsets changed meaning (M82/T82.1); VERSION must be 3");
        assert!(old.is_none(), "a version-2 buffer decoded under version {VERSION}");
    }

    #[test]
    fn a_damaged_buffer_is_refused_not_misread() {
        let l = every_command();
        let mut b = Vec::new();
        encode(&l, &mut b);
        // Every truncation.
        for n in 0..b.len() {
            assert!(decode(&b[..n]).is_none(), "a buffer cut at {n} of {} decoded", b.len());
        }
        // Another version.
        let mut v = b.clone();
        v[0] = VERSION + 1;
        assert!(decode(&v).is_none());
        // Trailing bytes.
        let mut t = b.clone();
        t.push(0);
        assert!(decode(&t).is_none());
        // A text range past the text: the `Text` command's length field.
        let mut small = DisplayList::new();
        small.text_weighted([0.0, 0.0, 40.0, 12.0], "ab", 10.0, 400.0, Color::BLACK);
        assert_eq!(small.cmds.len(), 1, "the fixture must hold the command it corrupts");
        let mut s = Vec::new();
        encode(&small, &mut s);
        // version, bounds flag, count(4), tag, rect(16), at(4) -> len at 1+1+4+1+16+4.
        let at = 1 + 1 + 4 + 1 + 16 + 4;
        assert_eq!(u32::from_le_bytes(s[at..at + 4].try_into().unwrap()), 2);
        s[at] = 9;
        assert!(decode(&s).is_none(), "a text range past the text decoded");
    }
}
