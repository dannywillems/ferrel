---
sidebar_position: 3
---

# The semantics of Emacs Lisp

Syntax tells you how text becomes objects; semantics tells you what evaluating
those objects does. This page covers the evaluation model, the symbol and
namespace design, scoping, truth, the distinction between functions, macros, and
special forms, equality, error handling, and the editor runtime. These are the
rules ferrel must respect when it lowers Rust or builds forms, because emitting
syntactically valid Lisp with the wrong semantics is worse than emitting nothing.

## The evaluation model

Evaluation is defined by cases on the kind of object:

- **Self-evaluating objects** evaluate to themselves: numbers, strings,
  characters, vectors, keywords, and the symbols `t` and `nil`.
- **A symbol** evaluates to the value in its variable cell. If the variable is
  unbound, evaluation signals `void-variable`.
- **A list** is a combination. Its first element determines what happens:
  - if it names a **special form**, the evaluator applies that form's own rule;
  - if it names a **macro**, the macro is expanded and the result evaluated;
  - otherwise it is a **function call**: the remaining elements are evaluated
    left to right, and the function is applied to those values.

That three-way split on the head of a list is the heart of the language.

## Lisp-2: two namespaces per symbol

A symbol carries several cells, the important ones being a value cell and a
function cell. The same name can be a variable and a function at once:

```elisp
(defun list (x) x)   ; would shadow nothing in the value namespace
(let ((list '(1 2))) ; `list' the variable
  (car list))        ; uses the value cell -> 1
```

Because the two namespaces are separate, you need explicit ways to cross them.
`#'f` (which reads as `(function f)`) takes the function-cell value of `f`, and
`funcall` and `apply` call a function held as an ordinary value:

```elisp
(funcall #'+ 1 2)          ; 3
(apply #'+ '(1 2 3))       ; 6
(mapcar #'1+ '(1 2 3))     ; (2 3 4)
```

A Lisp that uses one namespace (like Scheme) does not need `funcall`. ferrel
models this split directly: a function reference and a variable reference are
different AST nodes.

## Anatomy of a symbol

A symbol is a first-class object with four components: a print name, a value
cell, a function cell, and a property list. Symbols are interned in a table
called the obarray, so that reading the same name twice yields the same object,
which makes symbol comparison a pointer comparison. `make-symbol` produces an
uninterned symbol, used by macros for hygienic temporaries.

```elisp
(symbol-name 'foo)      ; "foo"
(get 'foo 'some-prop)   ; the property list
(intern "foo")          ; the interned symbol named "foo"
```

## Truth: nil and everything else

There is one false value, `nil`, which is also the empty list. Every other value
is true, and `t` is the canonical true. This conflation of false and empty list
is pervasive and idiomatic:

```elisp
(if nil 'yes 'no)        ; no
(if '() 'yes 'no)        ; no, since () is nil
(if 0 'yes 'no)          ; yes, because 0 is non-nil
(if "" 'yes 'no)         ; yes, the empty string is non-nil
```

ferrel maps the Rust `bool` `true`/`false` to `t`/`nil`, and `nil` doubles as
the empty list when that is what a form needs.

## Special forms, macros, and functions

These three look alike but evaluate differently, and confusing them is the most
common way to generate broken Lisp:

- A **function** receives already-evaluated arguments. `+`, `car`, `message` are
  functions.
- A **special form** is built into the evaluator and controls evaluation of its
  own arguments. `if`, `let`, `setq`, `quote`, `and`, `or`, `while`,
  `progn`, `function`, and `lambda` are special forms. `(if c a b)` must not
  evaluate both `a` and `b`; only a special rule can express that.
- A **macro** is a function from unevaluated source to source, run before
  evaluation (and once, at compile time, under byte-compilation). `use-package`,
  `with-eval-after-load`, `push`, `cl-loop`, and `when`/`unless` are macros.

ferrel never models a special form or macro as an ordinary call. `when`,
`dolist`, `let`, and `use-package` each have structural handling, because their
arguments are not a flat list of evaluated values.

## Scoping: dynamic versus lexical

This is the subtlest part of the language, and the reason the
`lexical-binding: t` cookie exists.

