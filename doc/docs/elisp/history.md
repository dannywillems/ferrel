---
sidebar_position: 1
---

# History and evolution of Emacs Lisp

Emacs Lisp (Elisp) is the extension language of GNU Emacs: a dynamically typed
Lisp dialect, first shipped with GNU Emacs in 1985, in which most of Emacs above
the C core is written. This page traces where it came from and how it has
changed, because several of its design choices only make sense in light of that
history.

## Before Emacs Lisp: TECO and the first Emacs

The original Emacs (1976), written by Richard Stallman with Guy Steele and
others, was not Lisp at all. It was a set of editing macros for TECO, a terse
text editor on the MIT ITS system. The name is an acronym: Editor MACroS. TECO
macros were powerful but unreadable, and extending the editor meant writing in
that cryptic macro language.

The pivotal influence was Multics Emacs (1978) by Bernard Greenberg, which
reimplemented the Emacs idea on top of Maclisp. It showed that a real
programming language, specifically a Lisp, made an editor far more extensible:
users could write commands in the same language the editor itself used. That
experience directly shaped the decision, when Stallman built GNU Emacs, to use a
Lisp as the extension language.

## GNU Emacs and the birth of Elisp (1985)

GNU Emacs, released in 1985, is structured as a small C core (the reader, the
evaluator, the redisplay engine, buffer and process primitives) with the large
majority of editor behavior written in Emacs Lisp on top. Elisp is its own
dialect. It was influenced by Maclisp but is not Common Lisp and not Scheme.

Two early design decisions still define the language:

- It is a **Lisp-2**: a symbol has a separate value cell and function cell, so a
  variable and a function can share a name. This is why `#'` (function quote)
  and `funcall` exist.
- It used **dynamic scoping** by default. Stallman defended this for an
  extension language: it let users rebind a package's internal variables around
  a call without that package's cooperation, which is convenient for an editor
  even though it complicates reasoning and prevents true closures.

## The long middle: versions 18 to 23

Through the late 1980s and the 1990s, Emacs Lisp accumulated the machinery a
serious editor needs while the language stayed largely stable:

- Emacs 19 (1994) brought a real X11 GUI, faces, and overlays.
- Emacs 20 (1997) added multibyte character support (MULE) and the Customize
  system for user options (`defcustom`).
- Emacs 21 (2001) introduced a rewritten redisplay engine with images and
  variable-width fonts.
- Emacs 23 (2009) moved to Unicode as the internal character representation.

Throughout this period, `cl.el` (later `cl-lib`) offered Common Lisp style
conveniences (`cl-loop`, `cl-defun`, destructuring) as a library, without
changing the base language.

## The modern era: lexical binding and packages (Emacs 24, 2012)

Emacs 24 is the most consequential release for the language itself. It added
**optional lexical scoping**, enabled per file with a local variable on the
first line:

```elisp
;;; my-file.el --- summary -*- lexical-binding: t; -*-
```

With this cookie, `let` bindings and function parameters are lexically scoped and
`lambda` produces real closures. Without it, the old dynamic scoping applies.
The default remained `nil` for backward compatibility, but essentially all new
code, and everything ferrel generates, sets it to `t`. Emacs 24 also integrated
`package.el`, giving Emacs a built-in package manager and the ELPA and MELPA
archives.

## Reaching out to other languages (Emacs 25, 2016)

Emacs 25 added **dynamic modules**: a C ABI (`emacs-module.h`) that lets a
compiled shared library expose functions to Elisp. This is the route the
Rust [`emacs`](https://crates.io/crates/emacs) crate takes to write native Emacs
extensions. It is a different strategy from ferrel, which generates `.el` rather
than a compiled module.

## Speed and structure (Emacs 26 to 28)

- Emacs 26 (2018) added `display-line-numbers-mode` and limited Lisp-level
  concurrency (cooperative threads).
- Emacs 27 (2020) added native JSON parsing, HarfBuzz text shaping,
  arbitrary-precision integers (bignums), and `early-init.el`.
- Emacs 28 (2022) added **native compilation**: with libgccjit, Elisp can be
  compiled through GCC to native code (`.eln` files), on top of the existing
  byte-compiler. See [the .elc format](./elc-format.md) for how this layers on
  the byte-code.

## Built-in tooling (Emacs 29, 2023)

Emacs 29 folded several long-standing external packages into core: a built-in
**tree-sitter** interface for fast, precise syntax parsing (`*-ts-mode`), the
`eglot` Language Server Protocol client, an SQLite interface, and the
`use-package` configuration macro. It also added a pure GTK build that runs
natively under Wayland.

## Why this matters for ferrel

The history explains the shape of the target language ferrel emits:

- Generated files carry `lexical-binding: t`, because lexical scoping is the
  modern default and the only sane base for reasoning about generated code.
- The Lisp-2 design means ferrel must distinguish a function reference (`#'f`)
  from a variable reference (`f`), which it models as distinct AST nodes.
- `use-package` is a recent, central idiom, which is why ferrel has a dedicated
  typed builder for it rather than treating it as an ordinary call.

## Frequently asked questions

### Is Emacs Lisp the same as Common Lisp or Scheme?

No. It is its own dialect. It shares the Lisp family's parenthesized syntax and
symbolic data, but it has its own standard library, its own object system
conventions, and a Lisp-2 design. `cl-lib` provides Common-Lisp-style utilities
as a compatibility layer, not a different language.

### Why did Emacs Lisp use dynamic scoping for so long?

Dynamic scoping was a deliberate choice for an extension language: it lets users
rebind another package's variables around a call without that package exposing
them. Lexical scoping (Emacs 24+) is now preferred because it enables closures,
better compilation, and clearer reasoning, but the dynamic default persisted for
decades of backward compatibility.
