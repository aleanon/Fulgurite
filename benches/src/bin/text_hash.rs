//! **M46/T46.0 item 3, R2-F4: the price of a text hash that is an
//! identity, written down before the change** (`docs/plan-hash-identity.md`
//! §5).
//!
//! `display::fold` is `h·K + b`, linear over ℤ/2⁶⁴, so two strings of one
//! length collide whenever a small integer combination of their byte
//! differences vanishes — and a fold from 0 with no length cannot see a
//! leading zero byte. This prints, per arm, whether the solved pair and
//! the NUL-prefix pair collide, and what the arm costs at 9 B and 60 B.
//!
//! **The first two arms call the crate, not a copy** — §5's first risk is
//! a bench that measures a function the frame does not call, and M38's
//! probe (b546755) was a copy. So `crate fold_of` is whatever
//! `FoldedText::fold_of` is in *this* build, and `crate get_or hit` is the
//! memo hit the widgets pay; comparing builds (T46.1's kill rule K1) is
//! two binaries of this bench run alternately, not two arms of one.
//! The other arms are candidates, pasted here because they do not exist
//! in the crate until one is chosen:
//!
//! - `linear` — the pre-T46.1 fold, for the collision column;
//! - `len+fin` — the first plan's candidate: fold the length, finalise;
//! - `word mix` — §2.1 A: seed with the length, a rotate-xor-multiply per
//!   8-byte word, a finaliser;
//! - `sip13 keyed` — §2.1 C: SipHash-1-3 under a per-process key (std's
//!   `RandomState`).
//!
//! Eight runs per arm, interleaved, each run the median of 21 blocks.
//! Printed as min/median/max over the eight, with the load average before
//! and after. **Exit status 1 when the crate's own function lets either
//! pair collide** — red before T46.1, by construction.
//!
//! Run pinned, under the bench lock:
//! `flock …/bench.lock taskset -c 3 target/release/text-hash`.

use std::hash::{BuildHasher, Hasher};
use std::hint::black_box;
use std::time::Instant;

use lumen_static::style::Style;
use lumen_static::text::{FoldedText, MeasureKey};
use lumen_static::{Constraints, Size};

const K: u64 = 0x9E37_79B9_7F4A_7C15;

#[inline]
fn fold(h: u64, v: u64) -> u64 {
    h.wrapping_mul(K).wrapping_add(v)
}

