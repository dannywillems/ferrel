//! # ferrel
//!
//! A typed Rust eDSL that generates Emacs Lisp (`.el`) plugins and config.
//!
//! ferrel exists for people who want to write Emacs extensions and
//! configuration in a language they already know and trust, with a type
//! checker, instead of reading and writing parenthesis-dense, dynamically
//! typed Elisp by hand.
//!
//! The model is deliberately simple:
//!
//! 1. You build a [`Package`] out of typed expressions ([`El<T>`]) and
//!    top-level forms ([`Defun`], [`defcustom`], [`global_set_key`], ...).
//! 2. The Rust compiler type-checks the expressions for you.
//! 3. [`Package::render`] emits a clean, human-readable `.el` file that Emacs
//!    loads exactly like hand-written Elisp. No runtime, no native module, no
//!    dependency added to your Emacs.
//!
//! ## Example
//!
//! ```
//! use ferrel::*;
//!
//! let pkg = Package::new("ferrel-hello", "A greeting command.")
//!     .emacs_version("27.1")
//!     .defun(
//!         Defun::new("ferrel-hello")
//!             .doc("Greet the user.")
//!             .interactive()
//!             .body([message(string("Hello from Rust-generated Elisp!"), [])]),
//!     )
//!     .form(global_set_key("C-c h", "ferrel-hello"));
//!
//! let el = pkg.render();
//! assert!(el.contains("(defun ferrel-hello ()"));
//! ```
//!
//! ## What ferrel types, and what it does not
//!
//! Emacs is enormous and every third-party package adds more untyped
//! functions. ferrel types the core you touch constantly (arithmetic, strings,
//! control flow, common builtins) and gives you [`call`] as an ergonomic,
//! explicit escape hatch for everything else. The escape hatch is a feature,
//! not a wart: it keeps you productive on day one instead of waiting for full
//! coverage of Emacs that will never come.

//! ## Compiler pipeline
//!
//! ferrel is structured as a small compiler with two directions sharing one
//! AST ([`Sexp`]):
//!
//! ```text
//!   .el text --[lexer]--> tokens --[parser]--> Sexp --[codegen]--> .el text
//!                                               ^
//!   Rust eDSL (El<T>, typed authoring layer) ---+
//! ```
//!
//! - `lexer` (private): text to spanned tokens.
//! - `parser` (private): tokens to [`Sexp`]; re-exported as [`parse`] /
//!   [`parse_one`].
//! - [`Sexp`]: the untyped AST and the code generator (`render`).
//! - The typed layer ([`El<T>`], builders) lowers into [`Sexp`] for generation.
//!
//! Because both directions share [`Sexp`], the pipeline round-trips: parsing
//! then rendering reproduces an equivalent file, and rendering then parsing
//! reproduces an equal AST.

//! ## Naming notes
//!
//! A few names are known to be less than ideal but are kept for stability:
//!
//! - [`El<T>`] could read more clearly as `Expr`, but renaming the central type
//!   is a large breaking change and is deliberately not done here.
//! - The typed `setq` expression is re-exported as [`typed_setq`] to avoid a
//!   clash with the top-level statement [`setq`]; the underscored control-flow
//!   helpers [`if_`], [`when`], [`unless`] avoid Rust keyword collisions.

mod form;
mod key;
mod lexer;
mod package;
mod parser;
mod sexp;
mod stmt;
mod typed;
mod use_package;

pub use form::{
    CustomType, Defcustom, Defun, InferCustomType, add_hook, bind_key, defconst, defcustom, defvar,
    disable_mode, enable_mode, global_set_key, require, setq,
};
pub use key::{Command, KeyError, KeySeq};
pub use lexer::{LexError, Token, tokenize};
pub use package::Package;
pub use parser::{ParseError, parse, parse_one};
pub use sexp::Sexp;
pub use stmt::Stmt;
pub use typed::{
    Any, Bool, El, Float, Int, IntoSexp, List, Str, Symbol, add, boolean, buffer_name, call,
    call_typed, concat, equal, executable_find, expand_file_name, float, format,
    format_time_string, gt, if_, insert, int, lt, message, mul, nil, not, num_eq, pair, point,
    progn, quoted_symbol, string, sub, sum, t, unless, var, when,
};
pub use use_package::{Defer, Hook, UsePackage};

// `setq` exists both as a top-level statement (form::setq) and a typed
// expression (typed::setq). The form variant is re-exported above; the typed
// one is reached via `ferrel::typed_setq` to avoid a name clash.
pub use typed::setq as typed_setq;
