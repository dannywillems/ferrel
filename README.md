# ferrel

A typed Rust eDSL that generates Emacs Lisp (`.el`) plugins and configuration.

ferrel lets you write Emacs extensions and your `init.el` in Rust, with a type
checker, and emit ordinary `.el` files that Emacs loads exactly like
hand-written Elisp. There is no runtime, no native module, and nothing added to
your Emacs: the output is plain Elisp you can read, diff, and commit.

## Why

- You know Rust and its type system, and you would rather not hand-write
  parenthesis-dense, dynamically typed Elisp.
- You want the Rust compiler to reject mistakes such as passing a string where
  an integer is expected, before any `.el` is written.
- You want generated output that is readable Elisp, not an opaque blob, so you
  can still inspect what Emacs actually runs.

ferrel is also meant to be educational. The documentation site under `doc/`
teaches each Emacs plugin pattern twice: once in idiomatic Elisp, and once in
ferrel, side by side, so you learn how Emacs extension works while writing it
in a language you already know.

## Quick start

```rust
use ferrel::*;

fn main() -> std::io::Result<()> {
    let pkg = Package::new("ferrel-hello", "A greeting command.")
        .emacs_version("27.1")
        .defun(
            Defun::new("ferrel-hello")
                .doc("Greet the user.")
                .interactive()
                .body([message(string("Hello from Rust-generated Elisp!"), [])]),
        )
        .form(global_set_key("C-c h", "ferrel-hello"));

    pkg.write("ferrel-hello.el")
}
```

This generates:

```elisp
;;; ferrel-hello.el --- A greeting command. -*- lexical-binding: t; -*-

;; Package-Requires: ((emacs "27.1"))

;;; Commentary:
;; A greeting command.

;;; Code:

(defun ferrel-hello ()
  "Greet the user."
  (interactive)
  (message "Hello from Rust-generated Elisp!"))

(global-set-key (kbd "C-c h") #'ferrel-hello)

(provide 'ferrel-hello)
;;; ferrel-hello.el ends here
```

## What ferrel types, and what it does not

Emacs has thousands of functions, and every package adds more untyped ones.
ferrel types the core you reach for constantly (arithmetic, strings, control
flow, common builtins) and provides `call` as an explicit, ergonomic escape
hatch for everything else:

```rust
// (projectile-project-root) returns a path; assert the type you expect.
let root: El<Str> = call("projectile-project-root", []).cast();
```

The escape hatch is a feature: it keeps you productive immediately instead of
waiting for full coverage of an API surface that will never be complete.

## Commands

```
make build          # compile the crate
make test           # run tests
make example        # generate examples/out/ferrel-hello.el
make verify-elisp   # byte-compile and run the generated plugin in Emacs
make lint           # clippy
make check-format   # rustfmt --check
make doc-install    # install the documentation site dependencies
make doc-dev        # run the documentation site locally
```

## Status

Early. The core (AST, renderer, typed layer, package writer) works and is
verified against batch Emacs. The typed builtin surface is intentionally small
and grows as real configuration is ported.

## License

MIT OR Apache-2.0.
