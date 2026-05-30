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
- Docusaurus documentation site scaffold under `doc/`.
- GitHub Actions CI: Rust (fmt, clippy, test, build), generated-Elisp
  byte-compilation in Emacs, changelog and PR-hygiene checks, shellcheck, and a
  documentation build/deploy workflow.
