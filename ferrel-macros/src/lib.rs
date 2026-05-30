//! Inert attribute macros for ferrel transpiler input.
//!
//! The ferrel transpiler reads VALID, COMPILABLE Rust and recognizes a few
//! attributes on functions: `#[interactive]`, `#[interactive("p")]`, and
//! `#[elisp]` / `#[elisp(name = "..")]`. For that source to compile with
//! `rustc`, those attributes must be real, registered attribute macros. This
//! crate provides them as no-ops: they parse and accept their arguments, then
//! return the annotated item completely unchanged. The transpiler reads the
//! attribute syntax directly from the source; these macros only exist so the
//! Rust side type-checks.

use proc_macro::TokenStream;

/// Mark a function as an interactive Emacs command.
///
/// `#[interactive]` lowers to `(interactive)`; `#[interactive("p")]` lowers to
/// `(interactive "p")`. As a Rust macro it is a no-op: it returns the function
/// unchanged so the program still compiles.
#[proc_macro_attribute]
pub fn interactive(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Mark a function as a foreign Elisp declaration.
///
/// `#[elisp]` registers the function as an Elisp builtin whose name is the
/// kebab-case of the Rust name; `#[elisp(name = "1+")]` overrides that with an
/// explicit Elisp symbol. As a Rust macro it is a no-op: it returns the function
/// unchanged so the program still compiles, and the transpiler reads the
/// attribute from the source.
#[proc_macro_attribute]
pub fn elisp(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
