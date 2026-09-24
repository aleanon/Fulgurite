//! Load a library, read its ABI stamp, and accept or refuse it against
//! this binary's own — M37/T37.0 (`docs/plan-window-reload.md` §2.3).
//!
//! `stamp-host <lib> accept` or `stamp-host <lib> refuse <field>`. The
//! exit code is the verdict, so `scripts/abi_stamp.sh` is the gate, and
//! both directions run in it: a gate that refuses bad input must also
//! accept good input.

use lumen_static::reload::symbols;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (path, want) = (&args[0], args[1].as_str());
    let lib = unsafe { libloading::Library::new(path) }.expect("load");
    let f: libloading::Symbol<unsafe extern "C" fn(*mut u8, usize) -> usize> =
        unsafe { lib.get(symbols::ABI_STAMP) }.expect("the library exports no stamp");
    let theirs = unsafe { lumen_static_reload::abi::read(*f) };
    let mine = lumen_static_reload::abi::stamp(&[]);
    match (lumen_static_reload::abi::check(&mine, &theirs), want) {
        (Ok(()), "accept") => println!("accepted: {} fields agree", mine.lines().count()),
        (Err(m), "refuse") => {
            println!("refused: {m}");
            let field = args.get(2).expect("which field should be named");
            assert_eq!(&m.field, field, "refused, but for the wrong field");
        }
        (Ok(()), _) => panic!("accepted a library that should have been refused:\n{theirs}"),
        (Err(m), _) => panic!("refused a library built the same way: {m}"),
    }
}
