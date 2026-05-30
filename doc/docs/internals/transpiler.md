---
sidebar_position: 3
---

# Lowering, desugaring, and the transpiler

The transpiler is the front-end that reads a subset of real Rust and produces
Emacs Lisp. As a compiler component it is a **lowering pass**: it rewrites a
higher-level tree (the Rust AST from `syn`) into the lower-level `Sexp` IR, which
the existing back-end then renders. This page explains lowering, the desugaring
rules, the typed foreign-function interface for calling Elisp, and the design
rule that keeps it honest.

## Lowering is structured rewriting

Lowering walks the source tree and emits target forms, translating each
high-level construct into simpler ones. The core rules are direct because Rust
and Elisp share more structure than they appear to:

```
fn add(a, b) { a + b }      ->  (defun add (a b) (+ a b))
let x = 1; rest             ->  (let* ((x 1)) rest)
if c { a } else { b }       ->  (if c a b)
while c { body }            ->  (while c body)
for x in xs { body }        ->  (dolist (x xs) body)
match v { ... }             ->  (pcase v ...)
{ s1; s2; tail }            ->  (progn s1 s2 tail)
a && b   a || b   !a        ->  (and a b)  (or a b)  (not a)
```

One semantic match makes the whole thing pleasant: a Rust block returns its
trailing expression, and an Elisp body returns its last form, so functions and
blocks lower without any return-value bookkeeping.

## Desugaring

Some constructs have no direct target and must be rewritten into a combination of
simpler ones, which is called desugaring. A `for` loop desugars to `dolist`; a
multi-arm `match` desugars to `pcase` or a `cond`; a compound assignment may
desugar into a read, an operation, and a `setq`. Desugaring is where a transpiler
earns its keep, and where it is easiest to introduce bugs, so each rule is small,
explicit, and tested.

## The honesty rule

The single most important design rule: **every construct either lowers with
identical observable behavior, or it is refused with a clear error.** A
transpiler must never emit target code that silently behaves differently from the
source. When the Rust subset meets something it cannot translate faithfully, it
raises a `TranspileError` with a span rather than guessing. Keeping the supported
subset small is what makes this rule enforceable: a minimal language has few
lowering rules, and every one can be held to the standard.

## Outsourcing the type checker

ferrel requires the transpiler's input to be valid, compilable Rust. This is a
deliberate technique: **let the host language's type checker check the embedded
program.** Because the functions you transpile are ordinary Rust, `rustc`
type-checks them, enforces exhaustiveness on `match`, and lets you unit-test the
logic as Rust before transpiling, with no type theory implemented inside ferrel.
The same idea appears in the typed eDSL, where the phantom types of `El<T>` make
`add(string, int)` a Rust type error. A well-typed object program is, by
construction, a well-typed host program.

## The foreign-function interface

A transpiled function is useless if it cannot call Emacs. Calling external Elisp
is modeled as a typed **foreign-function interface**, the same idea as an
`extern` block in systems languages. You declare the Elisp functions you use with
Rust signatures; `rustc` then checks your calls, and ferrel maps each name from
Rust convention to Elisp convention (`snake_case` to `kebab-case`, with an
explicit override for names that do not transform cleanly, such as `1+` or
`string-empty-p`). The declaration is the contract: ferrel checks the Rust-side
use and emits the mapped symbol. A raw escape exists for the last resort, but the
typed declaration is the intended path, so that calling a package stays as
checked as the rest of your code.

## Why this is a front-end, not a new tool

Everything above produces `Sexp`. The transpiler reuses the same pretty-printer,
the same round-trip guarantees, and the same byte-compile verification as the
other front-ends. That is the whole point of the
[narrow-waist architecture](./architecture.md): a new way to write Elisp is a new
lowering into one shared IR, not a new compiler.
