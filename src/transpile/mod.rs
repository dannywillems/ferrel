//! The Rust-subset transpiler front-end.
//!
//! This module reads a subset of real, compilable Rust and lowers it into the
//! [`Sexp`](crate::Sexp) IR, which the existing code generator renders to
//! `.el`. It is the third front-end of ferrel, alongside the Elisp parser and
//! the typed eDSL, and it reuses the entire back-end:
//!
//! ```text
//!   Rust source --[syn]--> Rust AST --[lower]--> Sexp --[codegen]--> .el text
//! ```
//!
//! # The contract
//!
//! The input must be VALID, COMPILABLE, TYPE-CHECKED Rust. `rustc` is the type
//! checker; ferrel only lowers already-parsed syntax. To make user code compile
//! as ordinary safe Rust, the [`crate::rt`] prelude supplies zero-cost stubs for
//! the Elisp builtins and the intrinsics the transpiler recognizes. A user
//! writing `use ferrel::rt::*;` plus supported Rust gets code that both compiles
//! with `rustc` and transpiles.
//!
//! # The honesty rule
//!
//! Every supported construct lowers to Elisp with identical observable behavior,
//! or the transpiler refuses it with a [`TranspileError`] carrying a source
//! location. It never silently drops or mistranslates a construct.
//!
//! # The supported subset
//!
//! - Items: `fn`, `const`, `static`, `use` (ignored), and `#[elisp]`-marked
//!   foreign declarations.
//! - Statements: `let x = e;`, expression statements, the trailing expression,
//!   `x = e;`, and tail `return e;`.
//! - Expressions: literals, identifiers, arithmetic and comparison, `&&`/`||`/
//!   `!`, calls and method calls, `if`/`else`, `match`, `while`, `for`,
//!   closures, and the `format!`/`println!` macros.
//!
//! Anything outside the subset is refused; see the crate documentation and the
//! tests in this module for the exact list.
//!
//! ```
//! use ferrel::transpile::transpile_str;
//!
//! let forms = transpile_str("fn greet() { message(\"hi\"); }").unwrap();
//! assert_eq!(forms[0].render(), "(defun greet () (message \"hi\"))");
//! ```

mod error;
mod lower;
mod naming;

use std::path::Path;

pub use error::TranspileError;
pub use naming::kebab_case;

use crate::sexp::Sexp;

/// Transpile a string of Rust source into a sequence of top-level `Sexp` forms.
///
/// # Errors
///
/// Returns a [`TranspileError`] if the input fails to parse as Rust, or if it
/// uses a construct outside the supported subset. The error carries the line and
/// column of the offending construct.
///
/// ```
/// use ferrel::transpile::transpile_str;
///
/// let forms = transpile_str("const MAX_ITEMS: i64 = 200;").unwrap();
/// assert_eq!(forms[0].render(), "(defconst max-items 200)");
/// ```
pub fn transpile_str(src: &str) -> Result<Vec<Sexp>, TranspileError> {
    let file = syn::parse_file(src)?;
    lower::lower_file(&file)
}

