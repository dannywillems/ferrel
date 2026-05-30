# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Initial `ferrel` crate: a typed Rust eDSL that emits Emacs Lisp.
- `Sexp` AST and a width-aware pretty-printer with Lisp indentation and
  plist-pair alignment.
- Typed expression layer `El<T>` with phantom type markers (`Int`, `Float`,
  `Str`, `Bool`, `Symbol`, `List`, `Any`).
- Typed builtins: arithmetic, comparison, strings, `message`/`format`,
  common editor commands, and control flow (`if_`, `when`, `unless`, `progn`).
- The `call` escape hatch for untyped third-party Elisp functions.
- Top-level forms: `Defun` builder, `defvar`, `defconst`, `defcustom`,
  `require`, `setq`, `add_hook`, `global_set_key`.
- `Package` builder that renders a complete, loadable `.el` file with the
  `lexical-binding` header, `Commentary`/`Code` sections, and `provide` footer.
- `examples/hello.rs` generating a verified plugin that byte-compiles cleanly
  and runs in batch Emacs.
- Read side of the pipeline: a `lexer` (text to spanned tokens, with strings,
  chars, `#x`/`#o`/`#b`/`#NrDIGITS` radixes, reader-macro prefixes) and a
  `parser`/reader (`parse`, `parse_one`) producing the same `Sexp` AST the code
  generator renders. Verified to round-trip a real modular Emacs config.
- `Sexp` reader-macro variants: `Backquote`, `Unquote`, `Splice`, and the
  improper-list `Dotted`, so real Elisp survives a parse/render round-trip.
- `examples/editing.rs` porting a real hand-written `editing.el` module, and
  `examples/roundtrip.rs` round-tripping arbitrary `.el` files.
- Heterogeneous statement bodies via `Stmt` and the `stmts!` macro; the typed
  escape hatch accepts typed values through the `IntoSexp` trait and `args!`
  macro; `call_typed` for inline return-type assertions.
- Typed key sequences: `KeySeq` (checked `parse`), the `Command` newtype,
  `bind_key`, and the `kbd!` macro.
- `CustomType` enum and the `Defcustom` builder (with `:type` inference);
  `Package::push_form` / `extend_forms`; literal `From` conversions for `El<T>`.
- Rust-to-Elisp transpiler: a third front-end that lowers a defined subset of
  real, type-checked Rust (parsed with `syn`) into the `Sexp` IR. Covers `fn`,
  `const`/`static`, `let`, assignment, `if`/`match` (`pcase` with or-patterns
  and guards), `when`, `while`, `for`/`dolist`/`dotimes`, closures, operators,
  calls, and `format!`/`println!`. Anything outside the subset is refused with a
  `line:col` `TranspileError` rather than mistranslated. Public API
  `transpile_str` / `transpile_file`.
- Typed foreign-function interface for the transpiler: `#[elisp]` declarations
  (with `#[elisp(name = "...")]` overrides) and the `sym`/`func`/`kbd`/`raw`
  intrinsics, backed by the safe-stub `ferrel::rt` prelude so transpiled code
  compiles as ordinary Rust.
- `ferrel-transpile` CLI (`.rs` in, `.el` out) with `--byte-compile` and
  `--native-compile` flags that drive Emacs; the in-repo `ferrel-macros`
  proc-macro crate provides the inert `#[elisp]`/`#[interactive]` attributes
  (the crate is now a workspace).
- Transpiler regression suite under `tests/regression/` (grouped as the
  `regression` test crate): a readable table of Rust inputs and the exact Elisp
  they lower to, plus refusal cases that pin the `line:col` errors for
  unsupported syntax.
- `examples/config_git.rs`: a step-by-step port of a real `git.el` module to the
  typed `UsePackage` builder, with the parts the builder does not model yet
  flagged inline.
- MELPA corpus tooling: `scripts/fetch-melpa-corpus.sh` and the `corpus` example
  parse random real packages (never evaluating them) to surface parser gaps,
  wired into a scheduled CI job that files an issue on failure.
- Reusable agents under `.claude/agents/` (`ferrel-compiler-engineer`,
  `ferrel-rust-api-reviewer`) encoding the maintainer/reviewer split.
- Docusaurus documentation site under `doc/`, including a graduate-level Emacs
  Lisp course (history, syntax, semantics, loading, the `.elc` format) and a
  compiler-internals section.
- GitHub Actions CI: Rust (fmt, clippy, test, build), generated-Elisp
  byte-compilation in Emacs, changelog and PR-hygiene checks, shellcheck, and a
  documentation build/deploy workflow.
