---
sidebar_position: 1
---

# Architecture: one IR, many front-ends

ferrel is a small but real compiler, and it is organized around a single idea:
one intermediate representation in the middle, with several front-ends above it
and one back-end below. This page explains that shape, why it is the load-bearing
decision, and the general principles it borrows from larger compilers.

## The narrow waist

A compiler is conventionally split into a front-end (understand the input), a
middle (a neutral representation), and a back-end (produce the output). The piece
that makes the parts independent is the **intermediate representation**, the IR.
ferrel's IR is the `Sexp` type, the Emacs Lisp abstract syntax tree.

```
Elisp text          --lexer--> tokens --parser--> Sexp
Rust eDSL (El<T>)   --lower-->                     Sexp   --codegen--> .el text
Rust source subset  --syn-----> Rust AST --lower-->Sexp
```

Everything above the waist lowers into `Sexp`; the one thing below the waist, the
pretty-printer, renders `Sexp` back to text. The payoff is multiplicative: add a
front-end and every back-end benefits; add a back-end and every front-end
benefits. This "hourglass" or "narrow waist" shape is why LLVM IR and
WebAssembly matter at industrial scale, and it is why ferrel's Rust transpiler is
cheap to add: it only has to reach the waist, not the bottom.

## The pieces

- **`Sexp`** (the IR): a small enum modeling Elisp surface syntax, with a
  width-aware pretty-printer as the back-end.
- **Lexer and parser** (a front-end): turn `.el` text into `Sexp`. Covered in
  [parsing](./parsing.md).
- **The typed eDSL** (a front-end): `El<T>` and the builders, where the Rust
  type system checks the program as you build it.
- **The transpiler** (a front-end): `syn` parses a Rust subset, which is lowered
  into `Sexp`. Covered in [the transpiler](./transpiler.md).

## Why the IR is concrete, not abstract

There is a spectrum from a concrete syntax tree, which preserves every token, to
an abstract syntax tree, which keeps only meaning. ferrel's `Sexp` sits toward
the concrete end on purpose: it keeps reader macros (quote, backquote, unquote)
and dotted pairs as real nodes instead of desugaring them. The reason is a
correctness property: ferrel must be able to read real Elisp and render it back
faithfully. Keeping structure is a deliberate lever; you keep what you must
reproduce and discard what you only need to interpret.

## Reuse the platform back-end

ferrel stops at Elisp source and does not emit byte-code or native code. That is
a specific instance of a general compiler strategy: **target a higher-level
language and reuse the platform's mature back-end.** Many production compilers
emit C or LLVM IR rather than machine code, because a correct, optimizing,
well-maintained back-end already exists and is expensive to reproduce. For ferrel
the platform back-end is Emacs itself: its byte-compiler and native compiler turn
the generated `.el` into version-correct `.elc` and `.eln`. Emitting source and
deferring compilation is both less code and more correct than writing a
byte-code back-end, as discussed in
[the .elc chapter](../elisp/elc-format.md).

## What this buys the roadmap

Because the IR is the contract, future work slots in without disturbing the rest:
a Vimscript or Lua back-end would render `Sexp`-like trees for other editors; a
new front-end (a different surface syntax) only needs to reach `Sexp`. The
architecture is what keeps each addition a single, well-scoped change rather than
a rewrite.
