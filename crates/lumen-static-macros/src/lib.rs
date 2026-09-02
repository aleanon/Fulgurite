//! The three derives: `StaticTree` (a container's shape),
//! `Reloadable` (what survives a hot swap), and `App` (the application
//! root and its bindings).
//!
//! ```
//! use lumen_static::ops::{Fixed, Label, Spacer};
//! use lumen_static::StaticTree;
//!
//! #[derive(StaticTree, Default)]
//! #[widget(row, spacing = 4.0)]
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
//! every expansion: the spliced table is well-formed, and `DEPTH` fits
//! `MAX_DEPTH`, so a tree that could overflow the stack does not compile.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Ts2;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

enum Axis {
    Row,
    Column,
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
fn parse_wid(f: &syn::Field) -> syn::Result<String> {
    for attr in &f.attrs {
        if attr.path().is_ident("widget_id") {
            let lit: syn::LitStr = attr.parse_args().map_err(|_| {
                syn::Error::new_spanned(
                    attr,
                    "expected a string literal: `#[widget_id(\"save\")]`",
                )
            })?;
            return Ok(lit.value());
        }
    }
    Ok(String::new())
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
        let line = input.ident.span().start().line as u32;
        return expand_enum(name, &name_str, e, line);
    }

    // ---- #[widget(row|column, spacing = <f32>, fuse)] ----
    let mut axis = None;
    let mut spacing = 0.0f32;
    let mut fuse = false;
    for attr in &input.attrs {
        if !attr.path().is_ident("widget") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("row") {
                axis = Some(Axis::Row);
            } else if meta.path.is_ident("column") {
                axis = Some(Axis::Column);
            } else if meta.path.is_ident("spacing") {
                let lit: syn::LitFloat = meta.value()?.parse()?;
                spacing = lit.base10_parse()?;
            } else if meta.path.is_ident("fuse") {
                fuse = true;
            } else {
                return Err(meta.error("expected `row`, `column`, `spacing = <float>`, or `fuse`"));
            }
            Ok(())
        })?;
    }
    let Some(axis) = axis else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "StaticTree needs #[widget(row)] or #[widget(column)]",
        ));
    };
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
    let field_classes: Vec<Ts2> = fields
        .named
        .iter()
        .map(parse_classes)
        .collect::<syn::Result<Vec<_>>>()?;
    let field_wid: Vec<String> =
        fields.named.iter().map(parse_wid).collect::<syn::Result<Vec<_>>>()?;

    let axis_expr = match axis {
        Axis::Row => quote! { ::lumen_static::ops::Axis::Row },
        Axis::Column => quote! { ::lumen_static::ops::Axis::Column },
    };
    // A container fills across its cross axis and shrinks along its main
    // one — the shape that makes a column of rows behave as expected.
    let (default_w, default_h) = match axis {
        Axis::Row => (
            quote! { ::lumen_static::Length::FILL },
            quote! { ::lumen_static::Length::Shrink },
        ),
        Axis::Column => (
            quote! { ::lumen_static::Length::FILL },
            quote! { ::lumen_static::Length::Shrink },
        ),
    };
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
    let opaque_stmt = if cfg!(feature = "rdyn_opaque") && !fuse {
        quote! { let d = ::core::hint::black_box(d); }
    } else {
        quote! {}
    };

    Ok(quote! {
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
                .with_src(::core::file!(), #root_line);
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
            }

            // Proof-carrying table: the compiler re-checks the macro's
            // own output at every expansion, and the depth cap turns the
            // stack-overflow hazard into a compile error.
            const _: () = assert!(::lumen_static::widget::table_is_well_formed(&TABLE));
            const _: () = assert!(<#name as ::lumen_static::widget::Widget>::DEPTH <= ::lumen_static::MAX_DEPTH);

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
                    #name_str
                }

                fn defaults(&self) -> ::lumen_static::Style {
                    ::lumen_static::Style {
                        width: #default_w,
                        height: #default_h,
                        spacing: #sp,
                        ..::lumen_static::Style::default()
                    }
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
                    use ::lumen_static::ops::{
                        align_offset, fill_child, inner_limits, loose_child, resolve_self, spaced,
                        Axis,
                    };
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
                    let axis = #axis_expr;
                    let inner = inner_limits(limits, me.padding);
                    let gaps = spaced(#n, me.spacing);

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
                    let free = (axis.main_of(inner) - used).max(0.0);
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
                    ::lumen_static::ops::record(st, i, limits, resolve_self(limits, &me, padded))
                }

                fn place(
                    &mut self,
                    at_pos: ::lumen_static::Point,
                    style: &[::lumen_static::Style],
                    st: &mut [::lumen_static::desc::NodeState],
                    i: usize,
                ) {
                    use ::lumen_static::ops::{align_offset, Axis};
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
                }

                fn paint(
                    &self,
                    st: &[::lumen_static::desc::NodeState],
                    i: usize,
                    out: &mut ::lumen_static::DisplayList,
                ) {
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
            }
        };
    })
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

