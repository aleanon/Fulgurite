//! The three derives: `StaticTree` (a container's shape),
//! `Reloadable` (what survives a hot swap), and `App` (the application
//! root and its bindings).
//!
//! ```
//! use lumen_static_widgets::ops::{Fixed, Label, Spacer};
//! use lumen_static::StaticTree;
//!
//! #[derive(StaticTree, Default)]
//! #[widget(row, spacing = 4.0)]   // or `column`, or `stack`
//! struct Header {
//!     #[widget_id("title")]
//!     title: Label,
//!     gap: Spacer,
//!     close: Fixed,
//! }
//! ```
//!
//! Every field is a child widget — a leaf from `lumen_static::ops`, or
//! another derived struct. What `StaticTree` emits is all const:
//! `NODE_COUNT` and `DEPTH` summed over the field types, and a
//! `SUB_TABLE` of `NodeDesc` entries spliced at compile time from the
//! fields' own sub-tables, carrying parent/child/sibling links plus the
//! `#[widget_id]` and `#[widget_class]` selector inputs.
//!
//! The table is *addressing*, not dispatch. Traversal is recursive
//! descent — a container calls its children through `&mut dyn Node` —
//! so there is no kind list and no `fn`-pointer table
//! (`docs/results-traversal.md`). Two const assertions ride along with
//! every expansion: the spliced table is well-formed — DFS pre-order,
//! forward sibling links, and a last child's link 0, which is what
//! `subtree_range` and every chain walk assume (tightened by M41/T41.6;
//! see `table_is_well_formed`) — and `DEPTH` fits `MAX_DEPTH`, so a tree
//! that could overflow the stack does not compile.
#![forbid(unsafe_code)]

mod lss;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Ts2;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

/// A container's declared mode. **Not `ops::Axis`** — that enum has two
/// variants and every method on it (`main`, `cross`, `size`, and the two
/// `Length` accessors) presupposes a main axis. A stack has none, so it
/// is a third *codegen* branch here rather than a third value there
/// (M11/T11.6).
#[derive(Clone, Copy, PartialEq)]
enum Axis {
    Row,
    Column,
    /// Children share the container's rect. Painted in order, so the
    /// last field is on top; hit in reverse, so the topmost is found
    /// first.
    Stack,
    /// Children flow left to right and wrap — M27/T27.8.
    ///
    /// **`wrap` and `grid` are one lowering, not two.** A grid is a wrap
    /// whose line break is a *count* instead of a width, and the rest —
    /// measuring loosely, accumulating a line, carrying the tallest
    /// child down — is identical. Two lowerings would have been two
    /// places for the cross-axis arithmetic to disagree, which is the
    /// `Deferring` shape in a macro.
    ///
    /// `None` wraps on width; `Some(n)` breaks every `n` children.
    Flow(Option<usize>),
}

/// `include_lss!(Tree, "file.lss" [, sheet])` — a `.lss` stylesheet as
/// `const` rules, parsed while the crate compiles (M61,
/// `docs/plan-lss.md`). The expansion and its guarantees are described in
/// this crate's `lss` module; `lumen_static::include_lss`'s rustdoc is the
/// user's page.
#[proc_macro]
pub fn include_lss(input: TokenStream) -> TokenStream {
    lss::expand(input.into(), proc_macro::Span::call_site().local_file()).into()
}

