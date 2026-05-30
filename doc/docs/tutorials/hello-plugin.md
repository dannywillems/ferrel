---
sidebar_position: 1
---

# Tutorial: a hello plugin, in Elisp and in ferrel

This tutorial builds the same small Emacs plugin two ways: first in idiomatic
Emacs Lisp, then in ferrel. The plugin has a customizable greeting, two
interactive commands, a pure helper function, and a keybinding.

By the end you can see exactly which Elisp form each ferrel call produces.

## The goal

The plugin, `ferrel-hello`, provides:

- `ferrel-hello-greeting`: a user-customizable greeting string.
- `ferrel-hello`: an interactive command that shows the greeting.
- `ferrel-hello-insert`: an interactive command that inserts the greeting.
- `ferrel-hello-double-sum`: a pure function returning `(a + b) * 2`.
- a `C-c h` keybinding for `ferrel-hello`.

## Version 1: Emacs Lisp

```lisp
;;; ferrel-hello.el --- A greeting command -*- lexical-binding: t; -*-

;; Package-Requires: ((emacs "27.1"))

;;; Commentary:
;; A greeting command.

;;; Code:

(defcustom ferrel-hello-greeting "Hello from Rust-generated Elisp!"
  "Greeting shown by `ferrel-hello'."
  :type 'string
  :group 'ferrel-hello)

(defun ferrel-hello ()
  "Display the configured greeting in the echo area."
  (interactive)
  (message ferrel-hello-greeting))

(defun ferrel-hello-insert ()
  "Insert the configured greeting at point."
  (interactive)
  (insert ferrel-hello-greeting))

(defun ferrel-hello-double-sum (a b)
  "Return twice the sum of A and B."
  (* (+ a b) 2))

(global-set-key (kbd "C-c h") #'ferrel-hello)

(provide 'ferrel-hello)
;;; ferrel-hello.el ends here
```

## Version 2: ferrel

```rust
use ferrel::*;

fn main() -> std::io::Result<()> {
    let pkg = Package::new("ferrel-hello", "A greeting command.")
        .emacs_version("27.1")
        .form(defcustom(
            "ferrel-hello-greeting",
            string("Hello from Rust-generated Elisp!"),
            "Greeting shown by `ferrel-hello'.",
            "string",
            "ferrel-hello",
        ))
        .defun(
            Defun::new("ferrel-hello")
                .doc("Display the configured greeting in the echo area.")
                .interactive()
                .body([message(var::<Str>("ferrel-hello-greeting"), [])]),
        )
        .defun(
            Defun::new("ferrel-hello-insert")
                .doc("Insert the configured greeting at point.")
                .interactive()
                .body([insert(var::<Str>("ferrel-hello-greeting"))]),
        )
        .defun(
            Defun::new("ferrel-hello-double-sum")
                .doc("Return twice the sum of A and B.")
                .param("a")
                .param("b")
                .body([mul(add(var::<Int>("a"), var::<Int>("b")), int(2))]),
        )
        .form(global_set_key("C-c h", "ferrel-hello"));

    pkg.write("ferrel-hello.el")
}
```

Running this writes the exact Elisp from Version 1.

## Line-by-line mapping

| Elisp | ferrel |
| --- | --- |
| `(defcustom name val "doc" :type 'string :group 'g)` | `defcustom("name", val, "doc", "string", "g")` |
| `(defun f () ...)` | `Defun::new("f").body([...])` |
| `(interactive)` | `.interactive()` |
| `"docstring"` as first body form | `.doc("docstring")` |
| `(message x)` | `message(x, [])` |
| `(insert x)` | `insert(x)` |
| reading variable `name` | `var::<Str>("name")` |
| `(* (+ a b) 2)` | `mul(add(a, b), int(2))` |
| `(global-set-key (kbd "C-c h") #'f)` | `global_set_key("C-c h", "f")` |

## Where the types help

In Elisp, this mistake is silent until it runs and produces a confusing error:

```lisp
(+ "two" 3)   ;; wrong: a string is not a number
```

In ferrel, the equivalent does not compile:

```rust
add(string("two"), int(3)) // type error: expected El<Int>, found El<Str>
```

The Rust compiler rejects it before any `.el` is written. That is the core
value: the same generated Elisp, but with mistakes caught at compile time.

## Verifying the output

The generated file is ordinary Elisp, so you can byte-compile and run it in
batch Emacs:

```bash
emacs --batch --eval '(setq byte-compile-error-on-warn t)' \
  -f batch-byte-compile ferrel-hello.el
emacs --batch -l ./ferrel-hello.el \
  --eval '(princ (ferrel-hello-double-sum 2 3))'   # prints 10
```
