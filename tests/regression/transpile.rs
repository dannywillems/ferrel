//! Regression tests for the Rust-to-Elisp transpiler.
//!
//! Each case pairs a concrete Rust input with the exact Elisp the transpiler
//! produces, so the table doubles as readable documentation of what the subset
//! lowers to. The refusal cases pin the located errors for unsupported syntax.
//!
//! Outputs here were captured from the transpiler and are asserted verbatim; if
//! a lowering rule changes on purpose, update the expected string in the same
//! commit.

use ferrel::{Sexp, transpile_str};

/// Transpile `src` and render all top-level forms joined by blank-free newlines.
fn elisp(src: &str) -> String {
    let forms = transpile_str(src).expect("expected successful transpile");
    forms
        .iter()
        .map(Sexp::render)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Single-line lowerings: (name, rust input, expected elisp).
const CASES: &[(&str, &str, &str)] = &[
    (
        "arithmetic",
        "fn add(a: i64, b: i64) -> i64 { a + b }",
        "(defun add (a b) (+ a b))",
    ),
    (
        "interactive command with a message",
        "#[interactive]\nfn hi() { message!(\"hello\"); }",
        "(defun hi () (interactive) (message \"hello\"))",
    ),
    (
        "if / else",
        "fn sign(n: i64) -> i64 { if n < 0 { -1 } else { 1 } }",
        "(defun sign (n) (if (< n 0) (- 1) 1))",
    ),
    (
        "mutable local and assignment lower to let* and setq",
        "fn run(n: i64) -> i64 { let mut total = 0; total = total + n; total }",
        "(defun run (n) (let* ((total 0)) (setq total (+ total n)) total))",
    ),
    (
        "for over a sequence lowers to dolist",
        "fn each(xs: i64) { for x in xs { insert(x); } }",
        "(defun each (xs) (dolist (x xs) (insert x)))",
    ),
    (
        "while loop",
        "fn countdown(mut n: i64) { while n > 0 { n = n - 1; } }",
        "(defun countdown (n) (while (> n 0) (setq n (- n 1))))",
    ),
    (
        "FFI name override maps inc to the elisp 1+",
        "#[elisp(name = \"1+\")]\nfn inc(n: i64) -> i64 { unreachable!() }\nfn bump(n: i64) -> i64 { inc(n) }",
        "(defun bump (n) (1+ n))",
    ),
    (
        "logical and comparison operators",
        "fn ok(a: i64, b: i64) -> i64 { if a > 0 && !(b == 0) { a } else { b } }",
        "(defun ok (a b) (if (and (> a 0) (not (equal b 0))) a b))",
    ),
];

#[test]
fn single_line_lowerings() {
    for (name, src, expected) in CASES {
        assert_eq!(&elisp(src), expected, "case `{name}`");
    }
}

#[test]
fn match_lowers_to_pcase_with_or_patterns_and_guards() {
    let src = "\
fn label(n: i64) {
  match n {
    0 => message!(\"zero\"),
    1 | 2 => message!(\"small\"),
    m if m > 10 => message!(\"big\"),
    _ => message!(\"other\"),
  }
}";
    let expected = "\
(defun label (n)
  (pcase n
         (0 (message \"zero\"))
         ((or 1 2) (message \"small\"))
         ((and m (guard (> m 10))) (message \"big\"))
         (_ (message \"other\"))))";
    assert_eq!(elisp(src), expected);
}

#[test]
fn for_range_lowers_to_dotimes() {
    let src = "fn blanks(n: i64) { for _i in 0..n { insert(\"\\n\"); } }";
    // The inserted string contains a real newline.
    let expected = "(defun blanks (n)\n  (dotimes (_i n)\n    (insert \"\n\")))";
    assert_eq!(elisp(src), expected);
}

#[test]
fn intrinsics_lower_to_reader_macros() {
    let src = "\
fn setup() {
  add_hook(sym(\"prog-mode-hook\"), func(\"my-fn\"));
  global_set_key(kbd(\"C-c h\"), func(\"my-cmd\"));
  raw(\"(message \\\"raw escape\\\")\");
}";
    let expected = "\
(defun setup ()
  (add-hook 'prog-mode-hook #'my-fn)
  (global-set-key (kbd \"C-c h\") #'my-cmd)
  (message \"raw escape\"))";
    assert_eq!(elisp(src), expected);
}

#[test]
fn refuses_question_mark_operator_with_location() {
    let err = transpile_str("fn f(x: i64) { x?; }").unwrap_err();
    assert!(err.message().contains('?'), "message: {}", err.message());
    assert_eq!((err.line(), err.col()), (1, 15));
}

#[test]
fn refuses_field_access_with_location() {
    let err = transpile_str("fn f(p: i64) -> i64 { p.x }").unwrap_err();
    assert!(
        err.message().contains("field access"),
        "message: {}",
        err.message()
    );
    assert_eq!((err.line(), err.col()), (1, 22));
}