#[proc_macro_derive(StaticTree, attributes(widget, widget_id, widget_class))]
pub fn derive_static_tree(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// `#[widget_class("row", "striped")]` — the documented spelling — or the
/// bare-ident form `#[widget_class(row, striped)]`. Both are accepted;
/// anything else is an error rather than an empty class list.
///
/// This used to be `let _ = attr.parse_nested_meta(...)`, which silently
/// produced `&[]` for the *documented* form, because `parse_nested_meta`
/// yields paths and a string literal is not one. `Selector::Class` was
/// therefore unreachable through the derive for anyone who read the docs
/// (`docs/plan-review-fixes.md` F1.5).
/// The same, for a **struct-level** `#[widget_class(...)]` — the
/// container's own node.
///
/// **It was accepted and silently ignored until M27/T27.7.**
/// `attributes(widget, widget_id, widget_class)` let it through, nothing
/// read it, and the class never reached the table — so a rule written
/// against it matched no node. That is exactly the F1.5 shape the
/// comment above describes, one level up: the documented-looking
/// spelling produced `&[]`. Found because `every_stylesheet_rule_changes_something`
/// reported the rule as `Unmatched`, which is the sheet lint doing the
/// job it was built for (M25/T25.2).
fn parse_struct_classes(attrs: &[syn::Attribute]) -> syn::Result<Ts2> {
    let mut out: Vec<String> = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("widget_class") {
            continue;
        }
        attr.parse_args_with(|input: syn::parse::ParseStream| {
            let names =
                syn::punctuated::Punctuated::<ClassName, syn::Token![,]>::parse_terminated(input)?;
            out.extend(names.into_iter().map(|c| c.0));
            Ok(())
        })?;
    }
    Ok(if out.is_empty() { quote! { &[] } } else { quote! { &[#(#out),*] } })
}

fn parse_classes(f: &syn::Field) -> syn::Result<Ts2> {
    let mut out: Vec<String> = Vec::new();
    for attr in &f.attrs {
        if !attr.path().is_ident("widget_class") {
            continue;
        }
        attr.parse_args_with(|input: syn::parse::ParseStream| {
            let names =
                syn::punctuated::Punctuated::<ClassName, syn::Token![,]>::parse_terminated(input)?;
            out.extend(names.into_iter().map(|c| c.0));
            Ok(())
        })?;
    }
    Ok(if out.is_empty() { quote! { &[] } } else { quote! { &[#(#out),*] } })
}

/// One class name: a string literal or a bare identifier.
struct ClassName(String);

impl syn::parse::Parse for ClassName {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if input.peek(syn::LitStr) {
            Ok(ClassName(input.parse::<syn::LitStr>()?.value()))
        } else if input.peek(syn::Ident) {
            Ok(ClassName(input.parse::<syn::Ident>()?.to_string()))
        } else {
            Err(input.error("expected a class name: `\"striped\"` or `striped`"))
        }
    }
}

/// `#[widget_id("save")]`. The bare-ident form is rejected rather than
/// silently yielding an empty id — `wid` is the agent's stable address
/// and a selector input, so a dropped one fails far from the typo.
///
/// **So are a second one and an empty one** — M55/T55.2 (R16-F4). This
/// returned the *first* `#[widget_id]` and dropped the rest, and
/// `#[widget_id("")]` meant the same as no id at all: both compiled, and
/// the author's selector then matched nothing, far from the attribute.
/// One on the struct is refused in [`expand`].
fn parse_wid(f: &syn::Field) -> syn::Result<String> {
    let mut found: Option<(&syn::Attribute, String)> = None;
    for attr in &f.attrs {
        if attr.path().is_ident("widget_id") {
            let lit: syn::LitStr = attr.parse_args().map_err(|_| {
                syn::Error::new_spanned(
                    attr,
                    "expected a string literal: `#[widget_id(\"save\")]`",
                )
            })?;
            if lit.value().is_empty() {
                return Err(syn::Error::new_spanned(
                    &lit,
                    "an empty `#[widget_id]` addresses nothing; name the node or remove the attribute",
                ));
            }
            if let Some((prev, _)) = found {
                let mut e = syn::Error::new_spanned(attr, "a second `#[widget_id]` on one field");
                e.combine(syn::Error::new_spanned(prev, "the first is here"));
                return Err(e);
            }
            found = Some((attr, lit.value()));
        }
    }
    Ok(found.map(|(_, id)| id).unwrap_or_default())
}

/// Refuse `#[widget_id]` on a container struct — M55/T55.2 (R16-F4).
///
/// A node's id is given by the field that holds it, in its *parent*: the
/// struct's own node is the root, or a field of some other container,
/// and that field is where its id goes. On the struct it was accepted and
/// nothing read it, so the root kept the empty id.
fn refuse_struct_wid(attrs: &[syn::Attribute]) -> syn::Result<()> {
    match attrs.iter().find(|a| a.path().is_ident("widget_id")) {
        Some(a) => Err(syn::Error::new_spanned(
            a,
            "`#[widget_id]` goes on the field that holds this container, not on the struct: \
             a node's id is given by its parent",
        )),
        None => Ok(()),
    }
}

/// Refuse a `StaticTree` attribute on an enum or its variants — M55/T55.2.
///
/// An enum is a variant switch: it has no axis, and its node's id and
/// classes come from the field that holds it. `attributes(widget,
/// widget_id, widget_class)` let all three through here, and nothing
/// read them.
fn refuse_enum_attrs(data: &syn::DataEnum, attrs: &[syn::Attribute]) -> syn::Result<()> {
    let is_ours = |a: &&syn::Attribute| {
        ["widget", "widget_id", "widget_class"].iter().any(|n| a.path().is_ident(n))
    };
    let variant_attrs = data.variants.iter().flat_map(|v| {
        v.attrs.iter().chain(v.fields.iter().flat_map(|f| f.attrs.iter()))
    });
    match attrs.iter().chain(variant_attrs).find(is_ours) {
        Some(a) => Err(syn::Error::new_spanned(
            a,
            "an enum widget takes no `#[widget]`, `#[widget_id]` or `#[widget_class]`: it has no \
             axis, and its id and classes go on the field that holds it",
        )),
        None => Ok(()),
    }
}

/// Reject a generic or lifetime-parameterized widget with one sentence.
///
/// `expand`, `expand_enum` and `expand_reloadable` ignore `generics`
/// entirely, so a generic struct produced a cascade of a dozen raw
/// resolution failures inside generated code. Three lines turn that into
/// one error at the parameter list (F6.2).
fn reject_generics(input: &DeriveInput, what: &str) -> syn::Result<()> {
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            format!("{what} does not support generic or lifetime parameters"),
        ));
    }
    Ok(())
}

fn expand(input: DeriveInput) -> syn::Result<Ts2> {
    reject_generics(&input, "StaticTree")?;
    let name = &input.ident;
    let name_str = name.to_string();

    // An enum is a variant switch, not a container: it has no axis and
    // takes no `#[widget(...)]`.
    if let Data::Enum(e) = &input.data {
        refuse_enum_attrs(e, &input.attrs)?;
        let line = input.ident.span().start().line as u32;
        return expand_enum(name, &name_str, e, line);
    }
    refuse_struct_wid(&input.attrs)?;

    // ---- #[widget(row|column|stack, spacing = <f32>, fuse)] ----
    //
    // **Every key once** — M55/T55.2 (R16-F4). They used to overwrite:
    // `#[widget(row, column)]` laid out as a column, and so did the same
    // two keywords in two separate `#[widget]` attributes, which read as
    // though both applied. An axis given twice names the first; any
    // other key given twice is refused at the repeat.
    let mut axis = None;
    let mut axis_kw: Option<syn::Path> = None;
    let mut seen: Vec<String> = Vec::new();
    let mut spacing = 0.0f32;
    let mut spacing_span = None;
    let mut columns: Option<usize> = None;
    let mut columns_span: Option<syn::LitInt> = None;
    let mut fuse = false;
    for attr in &input.attrs {
        if !attr.path().is_ident("widget") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            let mode = if meta.path.is_ident("row") {
                Some(Axis::Row)
            } else if meta.path.is_ident("column") {
                Some(Axis::Column)
            } else if meta.path.is_ident("stack") {
                Some(Axis::Stack)
            } else if meta.path.is_ident("wrap") {
                Some(Axis::Flow(None))
            } else if meta.path.is_ident("grid") {
                Some(Axis::Flow(Some(0)))
            } else {
                None
            };
            if let Some(mode) = mode {
                if let Some(first) = &axis_kw {
                    let name = |p: &syn::Path| p.get_ident().map(|i| i.to_string()).unwrap_or_default();
                    let mut e = meta.error(format!(
                        "a second container mode: `{}` after `{}` — a container has one",
                        name(&meta.path),
                        name(first)
                    ));
                    e.combine(syn::Error::new_spanned(first, "the first is here"));
                    return Err(e);
                }
                axis = Some(mode);
                axis_kw = Some(meta.path.clone());
                return Ok(());
            }
            let key = meta.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
            if seen.contains(&key) {
                return Err(meta.error(format!("`{key}` given twice")));
            }
            if meta.path.is_ident("columns") {
                let lit: syn::LitInt = meta.value()?.parse()?;
                columns = Some(lit.base10_parse()?);
                columns_span = Some(lit);
            } else if meta.path.is_ident("spacing") {
                let lit: syn::LitFloat = meta.value()?.parse()?;
                spacing = lit.base10_parse()?;
                spacing_span = Some(lit);
            } else if meta.path.is_ident("fuse") {
                fuse = true;
            } else {
                return Err(meta.error(
                    "expected `row`, `column`, `stack`, `wrap`, `grid`, \
                     `columns = <int>`, `spacing = <float>`, or `fuse`",
                ));
            }
            seen.push(key);
            Ok(())
        })?;
    }
    let Some(axis) = axis else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "StaticTree needs #[widget(row)], #[widget(column)], #[widget(stack)], \
             #[widget(wrap)] or #[widget(grid, columns = N)]",
        ));
    };
    // **`columns` belongs to `grid` and nothing else.** Accepting it on
    // a row and ignoring it is the shape the `stack`/`spacing` check
    // below exists to refuse, so it is an error at the token that asked.
    // (The same shape as a repeated key, which the loop above refuses —
    // M55/T55.2.)
    let axis = match (axis, columns) {
        (Axis::Flow(Some(_)), Some(0)) => {
            return Err(syn::Error::new_spanned(
                columns_span.unwrap(),
                "a grid needs at least one column",
            ))
        }
        (Axis::Flow(Some(_)), Some(c)) => Axis::Flow(Some(c)),
        (Axis::Flow(Some(_)), None) => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "#[widget(grid)] needs `columns = <int>`; without one it is a wrap",
            ))
        }
        (_, Some(_)) => {
            return Err(syn::Error::new_spanned(
                columns_span.unwrap(),
                "`columns` applies to `grid` only — a row, column, stack or wrap has none",
            ))
        }
        (other, None) => other,
    };
    // **A stack has no gaps to space.** Accepting the attribute and
    // ignoring it is the shape that makes someone spend an afternoon on
    // a layout that was never going to move, so it is an error at the
    // token that asked for it.
    if axis == Axis::Stack {
        if let Some(lit) = spacing_span {
            return Err(syn::Error::new_spanned(
                lit,
                "`spacing` has no meaning on #[widget(stack)]: its children share one rect \
                 rather than sitting next to each other",
            ));
        }
    }
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "StaticTree derives structs (containers) and enums (variant switches)",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(&input.ident, "StaticTree needs named fields"));
    };

    let field_ident: Vec<_> = fields.named.iter().map(|f| f.ident.clone().unwrap()).collect();
    let field_ty: Vec<_> = fields.named.iter().map(|f| f.ty.clone()).collect();
    let n = field_ident.len();
    let n_f32 = n as f32;
    let sp = spacing;
    let spaced_total = if n > 1 { spacing * (n - 1) as f32 } else { 0.0 };
    let first_child: u32 = if n > 0 { 1 } else { 0 };
    let child_count = n as u16;
    let has_children = n > 0;

    // §9 provenance + author ids.
    let root_line = input.ident.span().start().line as u32;
    let field_line: Vec<u32> =
        fields.named.iter().map(|f| f.ident.as_ref().unwrap().span().start().line as u32).collect();
    let root_classes = parse_struct_classes(&input.attrs)?;
    let field_classes: Vec<Ts2> = fields
        .named
        .iter()
        .map(parse_classes)
        .collect::<syn::Result<Vec<_>>>()?;
    let field_wid: Vec<String> =
        fields.named.iter().map(parse_wid).collect::<syn::Result<Vec<_>>>()?;

    // Only the row/column bodies mention it; the stack bodies have no
    // axis to name.
    let axis_expr = match axis {
        // A flow runs along the row axis; its wrapping is the cross one.
        Axis::Row | Axis::Stack | Axis::Flow(_) => quote! { ::lumen_static::ops::Axis::Row },
        Axis::Column => quote! { ::lumen_static::ops::Axis::Column },
    };
    // A container fills across its cross axis and shrinks along its main
    // one — the shape that makes a column of rows behave as expected.
    let (default_w, default_h) = (
        quote! { ::lumen_static::Length::FILL },
        quote! { ::lumen_static::Length::Shrink },
    );
    let _ = (n_f32, spaced_total);

    // `fuse` is a *call style*, not a separate lowering: the same bodies
    // with static calls, so the subtree monomorphizes into one kind.
    let call_child = |m: &str| {
        let m = syn::Ident::new(m, proc_macro2::Span::call_site());
        if fuse {
            quote! { let d = &mut self.#m; }
        } else {
            quote! { let d: &mut dyn ::lumen_static::Node = &mut self.#m; }
        }
    };
    let call_child_ref = |m: &str| {
        let m = syn::Ident::new(m, proc_macro2::Span::call_site());
        if fuse {
            quote! { let d = &self.#m; }
        } else {
            quote! { let d: &dyn ::lumen_static::Node = &self.#m; }
        }
    };
    let mut_calls: Vec<Ts2> = field_ident.iter().map(|f| call_child(&f.to_string())).collect();
    let ref_calls: Vec<Ts2> = field_ident.iter().map(|f| call_child_ref(&f.to_string())).collect();
    // Each child's start index as a `const` expression, so the hit walk
    // can run in **reverse** without accumulating `at` forward. A stack
    // paints in field order, so the last field is on top and must be
    // hit first; every other container may walk forward because its
    // children cannot overlap (M11/T11.6).
    let child_at: Vec<Ts2> = (0..n)
        .map(|k| {
            let prev = field_ty[..k].to_vec();
            quote! { i + 1 #( + <#prev as ::lumen_static::widget::Widget>::NODE_COUNT )* }
        })
        .collect();

    let opaque_stmt = if cfg!(feature = "rdyn_opaque") && !fuse {
        quote! { let d = ::core::hint::black_box(d); }
    } else {
        quote! {}
    };

    // Declared, not inferred: `W1004` must tell an intended overlap from
    // an accidental one, and geometry cannot.
    let overlaps_impl = match axis {
        Axis::Stack => quote! {
            fn overlaps_children(&self) -> bool {
                true
            }
        },
        // A flow's children cannot overlap: each is placed past the
        // last, and a wrap moves *down* rather than back over.
        Axis::Row | Axis::Column | Axis::Flow(_) => quote! {},
    };

    // ---- the three bodies that differ by mode (M11/T11.6) ----
    //
    // `paint` is **not** among them: a stack paints its children in
    // field order exactly as a row does, and that order is what puts the
    // last field on top. Only measuring, placing and hitting change.
    let layout_use = match axis {
        Axis::Row | Axis::Column => quote! {
            use ::lumen_static::ops::{
                fill_child, inner_limits, loose_child, resolve_self, spaced, Axis,
            };
        },
        Axis::Stack => quote! {
            use ::lumen_static::ops::{inner_limits, resolve_self, stack_child};
        },
        Axis::Flow(_) => quote! {
            use ::lumen_static::ops::{inner_limits, resolve_self};
        },
    };
    let place_use = match axis {
        Axis::Row | Axis::Column => quote! {
            use ::lumen_static::ops::{align_offset, Axis};
        },
        Axis::Stack => quote! {
            use ::lumen_static::ops::align_offset;
        },
        // A flow positions from the sizes `layout` already recorded; it
        // needs no alignment helper, because a wrapped line is packed
        // from its start by definition.
        Axis::Flow(_) => quote! {},
    };

    // A grid's column count, and whether this flow is one (M57/A19): a
    // grid places every cell at its *column's* x, a wrap packs each line.
    // `flow_col` is a cell's column: a wrap has one, and emitting it as
    // `in_line % 1` put clippy's modulo-one lint on every crate that derives
    // a `wrap` (the M56 merge's clippy gate, `tests/flow.rs`).
    let (flow_cols, flow_grid, flow_col) = match axis {
        Axis::Flow(Some(c)) => (quote! { #c }, quote! { true }, quote! { in_line % #c }),
        _ => (quote! { 1usize }, quote! { false }, quote! { 0usize }),
    };
    // `None` wraps on the line's width; `Some(n)` breaks every `n`.
    let flow_break = match axis {
        Axis::Flow(Some(c)) => quote! { in_line >= #c },
        _ => quote! { in_line > 0 && cur + s.w > inner.max_w + 0.01 },
    };
    let layout_body = match axis {
        Axis::Row | Axis::Column => quote! {
                    let axis = #axis_expr;
                    let inner = inner_limits(limits, me.padding);
                    let gaps = spaced(#n, me.spacing);
                    // What `Fill` divides up. Normally the inner
                    // maximum; inside a scroll region that maximum is
                    // unbounded, and dividing infinity among portions
                    // produces NaN sizes (A5). `fill_extent` answers
                    // with the scroller's viewport there.
                    let fill_span = cx.fill_extent(
                        axis.main_of(inner),
                        ::lumen_static::ops::is_vertical(axis),
                    );

                    // Scan 1: everything that is not Fill along the main
                    // axis. Shrink children measure their content here.
                    let mut used = gaps;
                    let mut cross = 0.0f32;
                    let mut portions: u32 = 0;
                    let mut at = i + 1;
                    #(
                        {
                            let cs = style[at];
                            let l = axis.main_len(&cs);
                            if l.is_fill() {
                                portions += l.portion() as u32;
                            } else {
                                let avail = (axis.main_of(inner) - used).max(0.0);
                                #mut_calls
                                #opaque_stmt
                                let s = d.layout(cx, loose_child(axis, inner, avail, me.align), style, st, at);
                                used += axis.main(s);
                                // A child that stretches across the
                                // container conforms to it; it must not
                                // also define its cross size, or a
                                // `Fill` spacer in a `Shrink` row would
                                // inflate the row to the whole viewport.
                                if !axis.cross_len(&cs).is_fill() {
                                    cross = cross.max(axis.cross(s));
                                }
                            }
                            at += <#field_ty as ::lumen_static::widget::Widget>::NODE_COUNT;
                        }
                    )*

                    // Scan 2: divide what is left among the fills. No
                    // freeze-and-redistribute loop exists, because
                    // nothing here can be squeezed below its content.
                    // Skipped entirely when the container has no fills,
                    // which is the common case.
                    let free = (fill_span - used).max(0.0);
                    let mut at = i + 1;
                    if portions != 0 {
                    #(
                        {
                            let cs = style[at];
                            let l = axis.main_len(&cs);
                            if l.is_fill() {
                                let share = if portions == 0 {
                                    0.0
                                } else {
                                    free * l.portion() as f32 / portions as f32
                                };
                                #mut_calls
                                #opaque_stmt
                                let s = d.layout(cx, fill_child(axis, inner, share, me.align), style, st, at);
                                used += axis.main(s);
                                if !axis.cross_len(&cs).is_fill() {
                                    cross = cross.max(axis.cross(s));
                                }
                            }
                            at += <#field_ty as ::lumen_static::widget::Widget>::NODE_COUNT;
                        }
                    )*
                    }
                    let _ = at;

                    let content = axis.size(used, cross);
                    let padded = ::lumen_static::Size {
                        w: content.w + me.padding.horizontal(),
                        h: content.h + me.padding.vertical(),
                    };
        },
        // **Every child is offered the same box.** There is no scan for
        // fills and no remaining space to divide, because nothing is
        // divided: the container's size is the largest child on each
        // axis, independently.
        Axis::Stack => quote! {
            let inner = inner_limits(limits, me.padding);
            // A stack inside a scroll region. The scroller's axis is
            // unbounded, and `Fill` there cannot mean the maximum
            // because it is infinite (A5) -- `fill_extent` answers with
            // the viewport. Row and column ask this on their main axis;
            // a stack has none, so it asks on both.
            let box_ = ::lumen_static::Constraints {
                min_w: inner.min_w,
                max_w: cx.fill_extent(inner.max_w, false),
                min_h: inner.min_h,
                max_h: cx.fill_extent(inner.max_h, true),
            };
            let kid = stack_child(box_, me.align);
            let mut cw = 0.0f32;
            let mut ch = 0.0f32;
            let mut at = i + 1;
            #(
                {
                    #mut_calls
                    #opaque_stmt
                    let s = d.layout(cx, kid, style, st, at);
                    cw = cw.max(s.w);
                    ch = ch.max(s.h);
                    at += <#field_ty as ::lumen_static::widget::Widget>::NODE_COUNT;
                }
            )*
            let _ = at;
            // `record_painted` is the template's, shared with the
            // row/column body -- one place where a node's own box is
            // recorded, not two that must agree forever.
            let padded = ::lumen_static::Size {
                w: cw + me.padding.horizontal(),
                h: ch + me.padding.vertical(),
            };
        },
        // ---- a flow: children left to right, wrapping -------------
        //
        // **Measured loosely, every child.** A flow cannot divide `Fill`
        // among its children the way a row does, because it does not
        // know how many will share a line until it has measured them —
        // so a `Fill` child in a flow takes its natural size. That is a
        // real limitation and it is stated here rather than discovered:
        // a wrap of `Fill` items is a column with extra steps.
        Axis::Flow(_) => quote! {
            let inner = inner_limits(limits, me.padding);
            let loose = ::lumen_static::Constraints {
                min_w: 0.0,
                max_w: inner.max_w,
                min_h: 0.0,
                max_h: inner.max_h,
            };
            let mut at = i + 1;
            // The line being filled, and what is finished above it.
            let mut cur = 0.0f32;
            let mut line_h = 0.0f32;
            let mut in_line: usize = 0;
            let mut cw = 0.0f32;
            let mut ch = 0.0f32;
            // A grid's column widths: the widest cell in each (M57/A19).
            let mut col_w = [0.0f32; #flow_cols];
            #(
                {
                    #mut_calls
                    #opaque_stmt
                    let s = d.layout(cx, loose, style, st, at);
                    if #flow_grid {
                        let c = #flow_col;
                        if s.w > col_w[c] { col_w[c] = s.w; }
                    }
                    if #flow_break {
                        // Close the line: its width is what was placed
                        // minus the trailing gap, and the next line
                        // starts `spacing` below it.
                        cw = cw.max(cur - me.spacing);
                        ch += line_h + me.spacing;
                        cur = 0.0;
                        line_h = 0.0;
                        in_line = 0;
                    }
                    cur += s.w + me.spacing;
                    in_line += 1;
                    if s.h > line_h { line_h = s.h; }
                    at += <#field_ty as ::lumen_static::widget::Widget>::NODE_COUNT;
                }
            )*
            let _ = at;
            let _ = in_line;
            // The last line, which no break closed.
            cw = cw.max(cur - me.spacing).max(0.0);
            ch += line_h;
            // A grid is as wide as its columns, not as its widest line:
            // cells are placed on shared column edges (M57/A19).
            if #flow_grid {
                let used = col_w.iter().filter(|w| **w > 0.0).count();
                cw = col_w.iter().sum::<f32>() + me.spacing * used.saturating_sub(1) as f32;
            }
            let padded = ::lumen_static::Size {
                w: cw + me.padding.horizontal(),
                h: ch + me.padding.vertical(),
            };
        },
    };

    let place_body = match axis {
        Axis::Row | Axis::Column => quote! {
                    let axis = #axis_expr;
                    let cross_extent = axis.cross(st[i].size)
                        - match axis {
                            Axis::Row => me.padding.vertical(),
                            Axis::Column => me.padding.horizontal(),
                        };
                    let mut main = axis.main(::lumen_static::Size { w: at_pos.x, h: at_pos.y })
                        + match axis {
                            Axis::Row => me.padding.left,
                            Axis::Column => me.padding.top,
                        };
                    let cross0 = axis.cross(::lumen_static::Size { w: at_pos.x, h: at_pos.y })
                        + match axis {
                            Axis::Row => me.padding.top,
                            Axis::Column => me.padding.left,
                        };
                    let mut at = i + 1;
                    #(
                        {
                            // Stretch a cross-`Fill` child to the extent
                            // its container settled on. Its descendants
                            // were measured against the looser bound —
                            // the documented non-iterative limit.
                            // A hidden child is not stretched: `layout`
                            // zeroed its subtree, and cross-stretching it
                            // back to the container's extent would leave
                            // a hidden node reporting real geometry to
                            // the journal, the audit and the agent.
                            if style[at].visible && axis.cross_len(&style[at]).is_fill() {
                                let s = st[at].size;
                                st[at].size = match axis {
                                    Axis::Row => ::lumen_static::Size { w: s.w, h: cross_extent },
                                    Axis::Column => ::lumen_static::Size { w: cross_extent, h: s.h },
                                };
                            }
                            let size = st[at].size;
                            let off = align_offset(me.align, cross_extent, axis.cross(size));
                            let p = match axis {
                                Axis::Row => ::lumen_static::Point { x: main, y: cross0 + off },
                                Axis::Column => ::lumen_static::Point { x: cross0 + off, y: main },
                            };
                            #mut_calls
                            d.place(p, style, st, at);
                            main += axis.main(size) + me.spacing;
                            at += <#field_ty as ::lumen_static::widget::Widget>::NODE_COUNT;
                        }
                    )*
                    let _ = (at, main);
        },
        Axis::Stack => quote! {
            let inner_w = (st[i].size.w - me.padding.horizontal()).max(0.0);
            let inner_h = (st[i].size.h - me.padding.vertical()).max(0.0);
            let x0 = at_pos.x + me.padding.left;
            let y0 = at_pos.y + me.padding.top;
            let mut at = i + 1;
            #(
                {
                    // Stretch a `Fill` child to the box the container
                    // settled on -- the step a row takes for a
                    // cross-`Fill` child, on both axes because a stack
                    // has no cross axis. Its descendants were measured
                    // against the looser bound, which is the documented
                    // non-iterative limit. A hidden child is left alone:
                    // `layout` zeroed its subtree and stretching it back
                    // would have a hidden node reporting real geometry.
                    if style[at].visible {
                        let s = st[at].size;
                        st[at].size = ::lumen_static::Size {
                            w: if style[at].width.is_fill() { inner_w } else { s.w },
                            h: if style[at].height.is_fill() { inner_h } else { s.h },
                        };
                    }
                    let size = st[at].size;
                    let p = ::lumen_static::Point {
                        x: x0 + align_offset(me.align, inner_w, size.w),
                        y: y0 + align_offset(me.align, inner_h, size.h),
                    };
                    #mut_calls
                    d.place(p, style, st, at);
                    at += <#field_ty as ::lumen_static::widget::Widget>::NODE_COUNT;
                }
            )*
            let _ = at;
        },
        // **The same walk, from the sizes `layout` recorded.** `place`
        // has no `LayoutCx` and must not re-measure; every child's size
        // is already in `st`, so the break test can be re-run against it
        // and must produce the same lines. The two are written to look
        // alike on purpose: a flow whose place disagreed with its layout
        // would draw children outside the box it reported.
        Axis::Flow(_) => quote! {
            let p = style[i].padding;
            let inner = ::lumen_static::Constraints {
                min_w: 0.0,
                max_w: (st[i].size.w - p.horizontal()).max(0.0),
                min_h: 0.0,
                max_h: (st[i].size.h - p.vertical()).max(0.0),
            };
            let me = style[i];
            // **Pass 1: each line's width, and a grid's column widths** —
            // M57/A19. A grid placed each cell after the previous one on
            // its line, so a column's x depended on the widths of the cells
            // before it and unequal cells made jagged columns; and every
            // line was packed from the left, whatever `align` said.
            let mut line_w = [0.0f32; #n + 1];
            let mut col_w = [0.0f32; #flow_cols];
            {
                let mut at = i + 1;
                let mut cur = 0.0f32;
                let mut in_line: usize = 0;
                let mut line: usize = 0;
                #(
                    {
                        let s = st[at].size;
                        if #flow_break {
                            line_w[line] = cur - me.spacing;
                            line += 1;
                            cur = 0.0;
                            in_line = 0;
                        }
                        if #flow_grid {
                            let c = #flow_col;
                            if s.w > col_w[c] { col_w[c] = s.w; }
                        }
                        cur += s.w + me.spacing;
                        in_line += 1;
                        at += <#field_ty as ::lumen_static::widget::Widget>::NODE_COUNT;
                    }
                )*
                line_w[line] = (cur - me.spacing).max(0.0);
                let _ = in_line;
            }
            let grid_w = col_w.iter().sum::<f32>()
                + me.spacing * col_w.iter().filter(|w| **w > 0.0).count().saturating_sub(1) as f32;
            let line_off = |w: f32| ::lumen_static::ops::align_offset(me.align, inner.max_w, w);
            let mut at = i + 1;
            let mut cur = 0.0f32;
            let mut line_y = 0.0f32;
            let mut line_h = 0.0f32;
            let mut in_line: usize = 0;
            let mut line: usize = 0;
            #(
                {
                    let s = st[at].size;
                    if #flow_break {
                        line_y += line_h + me.spacing;
                        cur = 0.0;
                        line_h = 0.0;
                        in_line = 0;
                        line += 1;
                    }
                    #mut_calls
                    #opaque_stmt
                    // A grid cell sits at its column's edge; a wrapped
                    // item after the one before it, its line aligned.
                    let x = if #flow_grid {
                        let c = #flow_col;
                        line_off(grid_w) + col_w[..c].iter().map(|w| w + me.spacing).sum::<f32>()
                    } else {
                        line_off(line_w[line]) + cur
                    };
                    d.place(
                        ::lumen_static::Point {
                            x: at_pos.x + p.left + x,
                            y: at_pos.y + p.top + line_y,
                        },
                        style,
                        st,
                        at,
                    );
                    cur += s.w + me.spacing;
                    in_line += 1;
                    if s.h > line_h { line_h = s.h; }
                    at += <#field_ty as ::lumen_static::widget::Widget>::NODE_COUNT;
                }
            )*
            let _ = (at, in_line, line_y, inner, line);
        },
    };

    // Reversed for a stack, and only for a stack. `child_at` exists so
    // this needs no running offset -- walking backwards while
    // accumulating forwards is how you get an off-by-one that only
    // shows up on the third child.
    let hit_rev_at: Vec<Ts2> = child_at.iter().rev().cloned().collect();
    let hit_rev_ref: Vec<Ts2> = ref_calls.iter().rev().cloned().collect();
    let hit_body = match axis {
        // A flow walks forward like a row: its children cannot overlap.
        Axis::Row | Axis::Column | Axis::Flow(_) => quote! {
                    let mut at = i + 1;
                    #(
                        {
                            #ref_calls
                            if let ::core::option::Option::Some(h) = d.hit(st, at, x, y) {
                                return ::core::option::Option::Some(h);
                            }
                            at += <#field_ty as ::lumen_static::widget::Widget>::NODE_COUNT;
                        }
                    )*
                    let _ = at;
        },
        Axis::Stack => quote! {
            #(
                {
                    let at = #hit_rev_at;
                    #hit_rev_ref
                    if let ::core::option::Option::Some(h) = d.hit(st, at, x, y) {
                        return ::core::option::Option::Some(h);
                    }
                }
            )*
        },
    };

    let __out = quote! {
        const _: () = {
            // No `use` block: it would shadow a user type of the same
            // name at the point the impl is written, so a widget struct
            // named `Size`, `Node` or `Point` failed with an error
            // naming a framework type it never mentioned. Everything is
            // spelled out instead (F6.2).

            const TABLE: [::lumen_static::desc::NodeDesc; <#name as ::lumen_static::widget::Widget>::NODE_COUNT] = {
                let mut t = [::lumen_static::desc::NodeDesc::EMPTY; <#name as ::lumen_static::widget::Widget>::NODE_COUNT];
                t[0] = ::lumen_static::desc::NodeDesc::new(
                    #first_child,
                    ::lumen_static::NO_PARENT,
                    0,
                    #child_count,
                    {
                        let mut f = if #has_children { ::lumen_static::desc::NodeDesc::CONTAINER } else { 0 };
                        if <#name as ::lumen_static::widget::Widget>::IS_RIGID { f |= ::lumen_static::desc::NodeDesc::RIGID; }
                        f
                    },
                )
                .with_src(::core::file!(), #root_line)
                .with_classes(#root_classes);
                let mut at: usize = 1;
                let mut prev_root: usize = 0;
                #(
                    ::lumen_static::widget::splice_child(
                        &mut t, at, <#field_ty as ::lumen_static::widget::Widget>::SUB_TABLE, 0,
                    );
                    // A child's root carries its *field* site; the
                    // child's internals keep their own.
                    t[at] = t[at]
                        .with_src(::core::file!(), #field_line)
                        .with_id(#field_wid)
                        .with_classes(#field_classes);
                    if prev_root != 0 { t[prev_root].next_sibling = at as u32; }
                    prev_root = at;
                    at += <#field_ty as ::lumen_static::widget::Widget>::NODE_COUNT;
                )*
                let _ = (at, prev_root);
                t
            };

            impl ::lumen_static::widget::Widget for #name {
                const NODE_COUNT: usize = 1 #(+ <#field_ty as ::lumen_static::widget::Widget>::NODE_COUNT)*;
                const DEPTH: usize = 1 + {
                    let mut d = 0usize;
                    #( if <#field_ty as ::lumen_static::widget::Widget>::DEPTH > d { d = <#field_ty as ::lumen_static::widget::Widget>::DEPTH; } )*
                    d
                };
                const IS_RIGID: bool = true #(&& <#field_ty as ::lumen_static::widget::Widget>::IS_RIGID)*;
                const SUB_TABLE: &'static [::lumen_static::desc::NodeDesc] = &TABLE;

                const KIND: &'static str = #name_str;

                // **The static bind walk** — M52/T52.5. Each field's own
                // walk, on its concrete type: no `&dyn Node`, so a bound
                // tree links no vtable for `bind::node_of`'s sake.
                fn bind_walk(&self, target: usize) -> ::core::option::Option<usize> {
                    let lo = self as *const Self as usize;
                    if target < lo || target >= lo + ::core::mem::size_of::<Self>() {
                        return ::core::option::Option::None;
                    }
                    let mut at: usize = 1;
                    #(
                        if let ::core::option::Option::Some(j) =
                            <#field_ty as ::lumen_static::widget::Widget>::bind_walk(&self.#field_ident, target)
                        {
                            return ::core::option::Option::Some(at + j);
                        }
                        at += <#field_ty as ::lumen_static::widget::Widget>::NODE_COUNT;
                    )*
                    let _ = at;
                    ::core::option::Option::Some(0)
                }

                fn type_defaults() -> ::lumen_static::Style {
                    ::lumen_static::Style {
                        width: #default_w,
                        height: #default_h,
                        spacing: #sp,
                        ..::lumen_static::Style::default()
                    }
                }

                const SEEDS: &'static [::lumen_static::style::Seed] = &SEEDS;
            }

            /// One seed per node, spliced from the fields exactly as
            /// `TABLE` is — the cascade reads this instead of walking the
            /// live tree, so a hidden subtree still resolves from its own
            /// type (M24/T24.0).
            const SEEDS: [::lumen_static::style::Seed;
                1 #(+ <#field_ty as ::lumen_static::widget::Widget>::NODE_COUNT)*] = {
                let mut s = [::lumen_static::style::Seed::WIDGET;
                    1 #(+ <#field_ty as ::lumen_static::widget::Widget>::NODE_COUNT)*];
                s[0] = ::lumen_static::style::Seed::new(
                    <#name as ::lumen_static::widget::Widget>::KIND,
                    <#name as ::lumen_static::widget::Widget>::type_defaults, ::lumen_static::style::Fields::of(&[::lumen_static::style::Fields::WIDTH, ::lumen_static::style::Fields::HEIGHT, ::lumen_static::style::Fields::PADDING, ::lumen_static::style::Fields::SPACING, ::lumen_static::style::Fields::ALIGN, ::lumen_static::style::Fields::CURSOR, ::lumen_static::style::Fields::VISIBLE, ::lumen_static::style::Fields::BACKGROUND, ::lumen_static::style::Fields::BORDER, ::lumen_static::style::Fields::BACKGROUND_FILL, ::lumen_static::style::Fields::RADIUS, ::lumen_static::style::Fields::SHADOW, ::lumen_static::style::Fields::PRESSED_DIM, ::lumen_static::style::Fields::HOVER_DIM]));
                let mut at: usize = 1;
                #(
                    ::lumen_static::widget::splice_seeds(
                        &mut s, at, <#field_ty as ::lumen_static::widget::Widget>::SEEDS,
                    );
                    at += <#field_ty as ::lumen_static::widget::Widget>::NODE_COUNT;
                )*
                let _ = at;
                s
            };

            // Proof-carrying table: the compiler re-checks the macro's
            // own output at every expansion, and the depth cap turns the
            // stack-overflow hazard into a compile error.
            const _: () = assert!(::lumen_static::widget::table_is_well_formed(&TABLE));
            const _: () = assert!(<#name as ::lumen_static::widget::Widget>::DEPTH <= ::lumen_static::MAX_DEPTH);
            // **The lengths, asserted** (M26/T26.16).
            //
            // "The derive half is unfalsifiable" is true of exactly one
            // of four cases: only the *struct* derive's table length
            // literally is the node count. The struct derive's **seed**
            // length and both of the enum derive's lengths are
            // re-spelled sums and can drift from it — a splice written
            // with one offset wrong produces a table of the right
            // length and a seeds array of the wrong one, and nothing
            // said so.
            const _: () = assert!(
                TABLE.len() == <#name as ::lumen_static::widget::Widget>::NODE_COUNT,
                "the spliced table's length is not this tree's NODE_COUNT",
            );
            const _: () = assert!(
                SEEDS.len() == <#name as ::lumen_static::widget::Widget>::NODE_COUNT,
                "the spliced seeds array's length is not this tree's NODE_COUNT",
            );

            /// Delegates by index range, like `node`. The where-clause
            /// is what makes a mis-typed `Button<Other>` a compile error
            /// at the *container* that holds it.
            impl<__LnS> ::lumen_static::event::Handlers<__LnS> for #name
            where
                #(#field_ty: ::lumen_static::event::Handlers<__LnS>,)*
            {
                fn handler(
                    &self,
                    sel: ::lumen_static::event::NodeSel,
                ) -> ::core::option::Option<fn(&mut __LnS, &::lumen_static::EventCtx)> {
                    if sel.i == 0 { return ::core::option::Option::None; }
                    let mut at: usize = 1;
                    #(
                        {
                            let c = <#field_ty as ::lumen_static::widget::Widget>::NODE_COUNT;
                            if sel.i < at + c {
                                return ::lumen_static::event::Handlers::handler(
                                    &self.#field_ident,
                                    ::lumen_static::event::NodeSel { i: sel.i - at, inner: sel.inner },
                                );
                            }
                            at += c;
                        }
                    )*
                    let _ = at;
                    ::core::option::Option::None
                }
            }

            impl ::lumen_static::widget::Node for #name {
                fn kind_name(&self) -> &'static str {
                    <Self as ::lumen_static::widget::Widget>::KIND
                }

                // M48/T48.7: the agent's structural stand-in test.
                ::lumen_static::node_count!();

                fn defaults(&self) -> ::lumen_static::Style {
                    <Self as ::lumen_static::widget::Widget>::type_defaults()
                }

                /// iced's `resolve`, per container: one scan lays out the
                /// non-`Fill` children (which is where `Shrink` gets its
                /// content size), a second divides the remainder among
                /// the `Fill` ones. Each child's `layout` is called
                /// exactly once — no intrinsic pass, no correction pass
                /// (`docs/layout-and-storage-decisions.md` §2).
                fn layout(
                    &mut self,
                    cx: &mut ::lumen_static::LayoutCx,
                    limits: ::lumen_static::Constraints,
                    style: &[::lumen_static::Style],
                    st: &mut [::lumen_static::desc::NodeState],
                    i: usize,
                ) -> ::lumen_static::Size {
                    #layout_use
                    // **The relayout boundary** — `docs/plan-invalidation.md`
                    // T16.9, and the property the design spec's §2 ranks
                    // first of the four that produce performance.
                    //
                    // A subtree nothing marked, re-measured under the
                    // constraints it was already measured under, cannot
                    // have a different size. `st[i]` still holds that
                    // size and `record` still holds the limits it was
                    // taken under, so the answer is already here and the
                    // walk below is pure cost.
                    //
                    // Both halves are required. The dirty set alone is
                    // not enough: a clean subtree given *different*
                    // limits genuinely re-resolves, which is what
                    // happens to every child of a container that
                    // changed width. And the constraints alone are not
                    // enough: identical limits say nothing about
                    // whether the content changed.
                    //
                    // Under `Everything` — the oracle — `can_skip()` is
                    // false, so this is a load and a branch the
                    // optimizer removes, and the code below is what the
                    // crate shipped before T16.9.
                    //
                    // `place` still runs over the skipped subtree: a
                    // node whose *size* did not change may still have
                    // moved, because a sibling before it changed. Size
                    // and position are separate questions and only the
                    // first one is answered here.
                    if cx.changed.can_skip()
                        && cx.changed.is_clean(
                            i..i + <Self as ::lumen_static::widget::Widget>::NODE_COUNT,
                        )
                        && st[i].constraints == limits
                    {
                        return st[i].size;
                    }
                    let me = style[i];
                    if !me.visible {
                        // The whole subtree, not just this node. Zeroing
                        // only `i` left descendants holding last frame's
                        // geometry, which `paint` then drew and the
                        // journal and audit then read — a hidden panel
                        // stayed on screen at its old position
                        // (`docs/plan-review-fixes.md` F1.6).
                        return ::lumen_static::ops::hide_subtree(
                            st, i, limits, <Self as ::lumen_static::widget::Widget>::NODE_COUNT,
                        );
                    }
                    // **The content is laid out inside the container's
                    // own size**, where that size is `Fixed` — not across
                    // the whole offer (`ops::within_own_size`, M56). The
                    // offer itself is still what is recorded and what the
                    // box resolves against, so the relayout boundary
                    // above keys on the same limits it always did.
                    let offered = limits;
                    let limits = ::lumen_static::ops::within_own_size(offered, &me);
                    #layout_body
                    ::lumen_static::ops::record_painted(
                        st, i, offered, resolve_self(offered, &me, padded), &me,
                    )
                }

                fn place(
                    &mut self,
                    at_pos: ::lumen_static::Point,
                    style: &[::lumen_static::Style],
                    st: &mut [::lumen_static::desc::NodeState],
                    i: usize,
                ) {
                    #place_use
                    st[i].pos = at_pos;
                    let me = style[i];
                    if !me.visible {
                        // Collapse descendant positions too, so nothing
                        // downstream can read a stale one.
                        for k in i..(i + <Self as ::lumen_static::widget::Widget>::NODE_COUNT).min(st.len()) {
                            st[k].pos = at_pos;
                        }
                        return;
                    }
                    #place_body
                }

                #overlaps_impl

                fn paint(
                    &self,
                    st: &[::lumen_static::desc::NodeState],
                    i: usize,
                    out: &mut dyn ::lumen_static::Painter,
                ) {
                    // A container's own shadow and fill, before its
                    // children. This is what makes a card a card.
                    if st[i].shadow > 0.0 {
                        out.shadow(
                            ::lumen_static::ops::rect_of(&st[i]),
                            st[i].radius,
                            st[i].shadow,
                        );
                    }
                    // M82/T82.3: the node's fill table entry, if it has
                    // one, under the flat `bg` so a background colour
                    // still covers it.
                    ::lumen_static::ops::fill(st, i, out);
                    // **Guarded, like the shadow above it** (M26/T26.9).
                    // A fully transparent fill draws nothing a renderer
                    // can see, and every layout container in a tree has
                    // one — so this emitted a command per container per
                    // frame that retention then hashed, retained a slot
                    // for, and handed to a sink that dropped it. `hit`
                    // has had the same predicate since `examples/glass`
                    // found an empty wrapper swallowing the frame's
                    // input; now `paint` agrees with it.
                    if st[i].bg.3 != 0 {
                        out.rounded_rect(
                            ::lumen_static::ops::rect_of(&st[i]),
                            st[i].bg,
                            st[i].radius,
                        );
                    }
                    // Over the fill, under the children: a card's border
                    // belongs to the card, not in front of its content.
                    ::lumen_static::ops::border(st, i, out);
                    let mut at = i + 1;
                    #(
                        {
                            #ref_calls
                            d.paint(st, at, out);
                            at += <#field_ty as ::lumen_static::widget::Widget>::NODE_COUNT;
                        }
                    )*
                    let _ = at;
                }

                fn hit(&self, st: &[::lumen_static::desc::NodeState], i: usize, x: f32, y: f32)
                    -> ::core::option::Option<::lumen_static::Hit>
                {
                    if !::lumen_static::hit::contains(&st[i], x, y) {
                        return ::core::option::Option::None;
                    }
                    #hit_body
                    // **A container claims a hit only where it painted
                    // something.** Hit testing follows painting: a
                    // transparent gap between children was never drawn,
                    // so nothing was clicked there.
                    //
                    // Invisible in a row or a column, whose children
                    // cannot overlap -- declining there only returns the
                    // parent instead of the child, and the handler walk
                    // ends up in the same place. It matters in a
                    // `stack`, where a later transparent layer sits
                    // *over* the content and is hit first: without this
                    // an empty layout wrapper swallows the whole frame's
                    // input, which is what it did to `examples/glass`.
                    if st[i].bg.3 == 0 && st[i].shadow <= 0.0 {
                        return ::core::option::Option::None;
                    }
                    ::core::option::Option::Some(::lumen_static::Hit {
                        node: i as u32,
                        inner: ::core::option::Option::None,
                        local: ::lumen_static::Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
                    })
                }

                fn node(&self, i: usize) -> &dyn ::lumen_static::widget::Node {
                    if i == 0 { return self; }
                    let mut at: usize = 1;
                    #(
                        {
                            let c = <#field_ty as ::lumen_static::widget::Widget>::NODE_COUNT;
                            if i < at + c { return ::lumen_static::widget::Node::node(&self.#field_ident, i - at); }
                            at += c;
                        }
                    )*
                    let _ = at;
                    ::core::unreachable!("node index out of range")
                }

                fn node_mut(&mut self, i: usize) -> &mut dyn ::lumen_static::widget::Node {
                    if i == 0 { return self; }
                    let mut at: usize = 1;
                    #(
                        {
                            let c = <#field_ty as ::lumen_static::widget::Widget>::NODE_COUNT;
                            if i < at + c {
                                return ::lumen_static::widget::Node::node_mut(&mut self.#field_ident, i - at);
                            }
                            at += c;
                        }
                    )*
                    let _ = at;
                    ::core::unreachable!("node index out of range")
                }

                // **The static observe walk** — M35/T35.1. This node's
                // entry, then each field's subtree through
                // `emit_subtree`, which walks a field statically when it
                // has a walk of its own. So a derived row inside a `List`
                // or `Repeat` is never coerced to `&dyn Node` for the
                // agent's sake. Gated through `__a11y_items!` (M54/T54.1:
                // the walk follows the screen-reader tree, not the agent)
                // because this expands in the user's crate, where a `cfg`
                // would read the wrong features.
                ::lumen_static::__a11y_items! {
                    fn observe_walk(
                        &self,
                        _table: &[::lumen_static::desc::NodeDesc],
                        st: &[::lumen_static::NodeState],
                        id_of: &dyn Fn(usize) -> u64,
                        out: &mut ::lumen_static::observe::ObsVec<(
                            ::lumen_static::observe::NodeId,
                            ::lumen_static::observe::AkNode,
                        )>,
                    ) {
                        let table = <Self as ::lumen_static::widget::Widget>::SUB_TABLE;
                        ::lumen_static::observe::emit_node(table, st, 0, id_of, out, |n, o| {
                            ::lumen_static::widget::Node::observe(self, st, 0, id_of(0), n, o)
                        });
                        let mut at: usize = 1;
                        #(
                            {
                                let c = <#field_ty as ::lumen_static::widget::Widget>::NODE_COUNT;
                                let base = at;
                                let ids = |t: usize| id_of(base + t);
                                ::lumen_static::observe::emit_subtree(&self.#field_ident, &st[at..at + c], &ids, out);
                                at += c;
                            }
                        )*
                        let _ = at;
                    }
                }
            }
        };
    };
    Ok(__out)
}

// ---- #[derive(Reloadable)] (handoff §7) -------------------------------------

enum ReloadMode {
    /// `#[reload(keep)]` — serde data, migrated across the swap.
    Keep,
    /// `#[reload(fresh)]` / `#[reload(reset)]` — restore never touches it;
    /// the value from the fresh tree (new dylib's code) stands.
    Skip,
    /// No attribute — a nested `Reloadable` (widget subtree); recurse.
    Nested,
}

/// Emits `save` (keep fields as a JSON object) and `restore` with
/// per-field fallback: an incompatible or missing saved field keeps its
/// fresh value and is logged — the §12.2 rule applied to every field,
/// enum variants included. A fn pointer marked `keep` fails to compile
/// (no `Serialize`), keeping §7's "Saved cannot contain code pointers"
/// property structural. No constructor: an application is built by its
/// program's boot (M49/T49.4).
///
/// **On a widget enum** (M49/T49.5) — the shape `StaticTree` accepts,
/// one widget per variant — it saves which variant shows, by name, and
/// that variant's own state; a variant the new build lacks keeps the
/// fresh one, logged.
#[proc_macro_derive(Reloadable, attributes(reload))]
pub fn derive_reloadable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_reloadable(input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// A field's reload mode: `keep`, `fresh`/`reset`, or none (a nested
/// `Reloadable`).
///
/// **Once** — M55/T55.2 (R16-F4). The last mode used to win, so
/// `#[reload(keep)] #[reload(fresh)]` dropped the value the first
/// attribute asked to keep, silently.
fn reload_mode(field: &syn::Field) -> syn::Result<ReloadMode> {
    let mut mode = ReloadMode::Nested;
    let mut first: Option<syn::Path> = None;
    for attr in &field.attrs {
        if !attr.path().is_ident("reload") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            let m = if meta.path.is_ident("keep") {
                ReloadMode::Keep
            } else if meta.path.is_ident("fresh") || meta.path.is_ident("reset") {
                ReloadMode::Skip
            } else {
                return Err(meta.error("expected `keep`, `fresh`, or `reset`"));
            };
            if let Some(prev) = &first {
                let mut e = meta.error("a field has one reload mode");
                e.combine(syn::Error::new_spanned(prev, "the first is here"));
                return Err(e);
            }
            first = Some(meta.path.clone());
            mode = m;
            Ok(())
        })?;
    }
    Ok(mode)
}

