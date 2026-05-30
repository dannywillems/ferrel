---
sidebar_position: 0
---

# ferrel

ferrel is a typed Rust eDSL that generates Emacs Lisp (`.el`) files. You write
Emacs plugins and configuration in Rust, the Rust compiler type-checks them, and
ferrel emits ordinary Elisp that Emacs loads like any hand-written package.

There is no runtime and no native module. The output is plain `.el` text you can
read, diff, and commit.

## Who this is for

- People who know Rust and its type system and would rather not hand-write
  parenthesis-dense, dynamically typed Elisp.
- People who want to rewrite an `init.el` in a typed language while still
  producing standard Elisp that Emacs runs unchanged.
- People who want to learn how Emacs plugins work by writing them twice: once in
  Elisp, once in Rust.

## What ferrel does in one example

```rust
use ferrel::*;

let pkg = Package::new("ferrel-hello", "A greeting command.")
    .emacs_version("27.1")
    .defun(
        Defun::new("ferrel-hello")
            .doc("Greet the user.")
            .interactive()
            .body([message(string("Hello from Rust-generated Elisp!"), [])]),
    )
    .form(global_set_key("C-c h", "ferrel-hello"));
```

Generates:

```lisp
(defun ferrel-hello ()
  "Greet the user."
  (interactive)
  (message "Hello from Rust-generated Elisp!"))

(global-set-key (kbd "C-c h") #'ferrel-hello)
```

## How this site is organized

This documentation teaches each Emacs extension pattern twice, side by side: the
idiomatic Elisp version and the ferrel version. Start with
[Why ferrel](./why-ferrel.md) for the motivation, then
[Emacs Lisp basics](./elisp-basics.md) for the underlying concepts, then the
[tutorials](./tutorials/hello-plugin.md).

## Frequently asked questions

### Does ferrel replace Emacs Lisp?

No. ferrel generates Emacs Lisp. The Elisp it produces is what Emacs actually
runs, and you can read and edit it directly if you ever need to.

### Do I need to install anything into Emacs?

No. The generated `.el` is a normal file. Emacs needs no plugin, no runtime, and
no native module to load it.

### Can ferrel call functions from third-party packages?

Yes, through the `call` escape hatch. ferrel types the common core of Emacs and
lets you call anything else by name. See
[Why ferrel](./why-ferrel.md#what-ferrel-types).
