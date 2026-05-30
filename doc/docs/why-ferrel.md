---
sidebar_position: 1
---

# Why ferrel

ferrel exists to let you write Emacs plugins and configuration in Rust, a typed
language, and generate the Emacs Lisp that Emacs runs. This page explains the
motivation, the design choice to emit `.el`, and the trade-offs.

## The problem

Emacs is configured and extended in Emacs Lisp. Elisp is a dynamically typed
Lisp with no static type checking, and its syntax is parenthesis-dense. Two
common difficulties follow:

- Readability. Nested parentheses make it harder for some people to read a
  configuration at a glance, especially when coming from languages with
  infix syntax and explicit types.
- No types. Mistakes such as passing a string where an integer is expected are
  not caught until the code runs, if at all.

Many developers are comfortable in Rust and rely on its type system. For them,
writing Emacs configuration in Rust and generating Elisp is one way to keep a
familiar language and a compile-time checker.

## The approach: emit `.el`

ferrel generates Emacs Lisp source files. It does not interpret Elisp, embed a
Lisp runtime, or compile to a native module.

This choice has concrete consequences:

- **Full compatibility.** The output is ordinary Elisp. Emacs loads it exactly
  like a hand-written package, including byte-compilation.
- **Inspectable.** You can open the generated `.el` and read what Emacs runs.
  This matters when debugging or when you do not fully trust the generator yet.
- **No added dependency.** Emacs needs nothing extra installed. There is no
  ferrel runtime in your Emacs.
- **Incremental adoption.** You can generate one file, drop it next to your
  hand-written Elisp, and migrate the rest over time.

A different approach, writing a Rust dynamic module with the
[`emacs`](https://crates.io/crates/emacs) crate, produces a compiled `.so` that
Emacs loads. That is useful for performance-sensitive native code, but it does
not make configuration more readable and it adds a build and loading step. For
the goal of writing readable, typed configuration, generating `.el` is the more
direct fit.

## What ferrel types

Emacs has thousands of functions, and every third-party package adds more.
ferrel does not attempt to type all of Emacs. It types the core that
configuration touches constantly:

- Arithmetic and comparison over integers.
- String construction (`concat`, `format`, `message`).
- Control flow (`if`, `when`, `unless`, `progn`).
- Variable binding (`setq`, `defvar`, `defcustom`).
- A small set of common editor builtins.

For everything else, including every package-specific function, ferrel provides
an explicit escape hatch:

```rust
// (projectile-project-root) returns a path string.
let root: El<Str> = call("projectile-project-root", []).cast();
```

`call` builds an untyped function application; `cast` lets you assert the result
type you expect. This keeps you productive immediately instead of waiting for a
complete type model of an API surface that keeps growing.

## Trade-offs

ferrel is one approach with clear costs as well as benefits:

- You write Rust and run a generation step, rather than editing Elisp directly.
- The type guarantees stop at the boundary of what ferrel models. Calls through
  the escape hatch are as untyped as the Elisp they produce.
- The generated Elisp is only as idiomatic as the generator. ferrel aims for
  readable output, but a seasoned Elisp author may write some forms differently.

## Frequently asked questions

### Why not write a full Rust-to-Elisp transpiler?

A transpiler that accepts arbitrary Rust would need a complete parser and type
checker and would still have to map Rust semantics onto Elisp. An embedded DSL
reuses the Rust compiler directly and ships value sooner. A transpiler frontend
remains possible later on top of the same AST.

### Will ferrel support Vim or Neovim?

The internal AST is the target-independent part. Emitting Vimscript or Lua would
mean adding another renderer. Emacs Lisp is the first and current target.
