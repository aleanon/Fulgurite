//! Load a library, read its ABI stamp, and accept or refuse it against
//! this binary's own — M37/T37.0 (`docs/plan-window-reload.md` §2.3).
//!
//! `stamp-host <lib> accept` or `stamp-host <lib> refuse <field> [word]`,
//! where `word` is a feature the library's value of `field` lists and the
//! loader's does not (M83/T83.3). The
//! exit code is the verdict, so `scripts/abi_stamp.sh` is the gate, and
//! both directions run in it: a gate that refuses bad input must also
//! accept good input.

use fulgurite::reload::symbols;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (path, want) = (&args[0], args[1].as_str());
    let lib = unsafe { libloading::Library::new(path) }.expect("load");
    let f: libloading::Symbol<unsafe extern "C" fn(*mut u8, usize) -> usize> =
        unsafe { lib.get(symbols::ABI_STAMP) }.expect("the library exports no stamp");
    let theirs = unsafe { fulgurite_reload::abi::read(*f) };
    #[cfg(not(feature = "shell"))]
    let mine = fulgurite_reload::abi::stamp(&[]);
    // M81/T81.1: with the shell's fields, as a window's loader compares.
    #[cfg(feature = "shell")]
    let mine = fulgurite_reload::abi::stamp(&fulgurite_shell::session::abi_fields());
    match (fulgurite_reload::abi::check(&mine, &theirs), want) {
        (Ok(()), "accept") => println!("accepted: {} fields agree", mine.lines().count()),
        (Err(m), "refuse") => {
            println!("refused: {m}");
            let field = args.get(2).expect("which field should be named");
            match args.get(3) {
                None => assert_eq!(&m.field, field, "refused, but for the wrong field"),
                // M83/T83.3: a feature word -- `field` must differ, with
                // `word` in the library's value and not the loader's,
                // whichever field `check` happened to report first. A core
                // shader build is refused first by `DisplayList`'s size (its
                // uniform buffer); the word asks whether the feature entry
                // would have told the builds apart on its own, which is what
                // its injection removes.
                Some(word) => {
                    let parse = |s: &str| -> Vec<(String, String)> {
                        s.lines().filter_map(|l| l.split_once('=')).map(|(k, v)| (k.into(), v.into())).collect()
                    };
                    let (a, b) = (parse(&mine), parse(&theirs));
                    let get = |v: &[(String, String)], k: &str| v.iter().find(|(x, _)| x == k).map(|(_, y)| y.clone());
                    let differ: Vec<&str> =
                        a.iter().filter(|(k, v)| get(&b, k).as_deref() != Some(v.as_str())).map(|(k, _)| k.as_str()).collect();
                    let has = |v: Option<String>| v.as_deref().unwrap_or("").split(',').any(|w| w == word);
                    let (l, r) = (has(get(&a, field)), has(get(&b, field)));
                    println!("fields that differ: {differ:?}; `{word}` in `{field}`: loader {l}, library {r}");
                    assert!(differ.contains(&field.as_str()), "`{field}` does not differ: the entry cannot tell these builds apart");
                    assert!(r && !l, "`{field}` differs, but not by `{word}`");
                }
            }
        }
        (Ok(()), _) => panic!("accepted a library that should have been refused:\n{theirs}"),
        (Err(m), _) => panic!("refused a library built the same way: {m}"),
    }
}
