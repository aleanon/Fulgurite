//! **`Boundaries::mark` against a model** — M41/T41.0, for R18-F6
//! (`docs/plan-boundaries-repair.md` §1.4).
//!
//! `mark` absorbs every stored range the new one overlaps and stores the
//! union. The review's surviving mutant deleted `e = e.max(re)`, so an
//! absorbed range's end was lost whenever the new range ended earlier —
//! a nested mark *shrank* its ancestor — and the whole dev-shape suite
//! stayed green, the shadow oracle's test included. This line guards
//! every window since `Boundaries<256>` became the default (M34/T34.2).
//!
//! **A trap for any test here**: `Boundaries::default()` starts
//! all-dirty, so a test that forgets `clear()` asks every question of a
//! set that answers "dirty" to all of them, and passes under any mutant.
//! Both tests assert clean as their setup.

use fulgurite::invalidate::{Boundaries, Invalidation};

#[test]
fn a_nested_mark_does_not_shrink_its_ancestor() {
    let mut b: Boundaries<8> = Boundaries::default();
    b.clear();
    assert!(b.is_clean(0..64), "setup: a cleared set must be clean, or every assertion below is vacuous");
    b.mark(2..10);
    b.mark(3..4);
    b.mark(20..21);
    for (r, dirty) in [(5..6, true), (9..10, true), (2..3, true), (20..21, true), (12..13, false), (10..20, false)] {
        println!("{r:?}: dirty {} (want {dirty})", !b.is_clean(r.clone()));
        assert_eq!(!b.is_clean(r.clone()), dirty, "{r:?} after marking 2..10, 3..4, 20..21");
    }
}

/// A seeded generator, so a failure names a sequence that reproduces.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> usize {
        (self.next() % n) as usize
    }
}

/// 1 000 random mark sequences against a bitset. A set that has not
/// collapsed to "everything" must answer every range exactly as the
/// bitset does; one that has must answer "dirty" to all of them.
#[test]
fn boundaries_matches_a_bitset_model() {
    const LEN: usize = 40;
    let mut rng = Lcg(0x4d41_4e59_4d41_524b);
    let (mut disagreements, mut collapsed, mut queries) = (0usize, 0usize, 0usize);
    let mut first: Option<String> = None;
    for seq in 0..1000 {
        // A capacity of six, so some sequences overflow and the collapse
        // to "everything" is exercised as well as the merge.
        let mut b: Boundaries<6> = Boundaries::default();
        b.clear();
        assert!(b.is_clean(0..LEN), "setup: a cleared set must be clean");
        let mut model = [false; LEN];
        let mut marks = vec![];
        for _ in 0..rng.below(12) + 1 {
            let s = rng.below(LEN as u64);
            // Short and long ranges alike: long ones nest and merge,
            // short ones stay apart and fill the six slots.
            let span = if rng.below(2) == 0 { 4 } else { LEN - s };
            let e = s + rng.below(span.min(LEN - s) as u64 + 1);
            b.mark(s..e);
            marks.push(s..e);
            model[s..e].iter_mut().for_each(|d| *d = true);
        }
        if b.is_all() {
            collapsed += 1;
        }
        for s in 0..LEN {
            for e in s + 1..=LEN {
                queries += 1;
                let want_clean = !b.is_all() && !model[s..e].iter().any(|d| *d);
                if b.is_clean(s..e) != want_clean {
                    disagreements += 1;
                    first.get_or_insert_with(|| {
                        format!("sequence {seq}: marks {marks:?}, is_clean({s}..{e}) = {}, model says {want_clean}", !want_clean)
                    });
                }
            }
        }
    }
    println!(
        "sequences 1000, queries {queries}, collapsed to all {collapsed}, disagreements {disagreements} (bound: 0)"
    );
    if let Some(f) = &first {
        println!("first: {f}");
    }
    assert!(collapsed < 1000, "setup: every sequence collapsed, so no merge was checked");
    assert_eq!(disagreements, 0, "Boundaries disagreed with the bitset: {first:?}");
}
