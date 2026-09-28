//! `.lss` stylesheets for lumen-static — M61 (`docs/plan-lss.md`).
//!
//! A stylesheet written as a file, costing nothing at run time that a
//! hand-written `&[Rule]` does not already cost: `include_lss!` runs this
//! crate while the invoking crate compiles and emits the same `const`
//! rules a person writes today. The lexer and parser are adopted from
//! Lumen's `lumen-style`, trimmed to what this framework's cascade can
//! apply (§2.1); Lumen's resolver and animation modules are not here.
//!
//! - [`parse`] — text to syntax, every error with its `line:col`.

#![forbid(unsafe_code)]

pub mod ast;
pub mod lexer;
pub mod parser;

pub use ast::Span;
pub use parser::{parse, Error};

#[cfg(test)]
mod tests;