/// Emits `save` (keep fields as a JSON object), `fresh` (`Default`), and
/// `restore` with per-field fallback: an incompatible or missing saved
/// field keeps its fresh value and is logged — the §12.2 rule applied to
/// every field, enum variants included. A fn pointer marked `keep` fails
/// to compile (no `Serialize`), keeping §7's "Saved cannot contain code
/// pointers" property structural.
#[proc_macro_derive(Reloadable, attributes(reload))]
pub fn derive_reloadable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_reloadable(input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn reload_mode(field: &syn::Field) -> syn::Result<ReloadMode> {
    let mut mode = ReloadMode::Nested;
    for attr in &field.attrs {
        if !attr.path().is_ident("reload") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("keep") {
                mode = ReloadMode::Keep;
            } else if meta.path.is_ident("fresh") || meta.path.is_ident("reset") {
                mode = ReloadMode::Skip;
            } else {
                return Err(meta.error("expected `keep`, `fresh`, or `reset`"));
            }
            Ok(())
        })?;
    }
    Ok(mode)
}

fn expand_reloadable(input: DeriveInput) -> syn::Result<Ts2> {
    reject_generics(&input, "Reloadable")?;
    let name = &input.ident;
    let name_str = name.to_string();
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "Reloadable derives structs with named fields; enums travel as \
             #[reload(keep)] serde data inside a struct",
        ));
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
                save_stmts.push(quote! {
                    m.insert(#key.to_owned(), match ::lumen_static::reload::to_value(&self.#ident) {
                        Ok(v) => v,
                        Err(e) => {
                            ::lumen_static::reload::log_fallback(#name_str, #key, &format!("save failed: {e}"));
                            ::lumen_static::reload::Value::Null
                        }
                    });
                });
                restore_stmts.push(quote! {
                    match obj.get(#key) {
                        Some(v) => match ::lumen_static::reload::from_value(v.clone()) {
                            Ok(x) => self.#ident = x,
                            Err(e) => ::lumen_static::reload::log_fallback(
                                #name_str, #key, &format!("incompatible saved value: {e}")),
                        },
                        None => ::lumen_static::reload::log_fallback(#name_str, #key, "missing from saved state"),
                    }
                });
            }
            ReloadMode::Skip => {}
            ReloadMode::Nested => {
                save_stmts.push(quote! {
                    m.insert(#key.to_owned(), ::lumen_static::reload::Reloadable::save(&self.#ident));
                });
                restore_stmts.push(quote! {
                    match obj.get(#key) {
                        Some(v) => ::lumen_static::reload::Reloadable::restore(&mut self.#ident, v),
                        None => ::lumen_static::reload::log_fallback(#name_str, #key, "missing from saved state"),
                    }
                });
            }
        }
    }

    Ok(quote! {
        impl ::lumen_static::reload::Reloadable for #name {
            fn save(&self) -> ::lumen_static::reload::Value {
                let mut m = ::lumen_static::reload::Map::new();
                #(#save_stmts)*
                ::lumen_static::reload::Value::Object(m)
            }
            fn fresh() -> Self {
                <Self as ::core::default::Default>::default()
            }
            fn restore(&mut self, saved: &::lumen_static::reload::Value) {
                let Some(obj) = saved.as_object() else {
                    ::lumen_static::reload::log_fallback(#name_str, "<self>", "saved state is not an object");
                    return;
                };
                #(#restore_stmts)*
            }
        }
    })
}

/// `#[derive(StaticTree)]` on an enum — plan T4.4, §6's variant hatch.
///
/// This is the case the old `base + offset` path could not express:
/// variants overlap in memory, so no const byte offset identifies a node
/// inside one. The accessor path matches the discriminant instead, which
/// is why adopting it unblocked this task
/// (`docs/layout-and-storage-decisions.md` §4d).
///
/// Like `Either`, an enum contributes **one** table node and keeps the
/// active variant's subtree off-table with its own state arena, so
/// `NODE_COUNT` is independent of how many variants there are. Each
/// variant must be a single unnamed field holding a widget.
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
            }

            const _: () = assert!(::lumen_static::widget::table_is_well_formed(&TABLE));
            const _: () = assert!(<#name as ::lumen_static::widget::Widget>::DEPTH <= ::lumen_static::MAX_DEPTH);

            impl ::lumen_static::widget::Node for #name {
                fn kind_name(&self) -> &'static str {
                    #name_str
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
                    ::lumen_static::ops::hide_subtree(
                        st, i + 1, limits, <Self as ::lumen_static::widget::Widget>::NODE_COUNT - 1,
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
                    out: &mut ::lumen_static::DisplayList,
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
    /// format argument; further arguments are written out in full.
    Fmt(syn::LitStr, Vec<syn::Expr>),
    /// `#[bind(t, with = f)]` — `f: fn(&mut Target, &Source)`.
    With(syn::Path),
}

impl syn::parse::Parse for BindSpec {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let target: syn::Expr = input.parse()?;
        input.parse::<syn::Token![,]>().map_err(|_| {
            syn::Error::new_spanned(
                &target,
                "expected `#[bind(target, \"format {}\")]` or `#[bind(target, with = func)]`",
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

        let fmt: syn::LitStr = input.parse()?;
        let mut extra = Vec::new();
        while input.peek(syn::Token![,]) {
            input.parse::<syn::Token![,]>()?;
            if input.is_empty() {
                break;
            }
            extra.push(input.parse()?);
        }
        Ok(BindSpec { target, kind: BindKind::Fmt(fmt, extra) })
    }
}

/// The application root. Emits `impl App`: the tree accessors, and an
/// `update` built from the `#[bind]` declarations on the state fields.
///
/// ```
/// use lumen_static::ops::Label;
/// use lumen_static::{App, StaticTree};
///
/// #[derive(StaticTree, Default)]
/// #[widget(column, spacing = 0.0)]
/// struct Ui {
///     #[widget_id("value")]
///     value: Label<String>,
/// }
///
/// fn recompute(c: &mut Counter) {
///     c.recomputed += 1;
/// }
///
/// #[derive(App, Default)]
/// #[app(after_update = recompute)]     // optional, fn(&mut Self)
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
/// The whole impl is generated so that nothing about the update step is
/// hand-written: a derive that only emitted the binding body would still
/// leave the author to call it from `update`, which is the wiring this
/// is meant to remove.
#[proc_macro_derive(App, attributes(tree, bind, app))]
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

    // Struct-level `#[app(after_update = path)]`.
    let mut after_update: Option<syn::Path> = None;
    for attr in &input.attrs {
        if !attr.path().is_ident("app") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("after_update") {
                after_update = Some(meta.value()?.parse()?);
                Ok(())
            } else {
                Err(meta.error("expected `after_update = path`"))
            }
        })?;
    }

    let mut tree_field: Option<&syn::Field> = None;
    let mut binds = Vec::new();
    for f in &fields.named {
        let ident = f.ident.clone().unwrap();
        for attr in &f.attrs {
            if attr.path().is_ident("tree") {
                if let Some(prev) = tree_field {
                    let mut e = syn::Error::new_spanned(f, "a second `#[tree]` field");
                    e.combine(syn::Error::new_spanned(prev, "the first is here"));
                    return Err(e);
                }
                tree_field = Some(f);
            } else if attr.path().is_ident("bind") {
                let spec: BindSpec = attr.parse_args()?;
                let target = &spec.target;
                binds.push(match &spec.kind {
                    BindKind::Fmt(fmt, extra) => quote! {
                        ::lumen_static::bind::format_into(
                            &mut self.#target,
                            ::core::format_args!(#fmt, &self.#ident #(, #extra)*),
                        );
                    },
                    BindKind::With(path) => quote! {
                        // Spelled out so a mismatched `with =` names the
                        // expected signature instead of failing inside
                        // the generated call.
                        {
                            let f: fn(&mut _, &_) = #path;
                            f(&mut self.#target, &self.#ident);
                        }
                    },
                });
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

    let after = after_update.map(|p| {
        quote! {
            // Typed through a binding so a wrong signature is reported
            // at the hook, not at the call.
            let f: fn(&mut Self) = #p;
            f(self);
        }
    });

    let (impl_g, ty_g, where_g) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_g ::lumen_static::event::App for #name #ty_g #where_g {
            type Tree = #tree_ty;

            #[inline]
            fn tree(&self) -> &Self::Tree {
                &self.#tree_ident
            }

            #[inline]
            fn tree_mut(&mut self) -> &mut Self::Tree {
                &mut self.#tree_ident
            }

            fn update(&mut self) {
                #(#binds)*
                #after
            }
        }
    })
}
