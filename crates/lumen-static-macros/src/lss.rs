//! `include_lss!` — a `.lss` stylesheet as `const` rules, parsed while the
//! invoking crate compiles (M61/T61.3, `docs/plan-lss.md` §2.2).
//!
//! ```text
//! include_lss!(Tree, "app.lss")         // the top-level rules
//! include_lss!(Tree, "app.lss", pos)    // …then `@sheet pos`'s
//! ```
//!
//! The expansion is a block whose value is a `&'static [Rule]` built from
//! `Rule::new` / `Patch::new().setter(..)` — **the tokens a person writes
//! by hand**, as a `const`, so a sheet authored this way links to the same
//! bytes (kill rule K1, `scripts/check_lss_parity.sh`). Beside it:
//!
//! - `const _: &[u8] = include_bytes!("<absolute path>");` — a proc macro
//!   that reads a file is invisible to cargo, and this is what makes rustc
//!   record the file as an input, so an edited `.lss` rebuilds the crate
//!   (`scripts/lss_tracking.sh`). A `const _`, never a `static`: the bytes
//!   are only an input, and must not be linked.
//! - one `const` assertion per `#id` and `.class` the file names, that the
//!   tree has it (T61.4, `docs/plan-lss.md` §2.3).
//!
//! **The path is relative to the invoking source file**, as
//! `include_bytes!`'s is — not to `CARGO_MANIFEST_DIR`, which §2.2 first
//! said; the plan's "What building found" records why.
//!
//! A mistake in the file is a `compile_error!` carrying
//! `file:line:col: message`, spanned to the path literal — one per error
//! the parser and the lowering found, not only the first.

use proc_macro2::{Literal, Span, TokenStream as Ts2};
use quote::{quote, quote_spanned};
use syn::parse::{Parse, ParseStream};
use syn::{LitStr, Token, Type};

use lumen_static_lss::lower::{Edges, Length, Rule, Val};
use lumen_static_lss::ast::Part;

/// `Tree, "path"` and an optional `, sheet`.
struct Args {
    tree: Type,
    path: LitStr,
    sheet: Option<syn::Ident>,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let tree: Type = input.parse()?;
        input.parse::<Token![,]>()?;
        let path: LitStr = input.parse()?;
        let mut sheet = None;
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if !input.is_empty() {
                sheet = Some(input.parse()?);
                if input.peek(Token![,]) {
                    input.parse::<Token![,]>()?;
                }
            }
        }
        Ok(Args { tree, path, sheet })
    }
}