fn expand_reloadable(input: DeriveInput) -> syn::Result<Ts2> {
    reject_generics(&input, "Reloadable")?;
    let name = &input.ident;
    let name_str = name.to_string();
    if let Data::Enum(data) = &input.data {
        return expand_reloadable_enum(name, &name_str, data);
    }
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(&input.ident, "Reloadable derives structs and widget enums, not unions"));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(&input.ident, "Reloadable needs named fields"));
    };

    let mut save_stmts = Vec::new();
    let mut restore_stmts = Vec::new();
    for f in &fields.named {
        let ident = f.ident.clone().unwrap();
        let key = ident.to_string();
        match reload_mode(f)? {
            ReloadMode::Keep => {
                // Both halves live in the core (`save_keep`,
                // `restore_keep`), so the rules — a failed save is left
                // out, a null over a non-null fresh value is refused —
                // are code the core's own tests reach (M49/T49.6).
                // Spanned on the field's type (M49/T49.8, R5-F7): a type
                // with no `Serialize` is reported on the field, not on
                // the derive.
                let ty = &f.ty;
                save_stmts.push(quote_spanned! {ty.span()=>
                    ::lumen_static::reload::save_keep(&mut m, &self.#ident, #name_str, #key);
                });
                restore_stmts.push(quote_spanned! {ty.span()=>
                    ::lumen_static::reload::restore_keep(&mut self.#ident, obj.get(#key), #name_str, #key);
                });
            }
            ReloadMode::Skip => {}
            ReloadMode::Nested => {
                // Reported on the field (M49/T49.8, R5-F7), with
                // `Reloadable`'s `on_unimplemented` note naming the
                // attribute to add. **The spelling is what does it**:
                // `<#ty as Reloadable>::save` carries the field type's own
                // tokens, so the unmet bound is reported at them. With
                // `Reloadable::save(&self.#ident)` the error landed on the
                // derive whether or not the statement was spanned — the
                // injection that removed only the span left the gate
                // green, and that is how this was found.
                let ty = &f.ty;
                save_stmts.push(quote_spanned! {ty.span()=>
                    m.insert(#key.to_owned(), <#ty as ::lumen_static::reload::Reloadable>::save(&self.#ident));
                });
                restore_stmts.push(quote_spanned! {ty.span()=>
                    match obj.get(#key) {
                        Some(v) => <#ty as ::lumen_static::reload::Reloadable>::restore(&mut self.#ident, v),
                        // A widget added by this build that keeps nothing
                        // (a label, say) lost nothing: no line, so the
                        // bar does not cry wolf on every added node.
                        None if <#ty as ::lumen_static::reload::Reloadable>::save(&self.#ident).is_null() => {}
                        None => ::lumen_static::reload::log_fallback(#name_str, #key, "missing from saved state"),
                    }
                });
            }
        }
    }

    // **An application is the thing with a `#[tree]` field** — M31.
    //
    // `#[tree]` is the `App` derive's attribute, but a field attribute
    // is visible to *every* derive on the struct, so this one can tell
    // an application from a widget with no new attribute and no
    // coordination between the two derives.
    //
    // It used to decide where `fresh()` came from: `Application::build`
    // for an application, `Default` for a widget. **`fresh()` is gone**
    // (M49/T49.4, R5-F10): an application is built by its program's boot
    // — the window swap boots the new library's `program()`, and
    // `Host::reload` re-runs the boot the host was built from — so
    // routing a second constructor here was M31's two-path defect back
    // in a new shape, and it made `application(boot)` with a free
    // function un-reloadable (E0277 on `Application`). A container that
    // needs a fresh row takes `Default`. What remains keyed on `#[tree]`
    // is the guard below.
    let is_application = fields
        .named
        .iter()
        .any(|f| f.attrs.iter().any(|a| a.path().is_ident("tree")));

    // **A reloadable application must not also implement `Default`** —
    // M31/T31.2, kept by M49/T49.4.
    //
    // A `Default` beside the program's boot is a *second* construction
    // path that nothing reconciles: whatever calls one and whatever calls
    // the other drift until a handler assigned in only one of them comes
    // back as its placeholder. Nothing in the framework calls `Default`
    // on an application any more, which is exactly why a `Default` there
    // could only be a trap.
    //
    // Rust has no negative bounds, so this is the coherence trick: the
    // blanket covers `Self` only if `Self: Default`, and then the two
    // impls overlap and rustc refuses with E0119. When `Self` is not
    // `Default` the blanket does not apply and nothing conflicts. The
    // trait's *name* is the error message — E0119 prints it — which is
    // why it is a sentence rather than an identifier.
    //
    // Scoped to applications that reload, which is where the two paths
    // can exist at all: a test fixture that never swaps has one path
    // already and keeps `Default` (`docs/plan-application.md` T31.1).
    let no_default = if is_application {
        quote! {
            const _: () = {
                trait AnApplicationThatReloadsMustNotAlsoDeriveDefault {}
                impl<T: ::core::default::Default>
                    AnApplicationThatReloadsMustNotAlsoDeriveDefault for T {}
                impl AnApplicationThatReloadsMustNotAlsoDeriveDefault for #name {}
            };
        }
    } else {
        quote! {}
    };

    // Through the core's `__reloadable!` (M49/T49.9): nothing at all
    // unless the core's `reload` feature is on.
    Ok(quote! { ::lumen_static::__reloadable! {
        #no_default

        impl ::lumen_static::reload::Reloadable for #name {
            fn save(&self) -> ::lumen_static::reload::Value {
                let mut m = ::lumen_static::reload::Map::new();
                #(#save_stmts)*
                ::lumen_static::reload::Value::Object(m)
            }
            fn restore(&mut self, saved: &::lumen_static::reload::Value) {
                let Some(obj) = saved.as_object() else {
                    ::lumen_static::reload::log_fallback(#name_str, "<self>", "saved state is not an object");
                    return;
                };
                #(#restore_stmts)*
            }
        }
    }})
}

/// `#[derive(Reloadable)]` on a widget enum — M49/T49.5
/// (`docs/plan-reload-hardening.md` §2.3).
///
/// The shape `StaticTree` accepts: each variant holds exactly one widget.
/// It saves **which variant is showing, by name**, and that variant's own
/// save; restore switches to the saved variant (built with `Default`, as
/// every other fresh row is) and restores into it. A name the new build
/// does not have keeps the fresh variant and logs it — the §12.2 rule a
/// kept serde enum already follows. By name rather than index, so
/// reordering the variants in an edit keeps the user's place.
///
/// Before T49.5 the derive refused enums, so a `StaticTree` enum could
/// not sit inside a kept subtree at all.
fn expand_reloadable_enum(name: &syn::Ident, name_str: &str, data: &syn::DataEnum) -> syn::Result<Ts2> {
    let mut idents = Vec::new();
    let mut tys = Vec::new();
    for v in &data.variants {
        let syn::Fields::Unnamed(f) = &v.fields else {
            return Err(syn::Error::new_spanned(v, "a Reloadable enum's variants each hold one widget: `Variant(Widget)`"));
        };
        if f.unnamed.len() != 1 {
            return Err(syn::Error::new_spanned(v, "a Reloadable enum's variants each hold exactly one widget"));
        }
        idents.push(v.ident.clone());
        tys.push(f.unnamed[0].ty.clone());
    }
    let names: Vec<String> = idents.iter().map(|i| i.to_string()).collect();
    Ok(quote! { ::lumen_static::__reloadable! {
        impl ::lumen_static::reload::Reloadable for #name {
            fn save(&self) -> ::lumen_static::reload::Value {
                match self {
                    #(Self::#idents(w) => ::lumen_static::reload::json!({
                        "variant": #names,
                        "state": ::lumen_static::reload::Reloadable::save(w),
                    }),)*
                }
            }
            fn restore(&mut self, saved: &::lumen_static::reload::Value) {
                let Some(variant) = saved.get("variant").and_then(::lumen_static::reload::Value::as_str) else {
                    ::lumen_static::reload::log_fallback(#name_str, "<variant>", "saved state names no variant");
                    return;
                };
                let state = saved.get("state").unwrap_or(&::lumen_static::reload::Value::Null);
                match variant {
                    #(#names => {
                        if !matches!(self, Self::#idents(_)) {
                            *self = Self::#idents(<#tys as ::core::default::Default>::default());
                        }
                        if let Self::#idents(w) = self {
                            ::lumen_static::reload::Reloadable::restore(w, state);
                        }
                    })*
                    other => ::lumen_static::reload::log_fallback(
                        #name_str,
                        "<variant>",
                        &format!("variant `{other}` is not in this build"),
                    ),
                }
            }
        }
    }})
}