Under **lexical scoping** (the modern default that ferrel emits), a variable
reference resolves to the nearest enclosing binding in the source text, and a
`lambda` captures its environment, producing a true closure:

```elisp
;;; -*- lexical-binding: t; -*-
(defun adder (n)
  (lambda (x) (+ x n)))   ; closes over n
(funcall (adder 10) 5)    ; 15
```

Under **dynamic scoping** (the historical default), a variable reference
resolves to the most recent binding on the call stack at run time, regardless of
text, and `lambda` does not close over locals:

```elisp
;;; -*- lexical-binding: nil; -*-
(defvar n 0)
(defun show () n)         ; reads whatever n is bound to now
(let ((n 42)) (show))     ; 42, because the dynamic binding is visible
```

A variable declared with `defvar` (or `defcustom`) is **special**: even in a
lexically scoped file, `let`-binding it is dynamic. This is how configuration
variables are meant to be temporarily rebound. The practical rule ferrel follows:
emit `lexical-binding: t`, treat `let` and parameters as lexical, and reserve
dynamic behavior for variables explicitly declared with `defvar`/`defcustom`.

## Quoting controls evaluation

`quote` stops evaluation, returning its argument as data; backquote builds
structure with evaluated holes:

```elisp
(quote (a b))     ; the list (a b), unevaluated
'(a b)            ; same
`(1 ,(+ 1 1) 3)   ; (1 2 3)
```

## Equality has several notions

Choosing the wrong equality is a frequent bug:

- `eq` is object identity (pointer equality). Interned symbols and small
  integers are `eq` when equal.
- `eql` is `eq` plus numeric and character equality of the same type.
- `equal` is structural: it recurses through lists, strings, and vectors.
- `=` compares numbers, coercing across integer and float.

```elisp
(eq 'a 'a)              ; t
(equal '(1 2) '(1 2))   ; t
(eq '(1 2) '(1 2))      ; nil, different cons cells
(= 1 1.0)               ; t
```

## Error handling: signals, throws, and cleanup

Elisp separates exceptional control flow from non-local exits:

- `signal` raises a condition; `condition-case` catches by condition type. This
  is the try/catch analogue.
- `throw` and `catch` perform a labeled non-local exit for ordinary control
  flow, not errors.
- `unwind-protect` guarantees cleanup runs whether or not the body exits
  normally, like a `finally`.

```elisp
(condition-case err
    (signal 'arith-error nil)
  (arith-error (message "caught: %S" err)))
```

## The editor runtime

Unlike most languages, the Elisp standard environment is an editor. Much of the
library operates by side effect on the current buffer through an implicit cursor
called point:

```elisp
(with-current-buffer "*scratch*"
  (goto-char (point-min))
  (insert "hello"))
```

Buffers, markers, windows, and the current buffer are part of the language's
runtime, not an external API. Code that ferrel generates for commands routinely
moves point and edits buffers, so the generated forms must respect these
stateful conventions (for example, wrapping cursor motion in `save-excursion`
when the original position should be restored).

## One thing Elisp does not do

Emacs Lisp does not guarantee tail-call optimization. Deep recursion can exceed
`max-lisp-eval-depth`. Iterative constructs (`while`, `dolist`, `dotimes`) are
the idiomatic way to loop, which is why ferrel lowers Rust `for` and `while` to
those forms rather than to recursion.

## Frequently asked questions

### Why does `(if 0 'yes 'no)` return `yes`?

Because the only false value is `nil`. Zero, the empty string, and the empty
vector are all non-nil and therefore true. Only `nil` (equivalently `()`) is
false.

### When do I need `funcall` instead of just calling?

When the function is held as a value, for example passed in as an argument or
stored in a variable. `(funcall fn args...)` calls the function in `fn`'s value.
A name written directly in head position, `(fn args...)`, uses the function cell
and needs no `funcall`.

### What is the difference between a macro and a function for code generation?

A function sees evaluated arguments at run time; a macro sees raw source and
runs once, before evaluation, to produce new source. A code generator like
ferrel must expand or structurally build macro calls, because passing them
through as if they were functions would evaluate arguments that a macro intends
to receive unevaluated.