/// SplitMix64's finaliser.
#[inline]
fn fin(mut z: u64) -> u64 {
    z ^= z >> 30;
    z = z.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z ^= z >> 27;
    z = z.wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[inline(never)]
fn linear(s: &str) -> u64 {
    s.bytes().fold(0u64, |h, b| fold(h, b as u64))
}

#[inline(never)]
fn len_fin(s: &str) -> u64 {
    fin(fold(linear(s), s.len() as u64))
}

/// The last 1–7 bytes as one word, without a copy: two overlapping
/// 4-byte loads from 4 up, three single bytes below — every byte is read
/// at least once, and the length (already in the seed) says how they
/// overlap, so the map is injective for a given length.
#[inline]
fn tail(r: &[u8]) -> u64 {
    let n = r.len();
    if n >= 4 {
        let lo = u32::from_le_bytes([r[0], r[1], r[2], r[3]]) as u64;
        let hi = u32::from_le_bytes([r[n - 4], r[n - 3], r[n - 2], r[n - 1]]) as u64;
        lo | (hi << 32)
    } else {
        ((r[0] as u64) << 16) | ((r[n / 2] as u64) << 8) | r[n - 1] as u64
    }
}

#[inline(never)]
fn word_mix(s: &str) -> u64 {
    let s = s.as_bytes();
    let mut h = (s.len() as u64).wrapping_mul(K);
    let mut c = s.chunks_exact(8);
    for w in &mut c {
        h = (h.rotate_left(5) ^ u64::from_le_bytes(w.try_into().unwrap())).wrapping_mul(K);
    }
    let r = c.remainder();
    if !r.is_empty() {
        h = (h.rotate_left(5) ^ tail(r)).wrapping_mul(K);
    }
    fin(h)
}

/// The low and high halves of the 128-bit product, xored: wyhash's
/// step. Not a bijection, so a chosen difference in one word cannot be
/// cancelled by a computed word after it.
#[inline]
fn fmul(a: u64, b: u64) -> u64 {
    let p = (a as u128).wrapping_mul(b as u128);
    (p as u64) ^ ((p >> 64) as u64)
}

#[inline(never)]
fn word_fmul(s: &str) -> u64 {
    const P: u64 = 0xe703_7ed1_a0b4_28db;
    let s = s.as_bytes();
    let mut h = (s.len() as u64).wrapping_mul(K);
    let mut c = s.chunks_exact(8);
    for w in &mut c {
        h = fmul(h ^ u64::from_le_bytes(w.try_into().unwrap()), P);
    }
    let r = c.remainder();
    if !r.is_empty() {
        h = fmul(h ^ tail(r), P);
    }
    fin(h)
}

/// A per-process key, as §2.1 C would have one: a host would hand it
/// down; here it is one load from a static.
static KEY: std::sync::OnceLock<std::collections::hash_map::RandomState> = std::sync::OnceLock::new();

#[inline(never)]
fn sip13(s: &str) -> u64 {
    let mut h = KEY.get_or_init(Default::default).build_hasher();
    h.write(s.as_bytes());
    h.finish()
}

#[inline(never)]
fn crate_fold_of(s: &str) -> u64 {
    FoldedText::fold_of(s)
}

/// A named hash under test.
type Arm = (&'static str, fn(&str) -> u64);

const SOLVED: (&str, &str) = ("akaihaalsioa", "eadaaenaaaai");
const NUL: (&str, &str) = ("x", "\0x");

/// A same-length pair that collides under `word_mix`, **computed, not
/// searched**: its per-word step `(rot(h) ^ w)·K` is a bijection of the
/// word, so after any first word the second can be chosen to cancel the
/// difference — `w2' = w2 ^ rot(h1) ^ rot(h1')`. The loop only looks for a
/// first word whose cancelling second word is printable ASCII (about one
/// in 2,800 are). This is why T46.1 does not ship the review's word mix.
fn constructed_for_word_mix() -> (String, String) {
    let a = *b"aaaaaaaabbbbbbbb";
    let h0 = 16u64.wrapping_mul(K);
    let step = |h: u64, w: u64| (h.rotate_left(5) ^ w).wrapping_mul(K);
    let w1 = u64::from_le_bytes(a[..8].try_into().unwrap());
    let w2 = u64::from_le_bytes(a[8..].try_into().unwrap());
    let h1 = step(h0, w1);
    for n in 0u64.. {
        let mut b1 = *b"aaaaaaaa";
        let mut k = n;
        for byte in b1.iter_mut() {
            *byte = b'a' + (k % 26) as u8;
            k /= 26;
        }
        let w1b = u64::from_le_bytes(b1);
        if w1b == w1 {
            continue;
        }
        let w2b = w2 ^ h1.rotate_left(5) ^ step(h0, w1b).rotate_left(5);
        let b2 = w2b.to_le_bytes();
        if b2.iter().all(|c| (0x20..0x7f).contains(c)) {
            let mut b = b1.to_vec();
            b.extend_from_slice(&b2);
            return (String::from_utf8(a.to_vec()).unwrap(), String::from_utf8(b).unwrap());
        }
    }
    unreachable!()
}

fn loadavg() -> String {
    std::fs::read_to_string("/proc/loadavg")
        .map(|s| s.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|_| "?".into())
}

/// One run: the median of 21 blocks of `n` calls, in ns per call.
fn run(f: &mut dyn FnMut() -> u64, n: u32) -> f64 {
    for _ in 0..n {
        black_box(f());
    }
    let mut blocks: Vec<f64> = (0..21)
        .map(|_| {
            let t = Instant::now();
            let mut acc = 0u64;
            for _ in 0..n {
                acc ^= f();
            }
            black_box(acc);
            t.elapsed().as_nanos() as f64 / n as f64
        })
        .collect();
    blocks.sort_by(|a, b| a.partial_cmp(b).unwrap());
    blocks[10]
}

fn main() {
    println!("text-hash: M46/T46.0 item 3 -- load before {}", loadavg());

    let arms: [Arm; 6] = [
        ("crate fold_of", crate_fold_of),
        ("linear", linear),
        ("len+fin", len_fin),
        ("word mix", word_mix),
        ("word fmul", word_fmul),
        ("sip13 keyed", sip13),
    ];

    let (ca, cb) = constructed_for_word_mix();
    println!("\n-- collisions (true = the pair hashes equal) --");
    println!("   solved: {:?} vs {:?} (linear, from 0)", SOLVED.0, SOLVED.1);
    println!("   constructed: {ca:?} vs {cb:?} (word mix, one xor)");
    let mut crate_collides = false;
    for (name, f) in arms {
        let s = f(SOLVED.0) == f(SOLVED.1);
        let z = f(NUL.0) == f(NUL.1);
        let c = f(&ca) == f(&cb);
        println!(
            "{name:>14}: solved pair collision survives={s}   \"x\" vs \"\\0x\" collision survives={z}   \
             constructed={c}"
        );
        if name == "crate fold_of" {
            crate_collides = s || z;
        }
    }

    let short = "count: 41";
    let long = "The quick brown fox jumps over the lazy dog and keeps runnin";
    assert_eq!((short.len(), long.len()), (9, 60));

    let key = MeasureKey::of(Constraints::tight(100.0, 20.0), &Style::default(), 1.0);

    println!("\n-- cost, ns per call: min / median / max of 8 runs (each the median of 21 blocks) --");
    println!("   K1 (plan §5): a FoldedText hit may cost at most +0.5 ns against the pre-T46.1 build");
    for (label, s) in [("9 B", short), ("60 B", long)] {
        let text = s;
        let mut memo = FoldedText::new();
        memo.get_or(key, text, || Size { w: 1.0, h: 1.0 });
        let mut runs: Vec<Vec<f64>> = vec![vec![]; arms.len() + 1];
        for _ in 0..8 {
            for (k, (_, f)) in arms.iter().enumerate() {
                runs[k].push(run(&mut || f(black_box(s)), 200_000));
            }
            runs[arms.len()].push(run(
                &mut || memo.get_or(black_box(key), black_box(text), || Size { w: 2.0, h: 2.0 }).w as u64,
                200_000,
            ));
        }
        let names = arms.iter().map(|a| a.0).chain(std::iter::once("crate get_or hit"));
        for (name, mut r) in names.zip(runs) {
            r.sort_by(|a, b| a.partial_cmp(b).unwrap());
            println!(
                "{label:>5} {name:>16}: min {:>6.2}   median {:>6.2}   max {:>6.2}",
                r[0],
                (r[3] + r[4]) / 2.0,
                r[7]
            );
        }
    }
    println!("\ntext-hash: load after {}", loadavg());
    if crate_collides {
        println!("text-hash: RED -- the crate's FoldedText::fold_of lets a pair collide");
        std::process::exit(1);
    }
}
