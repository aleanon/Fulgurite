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

#[proc_macro_derive(StaticTree, attributes(widget, widget_id, widget_class))]
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
        .map(|f| {
            for attr in &f.attrs {
                if attr.path().is_ident("widget_class") {
                    let mut out: Vec<String> = Vec::new();
                    let _ = attr.parse_nested_meta(|m| {
                        if let Some(id) = m.path.get_ident() {
                            out.push(id.to_string());
                        }
                        Ok(())
                    });
                    if !out.is_empty() {
                        return quote! { &[#(#out),*] };
                    }
                }
            }
            quote! { &[] }
        })
        .collect();
    let field_wid: Vec<String> = fields
        .named
        .iter()
        .map(|f| {
            for attr in &f.attrs {
                if attr.path().is_ident("widget_id") {
                    if let Ok(lit) = attr.parse_args::<syn::LitStr>() {
                        return lit.value();
                    }
                }
            }
            String::new()
        })
        .collect();

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
            use ::lumen_static::desc::{NodeDesc, NodeState};
            use ::lumen_static::widget::{Node, Widget};
            use ::lumen_static::{Constraints, Point, Size};

            const TABLE: [NodeDesc; <#name as Widget>::NODE_COUNT] = {
                let mut t = [NodeDesc::EMPTY; <#name as Widget>::NODE_COUNT];
                t[0] = NodeDesc::new(
                    #first_child,
                    ::lumen_static::NO_PARENT,
                    0,
                    #child_count,
                    {
                        let mut f = if #has_children { NodeDesc::CONTAINER } else { 0 };
                        if <#name as Widget>::IS_RIGID { f |= NodeDesc::RIGID; }
                        f
                    },
                )
                .with_src(::core::file!(), #root_line);
                let mut at: usize = 1;
                let mut prev_root: usize = 0;
                #(
                    ::lumen_static::widget::splice_child(
                        &mut t, at, <#field_ty as Widget>::SUB_TABLE, 0,
                    );
                    // A child's root carries its *field* site; the
                    // child's internals keep their own.
                    t[at] = t[at]
                        .with_src(::core::file!(), #field_line)
                        .with_id(#field_wid)
                        .with_classes(#field_classes);
                    if prev_root != 0 { t[prev_root].next_sibling = at as u32; }
                    prev_root = at;
                    at += <#field_ty as Widget>::NODE_COUNT;
                )*
                let _ = (at, prev_root);
                t
            };

            impl Widget for #name {
                const NODE_COUNT: usize = 1 #(+ <#field_ty as Widget>::NODE_COUNT)*;
                const DEPTH: usize = 1 + {
                    let mut d = 0usize;
                    #( if <#field_ty as Widget>::DEPTH > d { d = <#field_ty as Widget>::DEPTH; } )*
                    d
                };
                const IS_RIGID: bool = true #(&& <#field_ty as Widget>::IS_RIGID)*;
                const SUB_TABLE: &'static [NodeDesc] = &TABLE;
            }

            // Proof-carrying table: the compiler re-checks the macro's
            // own output at every expansion, and the depth cap turns the
            // stack-overflow hazard into a compile error.
            const _: () = assert!(::lumen_static::widget::table_is_well_formed(&TABLE));
            const _: () = assert!(<#name as Widget>::DEPTH <= ::lumen_static::MAX_DEPTH);

            /// Delegates by index range, like `node`. The where-clause
            /// is what makes a mis-typed `Button<Other>` a compile error
            /// at the *container* that holds it.
            impl<S> ::lumen_static::event::Handlers<S> for #name
            where
                #(#field_ty: ::lumen_static::event::Handlers<S>,)*
            {
                fn handler(
                    &self,
                    sel: ::lumen_static::event::NodeSel,
                ) -> ::core::option::Option<fn(&mut S, &::lumen_static::EventCtx)> {
                    if sel.i == 0 { return ::core::option::Option::None; }
                    let mut at: usize = 1;
                    #(
                        {
                            let c = <#field_ty as Widget>::NODE_COUNT;
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

            impl Node for #name {
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
                    limits: Constraints,
                    style: &[::lumen_static::Style],
                    st: &mut [NodeState],
                    i: usize,
                ) -> Size {
                    use ::lumen_static::ops::{
                        align_offset, fill_child, inner_limits, loose_child, resolve_self, spaced,
                        Axis,
                    };
                    let me = style[i];
                    if !me.visible {
                        return ::lumen_static::ops::record(st, i, limits, Size::default());
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
                                let s = d.layout(loose_child(axis, inner, avail, me.align), style, st, at);
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
                            at += <#field_ty as Widget>::NODE_COUNT;
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
                                let s = d.layout(fill_child(axis, inner, share, me.align), style, st, at);
                                used += axis.main(s);
                                if !axis.cross_len(&cs).is_fill() {
                                    cross = cross.max(axis.cross(s));
                                }
                            }
                            at += <#field_ty as Widget>::NODE_COUNT;
                        }
                    )*
                    }
                    let _ = at;

                    let content = axis.size(used, cross);
                    let padded = Size {
                        w: content.w + me.padding.horizontal(),
                        h: content.h + me.padding.vertical(),
                    };
                    ::lumen_static::ops::record(st, i, limits, resolve_self(limits, &me, padded))
                }

                fn place(
                    &mut self,
                    at_pos: Point,
                    style: &[::lumen_static::Style],
                    st: &mut [NodeState],
                    i: usize,
                ) {
                    use ::lumen_static::ops::{align_offset, Axis};
                    st[i].pos = at_pos;
                    let me = style[i];
                    if !me.visible {
                        return;
                    }
                    let axis = #axis_expr;
                    let cross_extent = axis.cross(st[i].size)
                        - match axis {
                            Axis::Row => me.padding.vertical(),
                            Axis::Column => me.padding.horizontal(),
                        };
                    let mut main = axis.main(Size { w: at_pos.x, h: at_pos.y })
                        + match axis {
                            Axis::Row => me.padding.left,
                            Axis::Column => me.padding.top,
                        };
                    let cross0 = axis.cross(Size { w: at_pos.x, h: at_pos.y })
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
                            if axis.cross_len(&style[at]).is_fill() {
                                let s = st[at].size;
                                st[at].size = match axis {
                                    Axis::Row => Size { w: s.w, h: cross_extent },
                                    Axis::Column => Size { w: cross_extent, h: s.h },
                                };
                            }
                            let size = st[at].size;
                            let off = align_offset(me.align, cross_extent, axis.cross(size));
                            let p = match axis {
                                Axis::Row => Point { x: main, y: cross0 + off },
                                Axis::Column => Point { x: cross0 + off, y: main },
                            };
                            #mut_calls
                            d.place(p, style, st, at);
                            main += axis.main(size) + me.spacing;
                            at += <#field_ty as Widget>::NODE_COUNT;
                        }
                    )*
                    let _ = (at, main);
                }

                fn paint(&self, st: &[NodeState], i: usize, out: &mut u64) {
                    ::lumen_static::ops::fold_geom(out, 0, &st[i]);
                    let mut at = i + 1;
                    #(
                        {
                            #ref_calls
                            d.paint(st, at, out);
                            at += <#field_ty as Widget>::NODE_COUNT;
                        }
                    )*
                    let _ = at;
                }

                fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32)
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
                            at += <#field_ty as Widget>::NODE_COUNT;
                        }
                    )*
                    let _ = at;
                    ::core::option::Option::Some(::lumen_static::Hit {
                        node: i as u32,
                        inner: ::core::option::Option::None,
                        local: Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
                    })
                }

                fn node(&self, i: usize) -> &dyn Node {
                    if i == 0 { return self; }
                    let mut at: usize = 1;
                    #(
                        {
                            let c = <#field_ty as Widget>::NODE_COUNT;
                            if i < at + c { return Node::node(&self.#field_ident, i - at); }
                            at += c;
                        }
                    )*
                    let _ = at;
                    ::core::unreachable!("node index out of range")
                }

                fn node_mut(&mut self, i: usize) -> &mut dyn Node {
                    if i == 0 { return self; }
                    let mut at: usize = 1;
                    #(
                        {
                            let c = <#field_ty as Widget>::NODE_COUNT;
                            if i < at + c {
                                return Node::node_mut(&mut self.#field_ident, i - at);
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
    let idx: Vec<u64> = (0..var_ident.len() as u64).collect();

    Ok(quote! {
        const _: () = {
            use ::lumen_static::desc::{NodeDesc, NodeState};
            use ::lumen_static::widget::{Node, Widget};
            use ::lumen_static::{Constraints, Point, Size, Style};

            const TABLE: [NodeDesc; 1] = [NodeDesc::new(
                0,
                ::lumen_static::NO_PARENT,
                0,
                0,
                NodeDesc::CONTAINER,
            )
            .with_src(::core::file!(), #root_line)];

            impl Widget for #name {
                const NODE_COUNT: usize = 1;
                const DEPTH: usize = 1 + {
                    let mut d = 0usize;
                    #( if <#var_ty as Widget>::DEPTH > d { d = <#var_ty as Widget>::DEPTH; } )*
                    d
                };
                const IS_RIGID: bool = true #(&& <#var_ty as Widget>::IS_RIGID)*;
                const SUB_TABLE: &'static [NodeDesc] = &TABLE;
            }

            const _: () = assert!(::lumen_static::widget::table_is_well_formed(&TABLE));
            const _: () = assert!(<#name as Widget>::DEPTH <= ::lumen_static::MAX_DEPTH);

            impl Node for #name {
                fn kind_name(&self) -> &'static str {
                    #name_str
                }

                fn layout(
                    &mut self,
                    limits: Constraints,
                    style: &[Style],
                    st: &mut [NodeState],
                    i: usize,
                ) -> Size {
                    let me = style[i];
                    let size = match self {
                        #( #name::#var_ident(w) => {
                            let mut ist =
                                ::std::vec![NodeState::default(); <#var_ty as Widget>::NODE_COUNT];
                            let isty: ::std::vec::Vec<Style> =
                                (0..<#var_ty as Widget>::NODE_COUNT)
                                    .map(|j| Node::node(w, j).defaults())
                                    .collect();
                            let s = w.layout(limits, &isty, &mut ist, 0);
                            s
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
                    at: Point,
                    _style: &[Style],
                    st: &mut [NodeState],
                    i: usize,
                ) {
                    st[i].pos = at;
                }

                fn paint(&self, st: &[NodeState], i: usize, out: &mut u64) {
                    ::lumen_static::ops::fold_geom(out, 9, &st[i]);
                    match self {
                        #( #name::#var_ident(_) => {
                            ::lumen_static::ops::fold_pub(out, #idx);
                        } ),*
                    }
                }

                fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32)
                    -> ::core::option::Option<::lumen_static::Hit>
                {
                    if !::lumen_static::hit::contains(&st[i], x, y) {
                        return ::core::option::Option::None;
                    }
                    ::core::option::Option::Some(::lumen_static::Hit {
                        node: i as u32,
                        inner: ::core::option::Option::None,
                        local: Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
                    })
                }

                /// The match the byte-offset path could not express: the
                /// discriminant selects which subtree index `i` names.
                fn node(&self, _i: usize) -> &dyn Node {
                    self
                }

                fn node_mut(&mut self, _i: usize) -> &mut dyn Node {
                    self
                }
            }

            impl<S> ::lumen_static::event::Handlers<S> for #name {
                fn handler(
                    &self,
                    _sel: ::lumen_static::event::NodeSel,
                ) -> ::core::option::Option<fn(&mut S, &::lumen_static::EventCtx)> {
                    ::core::option::Option::None
                }
            }
        };
    })
}
