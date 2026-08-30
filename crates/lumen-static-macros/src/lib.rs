//! `#[derive(StaticTree)]` — emits the flat const node table for a
//! concrete container struct (handoff §5).
//!
//! ```ignore
//! #[derive(StaticTree)]
//! #[widget(row, spacing = 4.0)]
//! struct Header { title: Label, gap: Spacer, close: Fixed }
//! ```
//!
//! Every field is a child widget (a leaf from `lumen_static::ops` or
//! another derived struct). The emitted impl is all-const: `NODE_COUNT` /
//! `KIND_COUNT` arithmetic over the field types, a `TABLE` spliced
//! together at compile time with `offset_of!` byte bases, and a kind list
//! with the struct's own ops first.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Ts2;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

enum Axis {
    Row,
    Column,
}

#[proc_macro_derive(StaticTree, attributes(widget))]
pub fn derive_static_tree(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand(input: DeriveInput) -> syn::Result<Ts2> {
    let name = &input.ident;
    let name_str = name.to_string();

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
        return Err(syn::Error::new_spanned(&input.ident, "StaticTree derives structs only"));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(&input.ident, "StaticTree needs named fields"));
    };
    let field_ident: Vec<_> = fields.named.iter().map(|f| f.ident.clone().unwrap()).collect();
    let field_ty: Vec<_> = fields.named.iter().map(|f| f.ty.clone()).collect();
    let n_fields = field_ident.len();
    // §9 provenance: declaration lines from proc-macro2 span locations;
    // the file comes from `file!()` expanded at the derive's call site.
    let root_line = input.ident.span().start().line as u32;
    let field_line: Vec<u32> =
        fields.named.iter().map(|f| f.ident.as_ref().unwrap().span().start().line as u32).collect();

    if fuse {
        return expand_fused(
            name, &name_str, &axis, spacing, &field_ident, &field_ty, root_line,
        );
    }

    let (down_fn, up_fn, place_fn) = match axis {
        Axis::Row => (quote!(row_down), quote!(row_up), quote!(row_place)),
        Axis::Column => (quote!(col_down), quote!(col_up), quote!(col_place)),
    };
    let first_child: u32 = if n_fields > 0 { 1 } else { 0 };
    let child_count = n_fields as u16;
    let has_children = n_fields > 0;
    let sp = spacing;

    // The anonymous-const wrapper keeps helper items unnameable while the
    // trait impl leaks out, the standard derive pattern.
    Ok(quote! {
        const _: () = {
            use ::lumen_static::desc::{NodeDesc, NodeState, Size, WidgetOps};

            fn __down(_b: *const u8, t: &[NodeDesc], st: &mut [NodeState], i: usize) {
                ::lumen_static::ops::#down_fn(t, st, i, #sp)
            }
            fn __up(_b: *const u8, t: &[NodeDesc], st: &[NodeState], i: usize) -> Size {
                ::lumen_static::ops::#up_fn(t, st, i, #sp)
            }
            fn __place(_b: *const u8, t: &[NodeDesc], st: &mut [NodeState], i: usize) {
                ::lumen_static::ops::#place_fn(t, st, i, #sp)
            }

            const OPS: WidgetOps =
                WidgetOps::new(#name_str, __down, __up, __place, ::lumen_static::ops::container_paint);

            const TABLE: [NodeDesc; <#name as ::lumen_static::Widget>::NODE_COUNT] = {
                let mut t = [NodeDesc::EMPTY; <#name as ::lumen_static::Widget>::NODE_COUNT];
                t[0] = NodeDesc::new(
                    0,
                    0,
                    #first_child,
                    #child_count,
                    ::lumen_static::NO_PARENT,
                    0,
                    {
                        let mut f = if #has_children { NodeDesc::CONTAINER } else { 0 };
                        if <#name as ::lumen_static::Widget>::IS_RIGID {
                            f |= NodeDesc::RIGID;
                        }
                        f
                    },
                )
                .with_src(::core::file!(), #root_line);
                let mut at: usize = 1;
                let mut kb: u16 = 1;
                let mut prev_root: usize = 0;
                #(
                    ::lumen_static::widget::splice_child(
                        &mut t,
                        at,
                        <#field_ty as ::lumen_static::Widget>::SUB_TABLE,
                        0,
                        ::core::mem::offset_of!(#name, #field_ident) as u32,
                        kb,
                    );
                    // Provenance of a child's root is its *field* site;
                    // its internals keep their own declaration sites.
                    t[at] = t[at].with_src(::core::file!(), #field_line);
                    if prev_root != 0 {
                        t[prev_root].next_sibling = at as u32;
                    }
                    prev_root = at;
                    at += <#field_ty as ::lumen_static::Widget>::NODE_COUNT;
                    kb += <#field_ty as ::lumen_static::Widget>::KIND_COUNT as u16;
                )*
                let _ = (at, kb, prev_root);
                t
            };

            const KINDS: [&'static WidgetOps; <#name as ::lumen_static::Widget>::KIND_COUNT] = {
                let mut ks: [&'static WidgetOps; <#name as ::lumen_static::Widget>::KIND_COUNT] =
                    [&OPS; <#name as ::lumen_static::Widget>::KIND_COUNT];
                let mut at: usize = 1;
                #(
                    ::lumen_static::widget::splice_kinds(
                        &mut ks,
                        at,
                        <#field_ty as ::lumen_static::Widget>::KIND_LIST,
                    );
                    at += <#field_ty as ::lumen_static::Widget>::KIND_COUNT;
                )*
                let _ = at;
                ks
            };

            impl ::lumen_static::Widget for #name {
                const NODE_COUNT: usize =
                    1 #(+ <#field_ty as ::lumen_static::Widget>::NODE_COUNT)*;
                const KIND_COUNT: usize =
                    1 #(+ <#field_ty as ::lumen_static::Widget>::KIND_COUNT)*;
                const IS_RIGID: bool =
                    true #(&& <#field_ty as ::lumen_static::Widget>::IS_RIGID)*;
                const SUB_TABLE: &'static [NodeDesc] = &TABLE;
                const KIND_LIST: &'static [&'static WidgetOps] = &KINDS;
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

/// The §5 erasure dial: one table entry, one kind, and the subtree's
/// layout/paint emitted as straight-line code over `Inline` — children
/// are called directly, so the compiler inlines and const-folds across
/// the whole subtree. Fields must implement `Inline` (leaves or other
/// fused structs); a table container inside a fused one fails to compile.
#[allow(clippy::too_many_arguments)]
fn expand_fused(
    name: &syn::Ident,
    name_str: &str,
    axis: &Axis,
    spacing: f32,
    field_ident: &[syn::Ident],
    field_ty: &[syn::Type],
    root_line: u32,
) -> syn::Result<Ts2> {
    let k = field_ident.len();
    let spaced = if k > 1 { spacing * (k - 1) as f32 } else { 0.0 };
    let nf = if k > 0 { k as f32 } else { 1.0 };
    let sp = spacing;

    let (cc_expr, acc_child, result_expr, advance_stmt) = match axis {
        Axis::Row => (
            quote! { ::lumen_static::Constraints {
                min_w: 0.0,
                max_w: ((c.max_w - #spaced) / #nf).max(0.0),
                min_h: 0.0,
                max_h: c.max_h,
            } },
            quote! { sum += s.w; mx = mx.max(s.h); },
            quote! { c.clamp(::lumen_static::Size { w: sum + #spaced, h: mx }) },
            quote! { cur.x += s.w + #sp; },
        ),
        Axis::Column => (
            quote! { ::lumen_static::Constraints {
                min_w: 0.0,
                max_w: c.max_w,
                min_h: 0.0,
                max_h: ((c.max_h - #spaced) / #nf).max(0.0),
            } },
            quote! { sum += s.h; mx = mx.max(s.w); },
            quote! { c.clamp(::lumen_static::Size { w: mx, h: sum + #spaced }) },
            quote! { cur.y += s.h + #sp; },
        ),
    };

    Ok(quote! {
        const _: () = {
            use ::lumen_static::desc::{NodeDesc, NodeState, Size, WidgetOps};
            use ::lumen_static::fuse::Inline;

            impl Inline for #name {
                #[allow(unused_mut, unused_variables)]
                fn measure(&self, c: ::lumen_static::Constraints) -> ::lumen_static::Size {
                    let cc = #cc_expr;
                    let mut sum = 0.0f32;
                    let mut mx = 0.0f32;
                    #(
                        let s = Inline::measure(&self.#field_ident, cc);
                        #acc_child
                    )*
                    #result_expr
                }
                #[allow(unused_mut, unused_variables)]
                fn paint_inline(
                    &self,
                    c: ::lumen_static::Constraints,
                    pos: ::lumen_static::Point,
                    size: ::lumen_static::Size,
                    out: &mut u64,
                ) {
                    ::lumen_static::fuse::fold_tagged(out, 0, size, pos);
                    let cc = #cc_expr;
                    let mut cur = pos;
                    #(
                        let s = Inline::measure(&self.#field_ident, cc);
                        Inline::paint_inline(&self.#field_ident, cc, cur, s, out);
                        #advance_stmt
                    )*
                }
            }

            fn __up(b: *const u8, t: &[NodeDesc], st: &[NodeState], i: usize) -> Size {
                let s: &#name = unsafe { &*(b.add(t[i].offset as usize) as *const #name) };
                Inline::measure(s, st[i].constraints)
            }
            fn __paint(b: *const u8, d: &NodeDesc, st: &NodeState, out: &mut u64) {
                let s: &#name = unsafe { &*(b.add(d.offset as usize) as *const #name) };
                Inline::paint_inline(s, st.constraints, st.pos, st.size, out)
            }

            const OPS: WidgetOps = WidgetOps::new(
                #name_str,
                ::lumen_static::desc::leaf_nop,
                __up,
                ::lumen_static::desc::leaf_nop,
                __paint,
            );

            const TABLE: [NodeDesc; 1] = [NodeDesc::new(
                0,
                0,
                0,
                0,
                ::lumen_static::NO_PARENT,
                0,
                if <#name as ::lumen_static::Widget>::IS_RIGID {
                    NodeDesc::RIGID
                } else {
                    0
                },
            )
            .with_src(::core::file!(), #root_line)];

            impl ::lumen_static::Widget for #name {
                const NODE_COUNT: usize = 1;
                const KIND_COUNT: usize = 1;
                const IS_RIGID: bool =
                    true #(&& <#field_ty as ::lumen_static::Widget>::IS_RIGID)*;
                const SUB_TABLE: &'static [NodeDesc] = &TABLE;
                const KIND_LIST: &'static [&'static WidgetOps] = &[&OPS];
            }
        };
    })
}
