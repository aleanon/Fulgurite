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

    // ---- #[widget(row|column, spacing = <f32>)] ----
    let mut axis = None;
    let mut spacing = 0.0f32;
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
            } else {
                return Err(meta.error("expected `row`, `column`, or `spacing = <float>`"));
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

            const OPS: WidgetOps = WidgetOps {
                name: #name_str,
                down: __down,
                up: __up,
                place: __place,
                paint: ::lumen_static::ops::container_paint,
            };

            const TABLE: [NodeDesc; <#name as ::lumen_static::Widget>::NODE_COUNT] = {
                let mut t = [NodeDesc::EMPTY; <#name as ::lumen_static::Widget>::NODE_COUNT];
                t[0] = NodeDesc {
                    offset: 0,
                    kind: 0,
                    first_child: #first_child,
                    child_count: #child_count,
                    parent: ::lumen_static::NO_PARENT,
                    next_sibling: 0,
                    flags: {
                        let mut f = if #has_children { NodeDesc::CONTAINER } else { 0 };
                        if <#name as ::lumen_static::Widget>::IS_RIGID {
                            f |= NodeDesc::RIGID;
                        }
                        f
                    },
                };
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
