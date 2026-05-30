//! The `ferrel::rt` prelude: zero-cost stubs that make transpiler input compile.
//!
//! The transpiler reads VALID, COMPILABLE Rust. That is only possible if the
//! Elisp functions and the transpiler intrinsics a user writes actually exist as
//! Rust items. This module supplies them as safe, zero-cost stubs: a user writes
//! `use ferrel::rt::*;` and then ordinary supported Rust, and the file both
//! compiles with `rustc` and transpiles with [`crate::transpile`].
//!
//! The stub bodies are never the thing that runs in Emacs. The transpiler maps
//! each call to the corresponding Elisp symbol and discards the Rust body. The
//! bodies exist only so `rustc` can type-check the source.
//!
//! # Intrinsics
//!
//! Four helpers lower specially instead of as ordinary calls:
//!
//! - [`sym`] -> a quoted symbol literal `'name`.
//! - [`func`] -> a function reference `#'name`.
//! - [`kbd`] -> a key sequence `(kbd "..")`.
//! - [`raw`] -> verbatim Elisp text, the visible last resort.
//!
//! Each takes a string literal so the transpiler can read the value at compile
//! time, and each has a real body here so the source compiles.
//!
//! # FFI builtins
//!
//! The rest are a starter set of the Elisp builtins a typical config touches:
//! [`message`], [`format`], [`expand_file_name`], [`executable_find`], and so
//! on. To call a builtin not listed here, declare it yourself with an `#[elisp]`
//! foreign function (see [`crate::transpile`]) or reach for [`raw`].
//!
//! ```
//! // This is the shape of a transpiler input file.
//! use ferrel::rt::*;
//!
//! /// Greet the user in the echo area.
//! fn greet() {
//!     message("Hello from ferrel!");
//! }
//! # greet();
//! ```

#![allow(clippy::must_use_candidate, clippy::missing_panics_doc)]

pub use ferrel_macros::{elisp, interactive};

/// An opaque Elisp value, returned by stubs whose Elisp result has no useful
/// Rust type. It carries no data: the transpiler discards stub bodies, so the
/// value never participates in real evaluation.
///
/// `Elisp` is intentionally inert. It exists so that builtins which return a
/// value (rather than a string, integer, or boolean) still type-check on the
/// Rust side without forcing the user to invent a concrete type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Elisp;

// --- Intrinsics --------------------------------------------------------------

/// Intrinsic: a quoted symbol literal. Lowers to `'name`.
///
/// Use this wherever Elisp wants a literal symbol: hook names, mode names, the
/// car of an alist entry. The argument must be a string literal.
///
/// ```
/// use ferrel::rt::sym;
/// let _ = sym("text-mode-hook");
/// ```
pub fn sym(_name: &str) -> Elisp {
    Elisp
}

/// Intrinsic: a function reference. Lowers to `#'name`.
///
/// Use this for the function argument of `add-hook`, `global-set-key`, and
/// similar. The argument must be a string literal.
///
/// ```
/// use ferrel::rt::func;
/// let _ = func("save-buffer");
/// ```
pub fn func(_name: &str) -> Elisp {
    Elisp
}

/// Intrinsic: a key sequence. Lowers to `(kbd "..")`.
///
/// The argument must be a string literal in Emacs `kbd` syntax, e.g. `"C-c h"`.
///
/// ```
/// use ferrel::rt::kbd;
/// let _ = kbd("C-c h");
/// ```
pub fn kbd(_keys: &str) -> Elisp {
    Elisp
}

/// Intrinsic: verbatim Elisp text. Lowers to the given string unchanged.
///
/// This is the visible last resort for Elisp that the subset cannot express.
/// The argument must be a string literal and is emitted exactly as written, so
/// the user is responsible for its correctness.
///
/// ```
/// use ferrel::rt::raw;
/// let _ = raw("(setq gc-cons-threshold (* 100 1024 1024))");
/// ```
pub fn raw(_elisp: &str) -> Elisp {
    Elisp
}

// --- String and message builtins ---------------------------------------------

/// `(message fmt args...)`. Display a formatted message in the echo area.
///
/// The stub accepts any string-like format and returns it; the transpiler emits
/// `(message ..)` with the lowered arguments.
pub fn message(fmt: impl Into<String>) -> String {
    fmt.into()
}

/// `(format fmt args...)`. Build a formatted string.
pub fn format(fmt: impl Into<String>) -> String {
    fmt.into()
}

/// `(concat a b ...)`. Concatenate strings.
pub fn concat(a: impl Into<String>) -> String {
    a.into()
}

/// `(format-time-string fmt)`. Format the current time.
pub fn format_time_string(fmt: impl Into<String>) -> String {
    fmt.into()
}

// --- File and process builtins -----------------------------------------------

/// `(expand-file-name name)`. Expand a path into an absolute one.
pub fn expand_file_name(name: impl Into<String>) -> String {
    name.into()
}

/// `(executable-find program)`. Find an executable on `PATH`, or `nil`.
pub fn executable_find(program: impl Into<String>) -> String {
    program.into()
}

/// `(file-exists-p path)`. Whether a file exists.
pub fn file_exists_p(_path: impl Into<String>) -> bool {
    false
}

// --- Editing builtins --------------------------------------------------------

/// `(insert s)`. Insert text at point.
pub fn insert(_s: impl Into<String>) -> Elisp {
    Elisp
}

/// `(point)`. The current buffer position.
pub fn point() -> i64 {
    0
}

/// `(buffer-name)`. The current buffer's name.
pub fn buffer_name() -> String {
    String::new()
}

// --- Hooks and keybindings ---------------------------------------------------

/// `(add-hook 'hook #'function)`. Register a function on a hook.
///
/// Pass the hook with [`sym`] and the function with [`func`] so they lower to a
/// quoted symbol and a function reference respectively.
pub fn add_hook(_hook: Elisp, _function: Elisp) -> Elisp {
    Elisp
}

/// `(global-set-key (kbd "..") #'command)`. Bind a key globally.
///
/// Pass the key with [`kbd`] and the command with [`func`].
pub fn global_set_key(_key: Elisp, _command: Elisp) -> Elisp {
    Elisp
}

/// `(require 'feature)`. Load a feature if not already loaded.
pub fn require(_feature: Elisp) -> Elisp {
    Elisp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intrinsics_and_builtins_compile_and_run() {
        // The point of this test is that the stub surface type-checks; the
        // bodies are inert and never used by the transpiler.
        assert_eq!(sym("x"), Elisp);
        assert_eq!(func("f"), Elisp);
        assert_eq!(kbd("C-c h"), Elisp);
        assert_eq!(raw("(progn)"), Elisp);
        assert_eq!(message("hi"), "hi");
        assert_eq!(format("%d"), "%d");
        assert_eq!(expand_file_name("~/x"), "~/x");
        assert_eq!(point(), 0);
        assert!(!file_exists_p("/nope"));
    }
}
