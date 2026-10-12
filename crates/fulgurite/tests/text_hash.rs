//! M46/T46.1: `display::hash_bytes`, the one text identity, separates
//! every pair the linear fold it replaced could not.
//!
//! Each pair here is **proven to collide under the old fold first** —
//! the pre-T46.1 arithmetic is reproduced below for exactly that — so a
//! pair that stopped being a solved pair (a typo in the fixture) cannot
//! pass as "separated". A solved pair passes for any function other than
//! the one it was solved against, which is why it is not alone: the
//! length-prefix rows must be separated by any length-seeded function,
//! for a structural reason, and the constructed row is a pair built for
//! the review's rotate-xor-multiply mix (`docs/plan-hash-identity.md` §6).

use fulgurite::display::hash_bytes;
use fulgurite::text::FoldedText;

/// The pre-T46.1 fold, from a given start. `FoldedText` started at 0;
/// retention started at the command's tag and rect.
fn linear(start: u64, s: &[u8]) -> u64 {
    s.iter().fold(start, |h, &b| h.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(b as u64))
}

/// The review's word mix: `(rot(h,5) ^ w)·K` per 8-byte word.
fn word_mix(s: &[u8]) -> u64 {
    const K: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut h = (s.len() as u64).wrapping_mul(K);
    for w in s.chunks(8) {
        let mut b = [0u8; 8];
        b[..w.len()].copy_from_slice(w);
        h = (h.rotate_left(5) ^ u64::from_le_bytes(b)).wrapping_mul(K);
    }
    h
}

#[test]
fn every_solved_pair_is_separated() {
    // (a, b, the old function it collides under)
    let solved: [(&[u8], &[u8], &str, bool); 4] = [
        (b"akaihaalsioa", b"eadaaenaaaai", "linear from 0", linear(0, b"akaihaalsioa") == linear(0, b"eadaaenaaaai")),
        (b"x", b"\0x", "linear from 0", linear(0, b"x") == linear(0, b"\0x")),
        (b"x", b"\0\0\0x", "linear from 0", linear(0, b"x") == linear(0, b"\0\0\0x")),
        (
            b"aaaaaaaabbbbbbbb",
            b"cacaaaaa8gCJc!,f",
            "the review's word mix",
            word_mix(b"aaaaaaaabbbbbbbb") == word_mix(b"cacaaaaa8gCJc!,f"),
        ),
    ];
    for (a, b, under, collided) in solved {
        let (ha, hb) = (hash_bytes(a), hash_bytes(b));
        println!("{a:?} vs {b:?}: collided under {under}={collided}; hash_bytes {ha:#018x} vs {hb:#018x}");
        assert!(collided, "fixture: {a:?}/{b:?} must be a real collision under {under}");
        assert_ne!(ha, hb, "hash_bytes lets {a:?} and {b:?} collide");
    }
}

/// Retention's pair (`fulgurite-shell/tests/text_hash_collision.rs`)
/// collides from *any* start, because the old fold is linear and the
/// prefix cancels — which is why folding the rect first did not protect
/// retention. Three starts stand in for the one a `Recording` uses.
#[test]
fn the_retention_pair_is_separated() {
    let (a, b) = (b" k ih  lsio ", b"$a#aa$-aaaa(");
    for start in [0u64, 0x1234_5678_9abc_def0, u64::MAX] {
        assert_eq!(linear(start, a), linear(start, b), "fixture: must collide from {start:#x}");
    }
    assert_ne!(hash_bytes(a), hash_bytes(b));
}

/// Structural: every string of up to two bytes has its own hash. The
/// length is in the seed, and below a word the tail is read one-to-one,
/// so a collision here would be a defect in the construction, not luck.
#[test]
fn every_short_string_is_distinct() {
    let mut all: Vec<u64> = Vec::with_capacity(1 + 256 + 65_536);
    all.push(hash_bytes(b""));
    for x in 0..=255u8 {
        all.push(hash_bytes(&[x]));
    }
    for x in 0..=255u8 {
        for y in 0..=255u8 {
            all.push(hash_bytes(&[x, y]));
        }
    }
    let n = all.len();
    all.sort_unstable();
    all.dedup();
    println!("strings of 0..=2 bytes: {n}, distinct hashes: {} / bound {n}", all.len());
    assert_eq!(all.len(), n);
}

/// `FoldedText` keys on the same function, not on a second copy of it.
#[test]
fn the_memo_keys_on_the_same_function() {
    for s in ["", "x", "\0x", "count: 41", "The quick brown fox jumps over the lazy dog"] {
        assert_eq!(FoldedText::fold_of(s), hash_bytes(s.as_bytes()));
    }
}