/// `#[derive(StaticTree)]` on an enum — plan T4.4, §6's variant hatch.
///
/// This is the case the old `base + offset` path could not express:
/// variants overlap in memory, so no const byte offset identifies a node
/// inside one. The accessor path matches the discriminant instead, which
/// is why adopting it unblocked this task
/// (`docs/layout-and-storage-decisions.md` §4d).
///
/// The enum contributes one container node **plus every variant's
/// subtable**, each variant at its own index range: `NODE_COUNT` is
/// `1 + Σ variants`, so a node's index means the same thing whichever
/// variant is showing (F4.3; `TABLE` below). Inactive variants are
/// on-table and *hidden* — laid out as zero-sized, the way an empty
/// `Option` is — not kept off-table. `Either` has the same shape since
/// its own rework (`1 + A + B`, `lumen_static::hatch`). *Corrected in
/// M46/T46.3 (R2-F8): this paragraph said "one table node", "off-table
/// with its own state arena" and "independent of how many variants",
/// the pre-F4.3 shape, and "like `Either`", which was stale for the same
/// reason; `tests/table_invariants.rs` pins the count.* Each variant
/// must be a single unnamed field holding a widget.
fn expand_enum(
    name: &syn::Ident,
    name_str: &str,
    data: &syn::DataEnum,
    root_line: u32,
) -> syn::Result<Ts2> {
    let mut var_ident = Vec::new();
    let mut var_ty = Vec::new();
    for v in &data.variants {
        let syn::Fields::Unnamed(f) = &v.fields else {
            return Err(syn::Error::new_spanned(
                v,
                "each variant must hold exactly one widget: `Variant(Widget)`",
            ));
        };
        if f.unnamed.len() != 1 {
            return Err(syn::Error::new_spanned(v, "each variant must hold exactly one widget"));
        }
        var_ident.push(v.ident.clone());
        var_ty.push(f.unnamed[0].ty.clone());
    }
    if var_ident.is_empty() {
        return Err(syn::Error::new_spanned(name, "an enum widget needs at least one variant"));
    }
    let nvars = var_ident.len();
    let var_no: Vec<syn::Index> = (0..nvars).map(syn::Index::from).collect();

    Ok(quote! {
        const _: () = {
            // No `use` block: it would shadow a user type of the same
            // name at the point the impl is written, so a widget struct
            // named `Size`, `Node` or `Point` failed with an error
            // naming a framework type it never mentioned. Everything is
            // spelled out instead (F6.2).

            /// Where each variant's subtree starts, plus a terminator.
            const STARTS: [usize; #nvars + 1] = {
                let counts = [#(<#var_ty as ::lumen_static::widget::Widget>::NODE_COUNT),*];
                let mut s = [0usize; #nvars + 1];
                s[0] = 1;
                let mut k = 0;
                while k < #nvars {
                    s[k + 1] = s[k] + counts[k];
                    k += 1;
                }
                s
            };

            /// Every variant gets its own index range — `1 + sum`, not
            /// `1`. That is handoff §6's shape, and what it buys is that
            /// a node's index means the same thing whichever variant is
            /// showing, so agent ids and provenance survive a switch.
            ///
            /// The previous shape was `NODE_COUNT = 1` with the active
            /// variant laid out into a scratch `Vec` allocated per call.
            /// It could not paint (no retained geometry), could not be
            /// hit, could not carry handlers, and allocated twice per
            /// enum node per frame in framework-generated code
            /// (`docs/plan-review-fixes.md` F4.3).
            const TABLE: [::lumen_static::desc::NodeDesc; 1 #(+ <#var_ty as ::lumen_static::widget::Widget>::NODE_COUNT)*] = {
                let mut t = [::lumen_static::desc::NodeDesc::EMPTY; 1 #(+ <#var_ty as ::lumen_static::widget::Widget>::NODE_COUNT)*];
                t[0] = ::lumen_static::desc::NodeDesc::new(
                    1,
                    ::lumen_static::NO_PARENT,
                    0,
                    #nvars as u16,
                    ::lumen_static::desc::NodeDesc::CONTAINER,
                )
                .with_src(::core::file!(), #root_line);
                let mut at: usize = 1;
                let mut prev_root: usize = 0;
                #(
                    ::lumen_static::widget::splice_child(
                        &mut t, at, <#var_ty as ::lumen_static::widget::Widget>::SUB_TABLE, 0,
                    );
                    if prev_root != 0 { t[prev_root].next_sibling = at as u32; }
                    prev_root = at;
                    at += <#var_ty as ::lumen_static::widget::Widget>::NODE_COUNT;
                )*
                let _ = (at, prev_root);
                t
            };

            impl ::lumen_static::widget::Widget for #name {
                const NODE_COUNT: usize = 1 #(+ <#var_ty as ::lumen_static::widget::Widget>::NODE_COUNT)*;
                const DEPTH: usize = 1 + {
                    let mut d = 0usize;
                    #( if <#var_ty as ::lumen_static::widget::Widget>::DEPTH > d { d = <#var_ty as ::lumen_static::widget::Widget>::DEPTH; } )*
                    d
                };
                const IS_RIGID: bool = true #(&& <#var_ty as ::lumen_static::widget::Widget>::IS_RIGID)*;
                const SUB_TABLE: &'static [::lumen_static::desc::NodeDesc] = &TABLE;

                const KIND: &'static str = #name_str;

                // **The static bind walk** — M52/T52.5: the active
                // variant's own walk, offset to where its indices start.
                // An inactive variant's indices are never answered, which
                // is what the vtable walk's degenerate-index skip does.
                fn bind_walk(&self, target: usize) -> ::core::option::Option<usize> {
                    let lo = self as *const Self as usize;
                    if target < lo || target >= lo + ::core::mem::size_of::<Self>() {
                        return ::core::option::Option::None;
                    }
                    match self {
                        #( #name::#var_ident(w) => {
                            if let ::core::option::Option::Some(j) =
                                <#var_ty as ::lumen_static::widget::Widget>::bind_walk(w, target)
                            {
                                return ::core::option::Option::Some(STARTS[#var_no] + j);
                            }
                        } ),*
                    }
                    ::core::option::Option::Some(0)
                }

                /// **A container's defaults, like every other
                /// container's.** The enum switch emitted no `defaults`
                /// at all and so inherited `Node`'s `Style::default()`
                /// — `Shrink` on both axes, where `row`, `column` and
                /// `stack` all resolve `FILL` across and `Shrink` along.
                ///
                /// Nothing caught it because nothing had put an enum
                /// where the difference shows. `examples/showcase` did,
                /// in M24/T24.3: its stage is centred by a `FILL` spacer
                /// either side, and a `Shrink` middle child is measured
                /// in the row's *first* pass against all the space there
                /// is — so it took the whole 960 px and the spacers got
                /// nothing. The demo moved to the left edge.
                fn type_defaults() -> ::lumen_static::Style {
                    ::lumen_static::Style {
                        width: ::lumen_static::Length::FILL,
                        height: ::lumen_static::Length::Shrink,
                        ..::lumen_static::Style::default()
                    }
                }

                const SEEDS: &'static [::lumen_static::style::Seed] = &SEEDS;
            }

            /// One seed per node, **including every inactive variant's**.
            ///
            /// This is what the switch was missing. An inactive variant
            /// has no instance, so `node(i)` on those indices returns the
            /// enum itself — deliberately, so the agent can walk them —
            /// and the cascade therefore resolved a hidden `CheckBox`
            /// from the *enum's* kind and defaults: `Fixed(24.0)` became
            /// `Shrink`, which on screen is a control with no size of its
            /// own. A seed is a property of the variant's type, which is
            /// known here whether or not it is the one showing.
            /// `docs/plan-select.md` §1.
            const SEEDS: [::lumen_static::style::Seed;
                1 #(+ <#var_ty as ::lumen_static::widget::Widget>::NODE_COUNT)*] = {
                let mut s = [::lumen_static::style::Seed::WIDGET;
                    1 #(+ <#var_ty as ::lumen_static::widget::Widget>::NODE_COUNT)*];
                s[0] = ::lumen_static::style::Seed::new(
                    <#name as ::lumen_static::widget::Widget>::KIND,
                    <#name as ::lumen_static::widget::Widget>::type_defaults, ::lumen_static::style::Fields::of(&[::lumen_static::style::Fields::WIDTH, ::lumen_static::style::Fields::HEIGHT, ::lumen_static::style::Fields::CURSOR, ::lumen_static::style::Fields::VISIBLE]));
                let mut at: usize = 1;
                #(
                    ::lumen_static::widget::splice_seeds(
                        &mut s, at, <#var_ty as ::lumen_static::widget::Widget>::SEEDS,
                    );
                    at += <#var_ty as ::lumen_static::widget::Widget>::NODE_COUNT;
                )*
                let _ = at;
                s
            };

            const _: () = assert!(::lumen_static::widget::table_is_well_formed(&TABLE));
            const _: () = assert!(<#name as ::lumen_static::widget::Widget>::DEPTH <= ::lumen_static::MAX_DEPTH);
            // **The lengths, asserted** (M26/T26.16).
            //
            // "The derive half is unfalsifiable" is true of exactly one
            // of four cases: only the *struct* derive's table length
            // literally is the node count. The struct derive's **seed**
            // length and both of the enum derive's lengths are
            // re-spelled sums and can drift from it — a splice written
            // with one offset wrong produces a table of the right
            // length and a seeds array of the wrong one, and nothing
            // said so.
            const _: () = assert!(
                TABLE.len() == <#name as ::lumen_static::widget::Widget>::NODE_COUNT,
                "the spliced table's length is not this tree's NODE_COUNT",
            );
            const _: () = assert!(
                SEEDS.len() == <#name as ::lumen_static::widget::Widget>::NODE_COUNT,
                "the spliced seeds array's length is not this tree's NODE_COUNT",
            );

            impl ::lumen_static::widget::Node for #name {
                fn kind_name(&self) -> &'static str {
                    <Self as ::lumen_static::widget::Widget>::KIND
                }

                // M48/T48.7: the agent's structural stand-in test.
                ::lumen_static::node_count!();

                fn defaults(&self) -> ::lumen_static::Style {
                    <Self as ::lumen_static::widget::Widget>::type_defaults()
                }

                fn layout(
                    &mut self,
                    cx: &mut ::lumen_static::LayoutCx,
                    limits: ::lumen_static::Constraints,
                    style: &[::lumen_static::Style],
                    st: &mut [::lumen_static::desc::NodeState],
                    i: usize,
                ) -> ::lumen_static::Size {
                    let me = style[i];
                    // Inactive variants are hidden, not stale: their
                    // slots would otherwise keep the geometry from
                    // whenever they were last active, which paint, the
                    // journal and the audit all read.
                    //
                    // **The ranges either side of the active variant, not
                    // the whole span** — M26/T26.1. `hide_subtree` stamps
                    // `constraints = limits` as well as zeroing the size,
                    // and a derived container's `layout` begins with
                    // `can_skip() && is_clean(..) && st[i].constraints ==
                    // limits -> return st[i].size`. Hiding the active
                    // variant first therefore handed it a zero that its
                    // own relayout boundary then returned, on any frame
                    // where the enum was reached but its subtree was
                    // clean: the stage vanished under `Boundaries` and
                    // was correct under `Everything`, which is why no
                    // test had ever seen it
                    // (`crates/lumen-static-shell/tests/enum_relayout.rs`).
                    // The active variant's range, then two hides around
                    // it — two call sites, not two per arm.
                    let (astart, aend) = match self {
                        #( #name::#var_ident(_) => (STARTS[#var_no], STARTS[#var_no + 1]), )*
                    };
                    ::lumen_static::ops::hide_subtree(st, i + 1, limits, astart - 1);
                    ::lumen_static::ops::hide_subtree(
                        st,
                        i + aend,
                        limits,
                        <Self as ::lumen_static::widget::Widget>::NODE_COUNT - aend,
                    );
                    let size = match self {
                        #( #name::#var_ident(w) => {
                            w.layout(cx, limits, style, st, i + STARTS[#var_no])
                        } ),*
                    };
                    ::lumen_static::ops::record(
                        st,
                        i,
                        limits,
                        ::lumen_static::ops::resolve_self(limits, &me, size),
                    )
                }

                fn place(
                    &mut self,
                    at: ::lumen_static::Point,
                    style: &[::lumen_static::Style],
                    st: &mut [::lumen_static::desc::NodeState],
                    i: usize,
                ) {
                    st[i].pos = at;
                    match self {
                        #( #name::#var_ident(w) => {
                            w.place(at, style, st, i + STARTS[#var_no])
                        } ),*
                    }
                }

                fn paint(
                    &self,
                    st: &[::lumen_static::desc::NodeState],
                    i: usize,
                    out: &mut dyn ::lumen_static::Painter,
                ) {
                    match self {
                        #( #name::#var_ident(w) => {
                            w.paint(st, i + STARTS[#var_no], out)
                        } ),*
                    }
                }

                fn hit(&self, st: &[::lumen_static::desc::NodeState], i: usize, x: f32, y: f32)
                    -> ::core::option::Option<::lumen_static::Hit>
                {
                    if !::lumen_static::hit::contains(&st[i], x, y) {
                        return ::core::option::Option::None;
                    }
                    let inner = match self {
                        #( #name::#var_ident(w) => {
                            w.hit(st, i + STARTS[#var_no], x, y)
                        } ),*
                    };
                    inner.or(::core::option::Option::Some(::lumen_static::Hit {
                        node: i as u32,
                        inner: ::core::option::Option::None,
                        local: ::lumen_static::Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
                    }))
                }

                /// The match the byte-offset path could not express: the
                /// discriminant selects which subtree index `i` names.
                ///
                /// Indices belonging to an *inactive* variant resolve to
                /// the enum node itself rather than panicking — they are
                /// real table entries with real state, they are simply
                /// hidden, and the agent walks them.
                fn node(&self, i: usize) -> &dyn ::lumen_static::widget::Node {
                    if i == 0 { return self; }
                    match self {
                        #( #name::#var_ident(w) => {
                            let (lo, hi) = (STARTS[#var_no], STARTS[#var_no + 1]);
                            if i >= lo && i < hi {
                                return ::lumen_static::widget::Node::node(w, i - lo);
                            }
                        } ),*
                    }
                    self
                }

                fn node_mut(&mut self, i: usize) -> &mut dyn ::lumen_static::widget::Node {
                    if i == 0 { return self; }
                    let active = match self {
                        #( #name::#var_ident(_) => (STARTS[#var_no], STARTS[#var_no + 1]) ),*
                    };
                    if i < active.0 || i >= active.1 {
                        return self;
                    }
                    let off = active.0;
                    match self {
                        #( #name::#var_ident(w) => ::lumen_static::widget::Node::node_mut(w, i - off) ),*
                    }
                }
            }

            /// Delegates to the active variant. The where-clause makes a
            /// mis-typed handler a compile error at the enum, as it does
            /// for a struct container.
            impl<__LnS> ::lumen_static::event::Handlers<__LnS> for #name
            where
                #(#var_ty: ::lumen_static::event::Handlers<__LnS>,)*
            {
                fn handler(
                    &self,
                    sel: ::lumen_static::event::NodeSel,
                ) -> ::core::option::Option<fn(&mut __LnS, &::lumen_static::EventCtx)> {
                    if sel.i == 0 { return ::core::option::Option::None; }
                    match self {
                        #( #name::#var_ident(w) => {
                            let (lo, hi) = (STARTS[#var_no], STARTS[#var_no + 1]);
                            if sel.i >= lo && sel.i < hi {
                                return ::lumen_static::event::Handlers::handler(
                                    w,
                                    ::lumen_static::event::NodeSel { i: sel.i - lo, inner: sel.inner },
                                );
                            }
                        } ),*
                    }
                    ::core::option::Option::None
                }
            }
        };
    })
}

// ---------------------------------------------------------------------
// `#[derive(App)]` — the application root: tree accessors plus the
// update step, generated from `#[bind(...)]` declarations (plan T6.3).
// ---------------------------------------------------------------------

/// What one `#[bind(...)]` attribute says.
struct BindSpec {
    /// Path to the target widget, relative to `self` (`ui.value`).
    target: syn::Expr,
    kind: BindKind,
}

enum BindKind {
    /// `#[bind(t, "fmt {}", extra…)]` — the annotated field is the first
    /// format argument; further arguments are written out in full, and
    /// are evaluated in the user's scope only: the expansion's own locals
    /// are `Span::mixed_site()` and cannot be named from them (M55/T55.1).
    ///
    /// The optional [`syn::Expr`] is a path **within** the annotated
    /// field, so `#[bind(ui.value, count, "count: {}")]` on a field
    /// `state: Tally` formats `self.state.count` — M32. Relative to the
    /// field for the same reason `target` is relative to `self`: the
    /// thing you name is the thing the attribute is attached to, one
    /// level in. Without it the field itself is the source, which is
    /// every binding written before this existed.
    /// Boxed: a bare `syn::Expr` is ~216 B and made this variant four
    /// times the size of `With`, which `clippy::large_enum_variant`
    /// refuses.
    Fmt(Option<Box<syn::Expr>>, syn::LitStr, Vec<syn::Expr>),
    /// `#[bind(t, with = f)]` — `f: fn(&mut Target, &Source)`.
    With(syn::Path),
    /// `#[bind(t, source)]` — copy a value, M33/T33.1. `source` is a path
    /// within the annotated field, as the format form's is; the target
    /// implements `lumen_static::bind::BindValue` for its type, which
    /// compares before it writes, so the binding reports precisely — the
    /// property `with =` gives up, and the reason this form exists (a
    /// `Select`'s `open`, a glass material's blur).
    Value(Box<syn::Expr>),
}

impl syn::parse::Parse for BindSpec {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let target: syn::Expr = input.parse()?;
        input.parse::<syn::Token![,]>().map_err(|_| {
            syn::Error::new_spanned(
                &target,
                "expected `#[bind(target, \"format {}\")]`, `#[bind(target, source)]` or \
                 `#[bind(target, with = func)]`",
            )
        })?;

        if input.peek(syn::Ident) && input.peek2(syn::Token![=]) {
            let kw: syn::Ident = input.parse()?;
            if kw != "with" {
                return Err(syn::Error::new_spanned(&kw, "expected `with`"));
            }
            input.parse::<syn::Token![=]>()?;
            return Ok(BindSpec { target, kind: BindKind::With(input.parse()?) });
        }

        // **A source path, if one is given** — M32. Unambiguous without
        // lookahead games: the format string is a literal, so anything
        // else in this position is a path into the annotated field.
        let source: Option<Box<syn::Expr>> = if input.peek(syn::LitStr) {
            None
        } else {
            let src: syn::Expr = input.parse()?;
            // No format string after the source: the value form.
            if input.is_empty() {
                return Ok(BindSpec { target, kind: BindKind::Value(Box::new(src)) });
            }
            input.parse::<syn::Token![,]>().map_err(|_| {
                syn::Error::new_spanned(
                    &src,
                    "expected `#[bind(target, source, \"format {}\")]` — a source path \
                     must be followed by a format string",
                )
            })?;
            Some(Box::new(src))
        };

        let fmt: syn::LitStr = input.parse()?;
        let mut extra = Vec::new();
        while input.peek(syn::Token![,]) {
            input.parse::<syn::Token![,]>()?;
            if input.is_empty() {
                break;
            }
            extra.push(input.parse()?);
        }
        Ok(BindSpec { target, kind: BindKind::Fmt(source, fmt, extra) })
    }
}