/// The expansion, or the errors as `compile_error!`s. `invoker` is the
/// file the macro was called from, when the compiler knows it.
pub(crate) fn expand(input: Ts2, invoker: Option<std::path::PathBuf>) -> Ts2 {
    let args: Args = match syn::parse2(input) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error(),
    };
    let written = args.path.value();
    let span = args.path.span();
    let base = invoker
        .as_deref()
        .and_then(|f| f.parent())
        .map(|d| d.to_path_buf())
        .or_else(|| std::env::var_os("CARGO_MANIFEST_DIR").map(Into::into))
        .unwrap_or_default();
    let path = base.join(&written);
    let src = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            let msg = format!(
                "include_lss!: cannot read `{written}` ({}): {e} -- the path is relative to the file \
                 that invokes the macro",
                path.display()
            );
            return errors_in_place(vec![(span, msg)]);
        }
    };
    let sheet = match lumen_static_lss::compile(&src) {
        Ok(s) => s,
        Err(errors) => return errors_in_place(errors.iter().map(|e| (span, e.at(&written))).collect()),
    };
    let block = args.sheet.as_ref().map(|i| i.to_string());
    let rules = match sheet.select(block.as_deref()) {
        Ok(r) => r,
        Err(msg) => {
            let at = args.sheet.as_ref().map_or(span, |i| i.span());
            return errors_in_place(vec![(at, format!("include_lss!: {written}: {msg}"))]);
        }
    };
    // The tracking line wants an absolute path: `include_bytes!` resolves
    // a relative one against the file its expansion is attributed to,
    // which need not be the invoker's.
    let absolute = std::fs::canonicalize(&path).unwrap_or(path);
    let absolute = absolute.to_string_lossy().into_owned();
    let rules: Vec<Ts2> = rules.iter().map(|r| rule(r)).collect();
    let checks = tree_checks(&args.tree, &sheet, &written, span);
    quote! {{
        const _: &[u8] = ::core::include_bytes!(#absolute);
        #(#checks)*
        const __LUMEN_LSS: &[::lumen_static::style::Rule] = &[#(#rules),*];
        __LUMEN_LSS
    }}
}

/// **Ids and classes are checked against the tree type** (T61.4,
/// `docs/plan-lss.md` §2.3): one `const` assertion per distinct `#id` and
/// `.class` in the whole file — every block, not only the one selected —
/// that some node of `Tree` carries it. "Some node", not `wid_index!`'s
/// "exactly one": styling several nodes with one id is allowed; only
/// addressing them is not.
///
/// **The table is walked once**, into the list of the tree's ids
/// (`style::wids_of`) and of its classes (`style::classes_of`), and each
/// assertion reads the short list. Asserting `count_wid(..) > 0` per id
/// walked the whole table per id, and in `const` evaluation that was 88 ms
/// of a warm build for the showcase's 42 names (`docs/results-lss.md`, K2).
///
/// **Kinds are not checked**: `NodeDesc` carries no kind, which is
/// `Node::kind_name`, a run-time method. A kind selector is left to
/// `W1010` at run time (`lumen-static-agent`'s `tests/lss_kind.rs` pins
/// that), and checking it here would need a `const KIND` on `Widget` —
/// the plan's §4 says why that is its own change.
fn tree_checks(tree: &Type, sheet: &lumen_static_lss::Sheet, written: &str, span: Span) -> Vec<Ts2> {
    let tree_name = quote!(#tree).to_string().replace(' ', "");
    let table = quote_spanned!(span=> <#tree as ::lumen_static::widget::Widget>::SUB_TABLE);
    let mut seen: Vec<(bool, &str)> = Vec::new();
    let mut out = Vec::new();
    for r in sheet.all_rules() {
        let (is_id, name) = match &r.selector {
            Part::Id(s) => (true, s.as_str()),
            Part::Class(s) => (false, s.as_str()),
            Part::Kind(_) | Part::Any => continue,
        };
        if seen.contains(&(is_id, name)) {
            continue;
        }
        seen.push((is_id, name));
        let at = format!("{written}:{}:{}", r.span.line, r.span.col);
        // A format string under edition 2021's `assert!`: braces doubled.
        let msg = if is_id {
            format!(
                "{at}: `#{name}` names no node -- there is no `#[widget_id(\"{name}\")]` in `{tree_name}` \
                 (docs/plan-lss.md §2.3)"
            )
        } else {
            format!(
                "{at}: `.{name}` names no node -- there is no `#[widget_class(\"{name}\")]` in \
                 `{tree_name}` (docs/plan-lss.md §2.3)"
            )
        }
        .replace('{', "{{")
        .replace('}', "}}");
        let list = if is_id { quote!(__LUMEN_LSS_IDS) } else { quote!(__LUMEN_LSS_CLASSES) };
        out.push(quote_spanned! {span=>
            const _: () = ::core::assert!(::lumen_static::style::names_contain(&#list, #name), #msg);
        });
    }
    if out.is_empty() {
        return out;
    }
    let mut lists = Vec::new();
    if seen.iter().any(|(id, _)| *id) {
        lists.push(quote_spanned! {span=>
            const __LUMEN_LSS_IDS: [&str; ::lumen_static::style::wid_count(#table)] =
                ::lumen_static::style::wids_of::<{ ::lumen_static::style::wid_count(#table) }>(#table);
        });
    }
    if seen.iter().any(|(id, _)| !*id) {
        lists.push(quote_spanned! {span=>
            const __LUMEN_LSS_CLASSES: [&str; ::lumen_static::style::class_count(#table)] =
                ::lumen_static::style::classes_of::<{ ::lumen_static::style::class_count(#table) }>(#table);
        });
    }
    lists.extend(out);
    lists
}

/// Every error as its own `compile_error!`, in a block that still has the
/// expansion's type — so the author reads the sheet's errors and not a
/// type mismatch they caused.
fn errors_in_place(errors: Vec<(Span, String)>) -> Ts2 {
    let each = errors.iter().map(|(span, msg)| quote_spanned!(*span=> ::core::compile_error!(#msg);));
    quote! {{
        #(#each)*
        const __LUMEN_LSS: &[::lumen_static::style::Rule] = &[];
        __LUMEN_LSS
    }}
}

/// `Rule::new(selector, Patch::new().a(..).b(..))`.
fn rule(r: &Rule) -> Ts2 {
    let sel = match &r.selector {
        Part::Id(s) => quote!(::lumen_static::style::Selector::Id(#s)),
        Part::Class(s) => quote!(::lumen_static::style::Selector::Class(#s)),
        Part::Kind(s) => quote!(::lumen_static::style::Selector::Kind(#s)),
        Part::Any => quote!(::lumen_static::style::Selector::Any),
    };
    let calls = r.setters.iter().map(|s| {
        let m = syn::Ident::new(s.method, Span::call_site());
        let v = val(&s.value);
        quote!(.#m(#v))
    });
    quote!(::lumen_static::style::Rule::new(#sel, ::lumen_static::style::Patch::new() #(#calls)*))
}

fn f32_lit(v: f32) -> Literal {
    Literal::f32_suffixed(v)
}

fn val(v: &Val) -> Ts2 {
    match *v {
        Val::F32(x) => {
            let l = f32_lit(x);
            quote!(#l)
        }
        Val::U8(x) => {
            let l = Literal::u8_suffixed(x);
            quote!(#l)
        }
        Val::Bool(b) => quote!(#b),
        Val::Color([r, g, b, a]) => {
            let [r, g, b, a] = [r, g, b, a].map(Literal::u8_suffixed);
            quote!(::lumen_static::display::Color(#r, #g, #b, #a))
        }
        Val::Role(role) => variant(quote!(Role), role.variant()),
        Val::Align(a) => variant(quote!(Align), a.variant()),
        Val::Cursor(c) => variant(quote!(Cursor), c.variant()),
        Val::Length(Length::Fixed(x)) => {
            let l = f32_lit(x);
            quote!(::lumen_static::style::Length::Fixed(#l))
        }
        Val::Length(Length::Shrink) => quote!(::lumen_static::style::Length::Shrink),
        Val::Length(Length::FillPortion(k)) => {
            let l = Literal::u16_suffixed(k);
            quote!(::lumen_static::style::Length::FillPortion(#l))
        }
        Val::Edges(Edges { left, right, top, bottom }) => {
            let [l, r, t, b] = [left, right, top, bottom].map(f32_lit);
            quote!(::lumen_static::style::Edges { left: #l, right: #r, top: #t, bottom: #b })
        }
    }
}

fn variant(ty: Ts2, name: &str) -> Ts2 {
    let v = syn::Ident::new(name, Span::call_site());
    quote!(::lumen_static::style::#ty::#v)
}

