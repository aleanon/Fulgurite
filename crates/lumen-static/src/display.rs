//! The paint target — plan T5.1.
//!
//! Paint stops folding a checksum and starts emitting drawing commands.
//! This is the seam a renderer consumes: widgets describe *what* to
//! draw, and the renderer decides *how*, so a renderer can be injected
//! at the shell as `&mut dyn Renderer` without appearing anywhere in a
//! widget signature (the answer recorded in
//! `docs/plan-agent-parity.md`'s appendix).
//!
//! Text is stored as a range into one shared buffer rather than a
//! `String` per command, so a frame's worth of text costs one growable
//! allocation that is reused between frames.

/// Straight sRGB, 8 bits per channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

impl Color {
    pub const BLACK: Color = Color(0, 0, 0, 255);
    pub const TRANSPARENT: Color = Color(0, 0, 0, 0);
}

/// `[x0, y0, x1, y1]`, absolute.
pub type Rect = [f32; 4];

/// Whether a rect encloses anything a renderer could paint.
#[inline]
fn has_area(r: Rect) -> bool {
    r[2] > r[0] && r[3] > r[1]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cmd {
    Rect { rect: Rect, color: Color },
    /// `text` indexes [`DisplayList::text`].
    Text { rect: Rect, text: (u32, u32), size: f32, color: Color },
    PushClip { rect: Rect },
    PopClip,
}

impl Cmd {
    /// Content hash. Floats are hashed by bit pattern, which is what
    /// makes a command list comparable frame to frame — the change
    /// journal's "did this node's content change" signal.
    pub fn hash_into(&self, h: &mut u64) {
        #[inline]
        fn f(h: &mut u64, v: u64) {
            *h = h.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(v);
        }
        match self {
            Cmd::Rect { rect, color } => {
                f(h, 1);
                for v in rect {
                    f(h, v.to_bits() as u64);
                }
                f(h, u32::from_be_bytes([color.0, color.1, color.2, color.3]) as u64);
            }
            Cmd::Text { rect, text, size, color } => {
                f(h, 2);
                for v in rect {
                    f(h, v.to_bits() as u64);
                }
                f(h, ((text.0 as u64) << 32) | text.1 as u64);
                f(h, size.to_bits() as u64);
                f(h, u32::from_be_bytes([color.0, color.1, color.2, color.3]) as u64);
            }
            Cmd::PushClip { rect } => {
                f(h, 3);
                for v in rect {
                    f(h, v.to_bits() as u64);
                }
            }
            Cmd::PopClip => f(h, 4),
        }
    }
}

/// One frame's drawing commands, in paint order.
#[derive(Default, Debug)]
pub struct DisplayList {
    pub cmds: Vec<Cmd>,
    /// Text bytes for this frame; `Cmd::Text` holds ranges into it.
    pub text: String,
}

impl DisplayList {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reuse the buffers between frames — the allocations are the point
    /// of holding a list rather than rebuilding one.
    pub fn clear(&mut self) {
        self.cmds.clear();
        self.text.clear();
    }

    /// Record a filled rect. A rect with no area is dropped: it cannot
    /// draw, and recording it would put phantom content in front of the
    /// journal's per-node signature and the audit's "is anything
    /// visible" check. This is also what makes `Style::visible = false`
    /// reach the screen as absence rather than as a zero-sized command
    /// (`docs/plan-review-fixes.md` F1.6).
    pub fn rect(&mut self, rect: Rect, color: Color) {
        if has_area(rect) {
            self.cmds.push(Cmd::Rect { rect, color });
        }
    }

    /// Record a text run. Dropped when the box has no area, for the same
    /// reason as [`DisplayList::rect`].
    pub fn text(&mut self, rect: Rect, s: &str, size: f32, color: Color) {
        if !has_area(rect) {
            return;
        }
        let start = self.text.len() as u32;
        self.text.push_str(s);
        let range = (start, self.text.len() as u32 - start);
        self.cmds.push(Cmd::Text { rect, text: range, size, color });
    }

    pub fn push_clip(&mut self, rect: Rect) {
        self.cmds.push(Cmd::PushClip { rect });
    }

    pub fn pop_clip(&mut self) {
        self.cmds.push(Cmd::PopClip);
    }

    /// The string a `Cmd::Text` refers to.
    pub fn str_of(&self, range: (u32, u32)) -> &str {
        &self.text[range.0 as usize..(range.0 + range.1) as usize]
    }

    /// Hash of `cmds[from..]` — how the journal takes a node's content
    /// signature without building a list per node.
    pub fn hash_from(&self, from: usize) -> u64 {
        let mut h = 0u64;
        for c in &self.cmds[from.min(self.cmds.len())..] {
            c.hash_into(&mut h);
            if let Cmd::Text { text, .. } = c {
                for b in self.str_of(*text).as_bytes() {
                    h = h.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(*b as u64);
                }
            }
        }
        h
    }

    pub fn len(&self) -> usize {
        self.cmds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cmds.is_empty()
    }
}

/// What a renderer must do with a list. Implemented at the shell — the
/// renderer never appears in a widget signature.
pub trait Renderer {
    fn render(&mut self, list: &DisplayList, width: u32, height: u32);
}
