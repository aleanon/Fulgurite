//! Where an offset is on screen, and what a selection covers, over a
//! cluster list that may run both ways — M81/T81.5
//! (`docs/plan-shaped-paint.md`).
//!
//! `TextInput`, `TextArea` and `Selectable` place their caret, answer
//! `caret_rect` and tint their selection through these two functions, so
//! a caret, an input method's area and a highlight cannot disagree about
//! where an offset is.
//!
//! Both read [`Cluster`]'s contract: source order, `x` the **leading**
//! edge, and at a direction change one byte reported twice — the ending
//! run's trailing edge, then the next cluster's leading edge. Before
//! M81/T81.5 every reader found an offset with `find(byte >= at)` and
//! tinted from one offset's x to the other's, which is right only while x
//! rises with the byte.

use lumen_static::cx::Cluster;

/// The cluster that places offset `at`: **the last one with exactly that
/// byte** — so at a direction change the leading edge of what follows,
/// and at the text's end the trailing insertion point, which sits at the
/// paragraph direction's end (the right end of a left-to-right paragraph,
/// the left end of a right-to-left one).
///
/// An offset no cluster names (inside a ligature or a combining
/// sequence, which a grapheme step never stops at) falls back to the
/// first cluster after it, as every reader did before.
pub(crate) fn at(cl: &[Cluster], at: usize) -> Option<&Cluster> {
    let mut hit = None;
    for c in cl {
        let b = c.byte as usize;
        if b == at {
            hit = Some(c);
        } else if b > at {
            break;
        }
    }
    hit.or_else(|| cl.iter().find(|c| c.byte as usize >= at))
}

/// Two spans that meet or overlap on one line are one.
const TOUCH: f32 = 0.5;

/// The selection `sel` as **one rect per visually contiguous span** of
/// the clusters it covers, `f(x0, x1, line)` with `x0 < x1`.
// ANCHOR: tint_rule
/// Each selected cluster covers its leading edge to its trailing edge:
/// the x of the cluster after it in the list — which at a direction
/// change is the ending run's trailing insertion point, so the edge
/// exists — on the same line, and the line's entry in `ends` when the
/// line ends there (its leading edge when `ends` has none). Whether that
/// edge lies left or right of the leading one is the cluster's `rtl`, so
/// a span is the min and max of the two. Consecutive clusters whose spans
/// meet join one rect; a span that does not starts another, so selecting
/// across the boundary of `abc שלום` paints the Latin part and the Hebrew
/// part as two rects, never one rect over the unselected letters between
/// them.
// ANCHOR_END: tint_rule
///
/// Not generic, on purpose: `TextInput` is in every firmware binary and
/// paints its selection and its composing underline through this, so one
/// copy serves all three widgets (`docs/results-embedded.md` M81/T81.5).
pub(crate) fn spans(cl: &[Cluster], sel: core::ops::Range<usize>, ends: &[f32], f: &mut dyn FnMut(f32, f32, u16)) {
    // `cur` grows in source order; a finished one waits in `held` one
    // step, so a run that ends against the span before it (a whole word
    // selected after its neighbour) still joins it.
    let mut held: Option<Span> = None;
    let mut cur: Option<Span> = None;
    for (k, c) in cl.iter().enumerate() {
        let b = c.byte as usize;
        let next = cl.get(k + 1);
        // A direction change's first point is an edge, not a cluster.
        if b < sel.start || b >= sel.end || next.is_some_and(|n| n.byte == c.byte) {
            continue;
        }
        let trail = match next {
            Some(n) if n.line == c.line => n.x,
            _ => ends.get(c.line as usize).copied().unwrap_or(c.x),
        };
        let s = Span(c.x.min(trail), c.x.max(trail), c.line);
        cur = Some(match cur {
            Some(u) if u.meets(&s) => u.join(&s),
            Some(u) => {
                held = Some(match held {
                    Some(h) if h.meets(&u) => h.join(&u),
                    Some(h) => {
                        h.emit(f);
                        u
                    }
                    None => u,
                });
                s
            }
            None => s,
        });
    }
    match (held, cur) {
        (Some(h), Some(u)) if h.meets(&u) => h.join(&u).emit(f),
        (h, u) => {
            if let Some(h) = h {
                h.emit(f);
            }
            if let Some(u) = u {
                u.emit(f);
            }
        }
    }
}

/// One span of tint: x from, x to, line.
#[derive(Clone, Copy)]
struct Span(f32, f32, u16);

impl Span {
    /// On one line, meeting or overlapping.
    fn meets(&self, o: &Span) -> bool {
        self.2 == o.2 && o.0 <= self.1 + TOUCH && self.0 <= o.1 + TOUCH
    }

    fn join(&self, o: &Span) -> Span {
        Span(self.0.min(o.0), self.1.max(o.1), self.2)
    }

    fn emit(&self, f: &mut dyn FnMut(f32, f32, u16)) {
        if self.1 > self.0 {
            f(self.0, self.1, self.2)
        }
    }
}