/// The application root. Emits `impl App`: the tree accessors, and an
/// `update` built from the `#[bind]` declarations on the state fields.
///
/// ```
/// use lumen_static_widgets::ops::Label;
/// use lumen_static::{App, StaticTree};
///
/// #[derive(StaticTree, Default)]
/// #[widget(column, spacing = 0.0)]
/// struct Ui {
///     #[widget_id("value")]
///     value: Label<String>,
/// }
///
/// // `-> bool`: whether it changed anything the frame can see. A hook
/// // that cannot report leaves the derive no choice but to mark every
/// // pump changed, and a frame that never settles damages the whole
/// // window on every interaction (M27/T27.3).
/// fn recompute(c: &mut Counter) -> bool {
///     c.recomputed += 1;
///     false
/// }
///
/// #[derive(App, Default)]
/// #[app(after_update = recompute)]     // optional, fn(&mut Self) -> bool
/// // #[app(tick = advance)]             // optional, fn(&mut Self, f32) -> bool
/// // #[app(wake = next_frame)]          // optional, fn(&Self) -> Option<f32>
/// struct Counter {
///     #[tree]
///     ui: Ui,
///     #[bind(ui.value, "count: {}")]
///     count: i64,
///     recomputed: u32,
/// }
///
/// # fn main() {
/// use lumen_static::event::App as _;
/// let mut c = Counter { count: 7, ..Default::default() };
/// c.update();                                    // what the frame loop calls
/// assert_eq!(c.ui.value.as_str(), "count: 7");
/// assert_eq!(c.recomputed, 1);
/// # }
/// ```
///
/// **A format binding is capped at 256 bytes without `std`.** With `std`
/// it formats into a reused `String` and stores the whole value; in a
/// `no_std` build it formats into a 256-byte stack buffer and stores the
/// prefix up to the last `char` boundary that fits, the rule
/// `InlineText::set_from_str` follows. `fixtures/embedded/tests/bind_nostd.rs`
/// runs that branch (M43/T43.8).
///
/// The whole impl is generated so that nothing about the update step is
/// hand-written: a derive that only emitted the binding body would still
/// leave the author to call it from `update`, which is the wiring this
/// is meant to remove.
#[proc_macro_derive(App, attributes(tree, state, bind, app))]
pub fn derive_app(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_app(input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand_app(input: DeriveInput) -> syn::Result<Ts2> {
    let name = &input.ident;
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(name, "App derives a struct with named fields"));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(name, "App needs named fields"));
    };

    // Struct-level `#[app(after_update = path)]`, `#[app(tick = path)]`
    // and `#[app(wake = path)]` (M84/T84.3).
    let mut after_update: Option<syn::Path> = None;
    let mut tick: Option<syn::Path> = None;
    let mut wake: Option<syn::Path> = None;
    for attr in &input.attrs {
        if !attr.path().is_ident("app") {
            continue;
        }
        // Each key once (M55/T55.2, R16-F4): `tick = t1, tick = t2` used
        // to run `t2`, silently.
        attr.parse_nested_meta(|meta| {
            let slot = if meta.path.is_ident("after_update") {
                &mut after_update
            } else if meta.path.is_ident("tick") {
                &mut tick
            } else if meta.path.is_ident("wake") {
                &mut wake
            } else {
                return Err(meta.error("expected `after_update = path`, `tick = path` or `wake = path`"));
            };
            if slot.is_some() {
                let key = if meta.path.is_ident("tick") {
                    "tick"
                } else if meta.path.is_ident("wake") {
                    "wake"
                } else {
                    "after_update"
                };
                return Err(meta.error(format!("`#[app({key} = ..)]` given twice")));
            }
            *slot = Some(meta.value()?.parse()?);
            Ok(())
        })?;
    }

    let mut tree_field: Option<&syn::Field> = None;
    let mut bind_specs: Vec<(BindSpec, syn::Ident)> = Vec::new();
    for f in &fields.named {
        let ident = f.ident.clone().unwrap();
        for attr in &f.attrs {
            if attr.path().is_ident("tree") {
                // A marker: arguments were accepted and ignored until
                // M55/T55.2 (R16-F4), so `#[tree(junk = 5)]` compiled.
                attr.meta.require_path_only().map_err(|_| {
                    syn::Error::new_spanned(attr, "`#[tree]` takes no arguments")
                })?;
                if let Some(prev) = tree_field {
                    let mut e = syn::Error::new_spanned(f, "a second `#[tree]` field");
                    e.combine(syn::Error::new_spanned(prev, "the first is here"));
                    return Err(e);
                }
                tree_field = Some(f);
            } else if attr.path().is_ident("bind") {
                let spec: BindSpec = attr.parse_args()?;
                bind_specs.push((spec, ident.clone()));
            }
        }
    }

    let Some(tree) = tree_field else {
        return Err(syn::Error::new_spanned(
            name,
            "App needs a `#[tree]` field naming the widget tree, e.g. `#[tree] ui: Ui`",
        ));
    };
    let tree_ident = tree.ident.clone().unwrap();
    let tree_ty = &tree.ty;
    let generic = !input.generics.params.is_empty();
    // **The expansion's own locals are hygienic** — M55/T55.1 (R16-F3).
    //
    // They sit beside the user's `#[bind]` tokens — a format argument, a
    // `with =` path, a source path — and were declared at call-site
    // span, so a user item named `changed` resolved to the derive's
    // `bool`: `with = changed` became a fn-pointer mismatch at the
    // user's token, and an extra argument `changed()` became "expected
    // function, found `bool`". At `Span::mixed_site()` a local binding is
    // visible only to tokens carrying the same span, so the user's tokens
    // — which keep their own spans, so errors still point at them — can
    // no longer name one. The `__ln_` prefix stays as the second line of
    // defence, and is the *only* one for `__ln_site` in the non-generic
    // arm: that is a `static` item, and `mixed_site` does not hide item
    // names. A user item called `__ln_site` in the same module collides;
    // that is accepted (`docs/plan-derive-dx.md` §4).
    let hyg = |name: &str| syn::Ident::new(name, proc_macro2::Span::mixed_site());
    let changed = hyg("__ln_changed");
    let all = hyg("__ln_all");
    let mark = hyg("__ln_mark");
    let site_id = hyg("__ln_site");
    let ptr = hyg("__ln_p");
    let idx = hyg("__ln_i");
    let hook = hyg("__ln_f");
    let binds: Vec<Ts2> = bind_specs
        .iter()
        .map(|(spec, ident)| {
            let target = &spec.target;
            // **M29/T29.3: a binding that wrote marks the node it wrote**,
            // found by address (`bind::node_of`, T29.2) and cached per
            // binding site. A generic application gets no static — one
            // would be shared by every instantiation — and walks per
            // write. An address outside the table (an arena row) marks
            // everything, the conservative answer.
            let site = if generic {
                quote! { let #site_id = ::lumen_static::bind::BindSite::new(); }
            } else {
                quote! { static #site_id: ::lumen_static::bind::BindSite = ::lumen_static::bind::BindSite::new(); }
            };
            let locate = |write: Ts2| {
                quote! {
                    if #write {
                        #changed = true;
                        #site
                        let #ptr = &self.#target as *const _ as *const u8;
                        match #site_id.get(|| ::lumen_static::bind::node_of(&self.#tree_ident, #ptr)) {
                            ::core::option::Option::Some(#idx) => #mark(
                                ::lumen_static::style::subtree_range(
                                    <#tree_ty as ::lumen_static::widget::Widget>::SUB_TABLE,
                                    #idx,
                                ),
                            ),
                            ::core::option::Option::None => #all = true,
                        }
                    }
                }
            };
            let _ = ident;
            match &spec.kind {
                        // `format_into` compares before it writes and says
                        // so, so a binding reports precisely (T16.10).
                        BindKind::Fmt(source, fmt, extra) => {
                            // `self.field` or `self.field.path.into.it`.
                            let src = match source {
                                Some(p) => quote! { self.#ident.#p },
                                None => quote! { self.#ident },
                            };
                            let write = quote! {
                                ::lumen_static::bind::format_into(
                                    &mut self.#target,
                                    ::core::format_args!(#fmt, &#src #(, #extra)*),
                                )
                            };
                            locate(write)
                        }
                        BindKind::Value(p) => locate(quote! {
                            ::lumen_static::bind::BindValue::bind_value(
                                &mut self.#target,
                                &self.#ident.#p,
                            )
                        }),
                        BindKind::With(path) => quote! {
                            // Spelled out so a mismatched `with =` names the
                            // expected signature instead of failing inside
                            // the generated call.
                            //
                            // **Reports `true` unconditionally**: the hook
                            // returns nothing, so whether it wrote anything
                            // is unknowable from here. A `with =` binding
                            // therefore opts its application out of the
                            // still-frame optimisation, which is the
                            // conservative direction — the alternative is a
                            // frame that skipped a change.
                            {
                                let #hook: fn(&mut _, &_) = #path;
                                #hook(&mut self.#target, &self.#ident);
                                #changed = true;
                                #all = true;
                            }
                        },
                    }
        })
        .collect();

    let after = after_update.map(|p| {
        quote! {
            // Typed through a binding so a wrong signature is reported
            // at the hook, not at the call.
            //
            // **`-> bool`, mirroring `tick`, since M27/T27.3.** It was
            // `fn(&mut Self)` and the derive then had to set
            // `changed = true` unconditionally, because a hook that may
            // write anything and reports nothing leaves the frame not
            // still (T16.10). The cost was invisible until a drive
            // script measured per-widget damage and got **170x the
            // widget's own box** — the whole window, every pump, for a
            // click on a button. A hook that knows whether it changed
            // anything is the only thing that can say so.
            let #hook: fn(&mut Self) -> bool = #p;
            // A hook may write anything, so a hook that wrote marks
            // everything (M29 §2.4): it reports a `bool`, not ranges.
            if #hook(self) {
                #changed = true;
                #all = true;
            }
        }
    });

    // **M22/T22.4.** Without this a derived app could not advance an
    // animation at all: the derive writes the whole `App` impl, so
    // `tick` could only come from the trait default, and a widget that
    // animates — `Glass`'s press deformation is the first — would
    // retarget and then never move. The hook mirrors `after_update`.
    let dt = hyg("__ln_dt_ms");
    let tick_fn = tick.map(|p| {
        quote! {
            fn tick(&mut self, #dt: f32) -> bool {
                // Typed through a binding so a wrong signature is
                // reported at the hook, not at the call.
                let #hook: fn(&mut Self, f32) -> bool = #p;
                #hook(self, #dt)
            }
        }
    });

    // **M84/T84.3.** `App::wake_in_ms` for a derived application: when
    // it next needs a frame. An animated image's tick answers `true` only
    // on a change, so without this its window would idle after the first
    // frame; with it, the window sleeps until the next change is due.
    let wake_fn = wake.map(|p| {
        quote! {
            fn wake_in_ms(&self) -> ::core::option::Option<f32> {
                // Typed through a binding so a wrong signature is
                // reported at the hook, not at the call.
                let #hook: fn(&Self) -> ::core::option::Option<f32> = #p;
                #hook(self)
            }
        }
    });

    // **`#[state]` names what a handler may touch** — M33
    // (`docs/plan-state-split.md`).
    //
    // Absent, `State = Self` and a handler gets the whole application,
    // which is what every app did before this existed. Present, the tree
    // is out of a handler's reach *by type*, and `App::STATE_IS_SPLIT`
    // says so — which is what lets `Host::pump_with` narrow the mark for
    // a consumed event (M29/T29.6). So the two ways that constant could
    // lie are refused here rather than trusted: `#[state]` on the tree
    // itself, and a second `#[state]` field (the prototype silently took
    // the first).
    let mut state_field: Option<&syn::Field> = None;
    for f in fields.named.iter() {
        let mut states = f.attrs.iter().filter(|a| a.path().is_ident("state"));
        let Some(first) = states.next() else {
            continue;
        };
        // A marker, like `#[tree]` (M55/T55.2): `#[state(junk)]` was
        // detected by its path alone and compiled.
        for a in f.attrs.iter().filter(|a| a.path().is_ident("state")) {
            a.meta.require_path_only().map_err(|_| {
                syn::Error::new_spanned(a, "`#[state]` takes no arguments")
            })?;
        }
        if let Some(again) = states.next() {
            let mut e = syn::Error::new_spanned(again, "a second `#[state]` field");
            e.combine(syn::Error::new_spanned(first, "the first is here"));
            return Err(e);
        }
        if f.attrs.iter().any(|a| a.path().is_ident("tree")) {
            return Err(syn::Error::new_spanned(
                f,
                "`#[state]` on the `#[tree]` field would hand the tree to every handler — \
                 the state must be a sibling of the tree, not the tree itself",
            ));
        }
        if let Some(prev) = state_field {
            let mut e = syn::Error::new_spanned(f, "a second `#[state]` field");
            e.combine(syn::Error::new_spanned(prev, "the first is here"));
            return Err(e);
        }
        state_field = Some(f);
    }
    let split = state_field.is_some();
    let (state_ty, state_expr) = match state_field {
        Some(f) => {
            let ident = f.ident.as_ref().unwrap();
            let ty = &f.ty;
            (quote! { #ty }, quote! { &mut self.#ident })
        }
        None => (quote! { Self }, quote! { self }),
    };

    let (impl_g, ty_g, where_g) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_g ::lumen_static::event::App for #name #ty_g #where_g {
            type Tree = #tree_ty;

            type State = #state_ty;

            #[inline]
            fn state_mut(&mut self) -> &mut Self::State {
                #state_expr
            }

            const STATE_IS_SPLIT: bool = #split;

            /// M28/T28.2: a derived app defers nothing until it says so,
            /// and `Never` is uninhabited — so nothing can be sent and
            /// the field costs the application nothing.
            type Msg = ::lumen_static::event::Never;

            /// Unreachable by construction: `Never` has no values.
            fn apply(_: &mut Self::State, m: Self::Msg) -> bool {
                match m {}
            }

            #[inline]
            fn tree(&self) -> &Self::Tree {
                &self.#tree_ident
            }

            #[inline]
            fn tree_mut(&mut self) -> &mut Self::Tree {
                &mut self.#tree_ident
            }

            fn update_marked(
                &mut self,
                #mark: &mut dyn FnMut(::core::ops::Range<usize>),
            ) -> ::lumen_static::event::Wrote {
                #[allow(unused_mut)]
                let mut #changed = false;
                #[allow(unused_mut)]
                let mut #all = false;
                #(#binds)*
                #after
                ::lumen_static::event::Wrote::of(#changed, #all)
            }

            fn update(&mut self) -> bool {
                !::core::matches!(self.update_marked(&mut |_| {}), ::lumen_static::event::Wrote::Nothing)
            }

            #tick_fn

            #wake_fn
        }
    })
}