/// Transpile a `.rs` file on disk into a sequence of top-level `Sexp` forms.
///
/// # Errors
///
/// Returns a [`TranspileError`] if the file cannot be read, fails to parse as
/// Rust, or uses an unsupported construct. A read error is reported at line 0,
/// column 0 since it has no source span.
pub fn transpile_file(path: impl AsRef<Path>) -> Result<Vec<Sexp>, TranspileError> {
    let src = std::fs::read_to_string(path.as_ref())
        .map_err(|e| TranspileError::at(format!("cannot read input file: {e}"), 0, 0))?;
    transpile_str(&src)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(src: &str) -> String {
        let forms = transpile_str(src).expect("should transpile");
        assert_eq!(forms.len(), 1, "expected exactly one form for `{src}`");
        forms[0].render()
    }

    fn refuse(src: &str) -> TranspileError {
        transpile_str(src).expect_err("should be refused")
    }

    // --- Items ---------------------------------------------------------------

    #[test]
    fn lowers_fn_to_defun() {
        assert_eq!(
            one("fn say_hi() { message(\"hi\"); }"),
            "(defun say-hi () (message \"hi\"))"
        );
    }

    #[test]
    fn lowers_fn_with_params_and_arithmetic() {
        assert_eq!(
            one("fn add_two(a: i64, b: i64) -> i64 { a + b }"),
            "(defun add-two (a b) (+ a b))"
        );
    }

    #[test]
    fn lowers_doc_comment_to_docstring() {
        let out = one("/// Greet the user.\nfn greet() { message(\"hi\"); }");
        assert!(out.contains("\"Greet the user.\""), "{out}");
    }

    #[test]
    fn lowers_interactive_attribute() {
        let out = one("#[interactive]\nfn cmd() { message(\"hi\"); }");
        assert!(out.contains("(interactive)"), "{out}");
    }

    #[test]
    fn lowers_interactive_spec_attribute() {
        let out = one("#[interactive(\"p\")]\nfn cmd(n: i64) -> i64 { n }");
        assert!(out.contains("(interactive \"p\")"), "{out}");
    }

    #[test]
    fn lowers_const_to_defconst() {
        assert_eq!(one("const MAX: i64 = 200;"), "(defconst max 200)");
    }

    #[test]
    fn lowers_static_to_defvar() {
        assert_eq!(one("static FLAG: bool = true;"), "(defvar flag t)");
    }

    // --- Statements ----------------------------------------------------------

    #[test]
    fn lowers_let_to_let_star() {
        assert_eq!(
            one("fn f() -> i64 { let x = 1; x }"),
            "(defun f () (let* ((x 1)) x))"
        );
    }

    #[test]
    fn lowers_typed_let() {
        assert_eq!(
            one("fn f() -> i64 { let x: i64 = 2; x }"),
            "(defun f () (let* ((x 2)) x))"
        );
    }

    #[test]
    fn defun_body_splices_top_level_sequence() {
        // A defun body is an implicit progn, so the top-level sequence is
        // spliced flat rather than wrapped in an explicit `progn`.
        assert_eq!(
            one("fn f() { message(\"a\"); message(\"b\"); }"),
            "(defun f () (message \"a\") (message \"b\"))"
        );
    }

    #[test]
    fn single_expression_position_uses_progn() {
        // The then-branch of an `if`/`else` takes a single expression, so a
        // multi-statement branch must become an explicit `progn`.
        let out = one("fn f() -> i64 { if true { message(\"a\"); 1 } else { 2 } }");
        assert!(out.contains("(if t (progn"), "{out}");
    }

    #[test]
    fn when_body_splices_flat() {
        // `when` is itself an implicit progn, so a multi-statement body is
        // spliced flat rather than double-wrapped.
        assert_eq!(
            one("fn f() { if true { message(\"a\"); message(\"b\"); } }"),
            "(defun f () (when t (message \"a\") (message \"b\")))"
        );
    }

    #[test]
    fn lowers_assignment_to_setq() {
        let out = one("fn f() { x = 3; }");
        assert!(out.contains("(setq x 3)"), "{out}");
    }

    #[test]
    fn lowers_tail_return() {
        assert_eq!(one("fn f() -> i64 { return 7; }"), "(defun f () 7)");
    }

    // --- Expressions ---------------------------------------------------------

    #[test]
    fn lowers_comparisons() {
        assert_eq!(one("const A: bool = 1 == 2;"), "(defconst a (equal 1 2))");
        assert_eq!(
            one("const A: bool = 1 != 2;"),
            "(defconst a (not (equal 1 2)))"
        );
        assert_eq!(one("const A: bool = 1 < 2;"), "(defconst a (< 1 2))");
        assert_eq!(one("const A: bool = 1 <= 2;"), "(defconst a (<= 1 2))");
        assert_eq!(one("const A: bool = 1 > 2;"), "(defconst a (> 1 2))");
        assert_eq!(one("const A: bool = 1 >= 2;"), "(defconst a (>= 1 2))");
    }

    #[test]
    fn lowers_logic() {
        assert_eq!(
            one("const A: bool = true && false;"),
            "(defconst a (and t nil))"
        );
        assert_eq!(
            one("const A: bool = true || false;"),
            "(defconst a (or t nil))"
        );
        assert_eq!(one("const A: bool = !true;"), "(defconst a (not t))");
    }

    #[test]
    fn lowers_arithmetic_ops() {
        assert_eq!(one("const A: i64 = 6 % 4;"), "(defconst a (% 6 4))");
        assert_eq!(one("const A: i64 = 6 / 4;"), "(defconst a (/ 6 4))");
    }

    #[test]
    fn lowers_method_call() {
        assert_eq!(
            one("fn f() { thing.do_it(1); }"),
            "(defun f () (do-it thing 1))"
        );
    }

    #[test]
    fn lowers_if_with_else() {
        let out = one("fn f() -> i64 { if true { 1 } else { 2 } }");
        assert!(out.contains("(if t"), "{out}");
    }

    #[test]
    fn lowers_if_without_else_to_when() {
        let out = one("fn f() { if true { message(\"x\"); } }");
        assert!(out.contains("(when t"), "{out}");
    }

    #[test]
    fn lowers_while() {
        let out = one("fn f() { while true { message(\"x\"); } }");
        assert!(out.contains("(while t"), "{out}");
    }

    #[test]
    fn lowers_for_to_dolist() {
        let out = one("fn f() { for item in items { use_it(item); } }");
        assert!(out.contains("(dolist (item items)"), "{out}");
    }

    #[test]
    fn lowers_for_range_to_dotimes() {
        let out = one("fn f() { for i in 0..3 { use_it(i); } }");
        assert!(out.contains("(dotimes (i 3)"), "{out}");
    }

    #[test]
    fn lowers_closure_to_lambda() {
        assert_eq!(
            one("fn f() { run(|a, b| add(a, b)); }"),
            "(defun f () (run (lambda (a b) (add a b))))"
        );
    }

    #[test]
    fn lowers_match_to_pcase() {
        let out = one("fn f(x: i64) -> i64 { match x { 1 => 10, _ => 0 } }");
        assert!(out.contains("(pcase x"), "{out}");
        assert!(out.contains("(1 10)"), "{out}");
        assert!(out.contains("(_ 0)"), "{out}");
    }

    #[test]
    fn lowers_match_or_pattern_and_guard() {
        let out = one("fn f(x: i64) -> i64 { match x { 1 | 2 => 10, n if n > 5 => 1, _ => 0 } }");
        assert!(out.contains("(or 1 2)"), "{out}");
        assert!(out.contains("(guard (> n 5))"), "{out}");
    }

    // --- FFI / intrinsics ----------------------------------------------------

    #[test]
    fn maps_calls_to_kebab_case() {
        assert_eq!(
            one("fn f() { expand_file_name(\"~/x\"); }"),
            "(defun f () (expand-file-name \"~/x\"))"
        );
    }

    #[test]
    fn elisp_attribute_overrides_name() {
        let src = "#[elisp(name = \"1+\")]\nfn inc(n: i64) -> i64 { n }\n\
                   fn f(n: i64) -> i64 { inc(n) }";
        let forms = transpile_str(src).unwrap();
        // The declaration emits nothing; only `f` is emitted.
        assert_eq!(forms.len(), 1);
        assert!(
            forms[0].render().contains("(1+ n)"),
            "{}",
            forms[0].render()
        );
    }

    #[test]
    fn intrinsic_sym_lowers_to_quote() {
        assert_eq!(
            one("fn f() { add_to_list(sym(\"my-list\"), 1); }"),
            "(defun f () (add-to-list 'my-list 1))"
        );
    }

    #[test]
    fn intrinsic_func_lowers_to_function() {
        assert_eq!(
            one("fn f() { add_hook(sym(\"h\"), func(\"g\")); }"),
            "(defun f () (add-hook 'h #'g))"
        );
    }

    #[test]
    fn intrinsic_kbd_lowers_to_kbd_call() {
        let out = one("fn f() { global_set_key(kbd(\"C-c h\"), func(\"cmd\")); }");
        assert!(out.contains("(kbd \"C-c h\")"), "{out}");
    }

    #[test]
    fn intrinsic_raw_lowers_verbatim() {
        assert_eq!(
            one("fn f() { raw(\"(message (quote literal))\"); }"),
            "(defun f () (message (quote literal)))"
        );
    }

    #[test]
    fn format_macro_lowers_to_format() {
        assert_eq!(
            one("fn f() -> i64 { format!(\"%d\", 1) }"),
            "(defun f () (format \"%d\" 1))"
        );
    }

    #[test]
    fn println_macro_lowers_to_message() {
        assert_eq!(
            one("fn f() { println!(\"%s\", x); }"),
            "(defun f () (message \"%s\" x))"
        );
    }

    // --- Refusals ------------------------------------------------------------

    fn assert_located(err: &TranspileError, needle: &str) {
        assert!(
            err.message().contains(needle),
            "message `{}` should contain `{needle}`",
            err.message()
        );
        assert!(err.line() >= 1, "error should carry a real line: {err}");
    }

    #[test]
    fn refuses_question_mark() {
        let err = refuse("fn f() -> i64 { g()? }");
        assert_located(&err, "unsupported");
        assert_located(&err, "?");
    }

    #[test]
    fn refuses_await() {
        let err = refuse("fn f() -> i64 { g().await }");
        assert_located(&err, ".await");
    }

    #[test]
    fn refuses_struct_literal() {
        let err = refuse("fn f() { let _x = Point { x: 1 }; }");
        assert_located(&err, "struct");
    }

    #[test]
    fn refuses_field_access() {
        let err = refuse("fn f() -> i64 { p.x }");
        assert_located(&err, "field");
    }

    #[test]
    fn refuses_generic_fn() {
        let err = refuse("fn f<T>(x: T) -> T { x }");
        assert_located(&err, "generic");
    }

    #[test]
    fn refuses_async_fn() {
        let err = refuse("async fn f() {}");
        assert_located(&err, "async");
    }

    #[test]
    fn refuses_cast() {
        let err = refuse("const A: i64 = 1.5 as i64;");
        assert_located(&err, "cast");
    }

    #[test]
    fn refuses_range_outside_for() {
        let err = refuse("fn f() { let _r = 0..3; }");
        assert_located(&err, "range");
    }

    #[test]
    fn refuses_inclusive_range_in_for() {
        let err = refuse("fn f() { for i in 0..=3 { use_it(i); } }");
        assert_located(&err, "inclusive");
    }

    #[test]
    fn refuses_nonzero_range_in_for() {
        let err = refuse("fn f() { for i in 1..3 { use_it(i); } }");
        assert_located(&err, "start at 0");
    }

    #[test]
    fn refuses_path_pattern_in_match() {
        let err = refuse("fn f(x: i64) -> i64 { match x { None => 0, _ => 1 } }");
        assert_located(&err, "path pattern");
    }

    #[test]
    fn refuses_struct_item() {
        let err = refuse("struct S { x: i64 }");
        assert_located(&err, "unsupported item");
    }

    #[test]
    fn refuses_let_destructuring() {
        let err = refuse("fn f() { let (a, b) = pair(); }");
        assert_located(&err, "destructuring");
    }

    #[test]
    fn refuses_unknown_macro() {
        let err = refuse("fn f() { vec![1, 2]; }");
        assert_located(&err, "unsupported macro");
    }

    #[test]
    fn refuses_invalid_rust() {
        let err = transpile_str("fn f( {").unwrap_err();
        assert!(err.message().contains("parse error"), "{err}");
    }
}
