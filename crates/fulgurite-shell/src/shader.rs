//! A program's custom shaders, as data — M83/T83.4
//! (`docs/plan-gpu-extension.md`, "Shaders are data, delivered to the
//! backend").
//!
//! A program registers a fixed table of `ShaderDesc` with
//! `Program::shaders`; `Program::host` installs it through
//! `Host::set_shaders`, so every host a program builds — the window's,
//! every headless frame's, a hot library's — holds it. A widget names an
//! entry by its index, a `ShaderId`, in the
//! `Cmd::Shader` bracket it paints; the index is fixed for a build, the
//! `ImageId` pattern.
//!
//! The table reaches a backend as a
//! [`ShaderTable`](crate::shader::ShaderTable), by value, through two
//! required methods, each consumed on its own side of the seam:
//!
//! | step | method | when |
//! |---|---|---|
//! | host → window | `Session::shaders` | asked by the window; answers the host's table |
//! | window → backend | `Backend::set_shaders` | on every backend `resumed` makes, and after every successful swap |
//!
//! **No borrow survives the call.** A table borrows the session
//! that answered it, and a hot library's strings live in that library; a
//! backend that wants the sources later copies them during
//! `Backend::set_shaders` and keeps nothing of the table itself.
//!
//! **Opaque, and present in every build.** Without this crate's `shader`
//! feature a table is empty and has no contents to read, but the type
//! exists, so `Session` and `Backend` have one vtable shape in every
//! build — `a11y_tree`'s unconditional-slot precedent. Its *layout*
//! still differs with the feature, which is why the reload stamp carries
//! the shell's `shader` entry beside the core's (`session::abi_fields`).

/// One custom shader: a name for messages, its WGSL source, and how many
/// uniform bytes its `@group(0) @binding(1)` struct takes — M83/T83.4.
///
/// Built with [`ShaderDesc::new`], a `const fn`, so a table is a
/// `static`/`const` slice and a too-large `uniform_size` is a compile-time
/// error in such a table. The fields are private for the same reason: a
/// literal would skip the check.
#[cfg(feature = "shader")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShaderDesc {
    name: &'static str,
    wgsl: &'static str,
    uniform_size: u16,
}

#[cfg(feature = "shader")]
impl ShaderDesc {
    /// A shader named `name`, compiled from `wgsl`, taking `uniform_size`
    /// bytes of uniforms (0 declares no uniform binding).
    ///
    /// # Panics
    /// When `uniform_size` exceeds [`MAX_UNIFORMS`](fulgurite::MAX_UNIFORMS)
    /// (256), the most a `Cmd::Shader` bracket records — at compile time
    /// in a `const` or `static` table.
    pub const fn new(name: &'static str, wgsl: &'static str, uniform_size: u16) -> Self {
        assert!(uniform_size as usize <= fulgurite::MAX_UNIFORMS, "uniform_size exceeds MAX_UNIFORMS (256)");
        ShaderDesc { name, wgsl, uniform_size }
    }

    /// What a refusal names.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// The WGSL source a backend compiles.
    pub const fn wgsl(&self) -> &'static str {
        self.wgsl
    }

    /// Uniform bytes the shader's struct takes; a bracket whose uniforms
    /// are another length draws its fallback.
    pub const fn uniform_size(&self) -> u16 {
        self.uniform_size
    }
}

/// A program's shader table, crossing the session seam by value —
/// M83/T83.4. Answered by `Session::shaders` and handed to
/// `Backend::set_shaders`.
///
/// **Borrowed for the call and no longer** (the module's rule): the
/// lifetime is the session's, and a backend copies what it keeps.
/// **Empty without the shell's `shader` feature**, and then it has no
/// contents at all — only [`ShaderTable::len`], which answers 0.
#[derive(Clone, Copy, Debug, Default)]
pub struct ShaderTable<'a> {
    #[cfg(feature = "shader")]
    descs: &'a [ShaderDesc],
    #[cfg(not(feature = "shader"))]
    empty: core::marker::PhantomData<&'a ()>,
}

impl<'a> ShaderTable<'a> {
    /// No shaders: what a program that registered none answers, and the
    /// only table a build without `shader` has.
    pub const EMPTY: ShaderTable<'static> = ShaderTable {
        #[cfg(feature = "shader")]
        descs: &[],
        #[cfg(not(feature = "shader"))]
        empty: core::marker::PhantomData,
    };

    /// The table over `descs`; entry `n` is `ShaderId(n)`.
    #[cfg(feature = "shader")]
    pub const fn new(descs: &'a [ShaderDesc]) -> Self {
        ShaderTable { descs }
    }

    /// The registered shaders, in `ShaderId` order. Borrowed for the
    /// call only: copy what is kept.
    #[cfg(feature = "shader")]
    pub const fn descs(&self) -> &'a [ShaderDesc] {
        self.descs
    }

    /// How many shaders the program registered; 0 in a build without
    /// `shader`.
    pub const fn len(&self) -> usize {
        #[cfg(feature = "shader")]
        {
            self.descs.len()
        }
        #[cfg(not(feature = "shader"))]
        {
            0
        }
    }

    /// Whether the program registered none.
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
