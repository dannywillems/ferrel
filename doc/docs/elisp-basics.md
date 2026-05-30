---
sidebar_position: 2
---

# Emacs Lisp basics

This page explains the building blocks of an Emacs plugin in Emacs Lisp, so the
ferrel tutorials have something to map onto. Each concept is shown in Elisp
first; the [tutorials](./tutorials/hello-plugin.md) then show the ferrel
equivalent.

If you already know Elisp, skim this and move on.

## A plugin is a `.el` file with a specific shape

A well-formed Emacs Lisp package file has a header, a body, and a footer:

```lisp
;;; my-package.el --- One-line summary -*- lexical-binding: t; -*-

;; Author: Your Name
;; Package-Requires: ((emacs "27.1"))

;;; Commentary:
;; Longer description.

;;; Code:

;; ... definitions go here ...

(provide 'my-package)
;;; my-package.el ends here
```

Two parts matter for correctness:

- The `lexical-binding: t` cookie on the first line enables lexical scoping.
  Modern Elisp expects it.
- The trailing `(provide 'my-package)` lets other files `(require 'my-package)`.

ferrel writes this entire skeleton for you.

## Functions: `defun`

A function is defined with `defun`. The form is
`(defun NAME (ARGS) "DOC" BODY...)`.

```lisp
(defun my-double-sum (a b)
  "Return twice the sum of A and B."
  (* (+ a b) 2))
```

Note the prefix arithmetic: `(* (+ a b) 2)` means `(a + b) * 2`. This is one of
the parenthesis-heavy patterns ferrel removes by letting you write
`mul(add(a, b), int(2))`.

## Interactive commands: `(interactive)`

A function becomes a command the user can run with `M-x` when its body starts
with `(interactive)`:

```lisp
(defun my-hello ()
  "Show a greeting."
  (interactive)
  (message "Hello!"))
```

Without `(interactive)`, a function can only be called from other code.

## Variables: `defvar`, `defcustom`, `setq`

- `defvar` declares a variable with a default and documentation.
- `defcustom` declares a user-customizable variable with a type and a group, so
  it appears in the Emacs customization UI.
- `setq` assigns a value.

```lisp
(defcustom my-greeting "Hello!"
  "Greeting shown by `my-hello'."
  :type 'string
  :group 'my-package)

(setq my-greeting "Hi there")
```

The `:type 'string` is the customization type; the `:group` decides where the
option appears in the customize tree.

## Keybindings: `kbd` and `global-set-key`

A key sequence is parsed with `kbd`, and bound with `global-set-key`. The
`#'` prefix takes the function value of a symbol.

```lisp
(global-set-key (kbd "C-c h") #'my-hello)
```

## Hooks: `add-hook`

A hook runs a function when some event happens, such as entering a major mode.

```lisp
(add-hook 'prog-mode-hook #'display-line-numbers-mode)
```

## Loading other features: `require`

`require` loads another feature (another `provide`d file), failing if it is not
found.

```lisp
(require 'cl-lib)
```

## Putting it together

A minimal but complete plugin uses most of the above: a `defcustom` for
configuration, one or more interactive `defun`s, and a keybinding. The
[hello plugin tutorial](./tutorials/hello-plugin.md) builds exactly that, in
Elisp and in ferrel side by side.
