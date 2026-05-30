---
sidebar_position: 4
---

# How Emacs works with .el files

A `.el` file is loaded by reading each top-level form and evaluating it in order.
This page explains the whole lifecycle: how Emacs finds files, the difference
between `load`, `require`, and `autoload`, how startup files fit together, and
where byte-compilation and native compilation enter. Understanding this is what
lets ferrel emit a single `.el` that behaves like any hand-written package.

## Loading is read-and-evaluate

`load` opens a file, then repeatedly reads one form and evaluates it, top to
bottom, until end of file. There is no separate link or import phase: defining a
function is just evaluating a `defun` form, which installs a function in a
symbol's function cell as a side effect. Order matters, because a form can
depend on the effects of earlier forms.

```elisp
(load "/path/to/my-file.el")
(load "my-file")   ; without extension, searched on load-path
```

## The load path and file resolution

When given a name without a directory, `load` searches the directories in
`load-path`, trying compiled and source forms. Given `foo`, it looks for
`foo.elc`, then `foo.el` (and, with native compilation, may use a `foo.eln`).
If both a source and a byte-compiled file exist, Emacs prefers the `.elc` unless
the `.el` is newer, so a stale compiled file does not shadow edited source.

```elisp
(add-to-list 'load-path "~/.emacs.d/lisp/")
```

## require and provide: features, not files

`require` is the dependency mechanism. A file announces a feature with `provide`
at its end; `require` loads the file only if that feature is not already
present, which makes repeated requires cheap and idempotent:

```elisp
;; in editing.el
(provide 'editing)

;; elsewhere
(require 'editing)   ; loads editing.el once, by searching load-path for it
```

`featurep` tests whether a feature is loaded. This is why every ferrel
`Package` ends with `(provide 'name)`: it makes the generated file requirable
like any other.

## autoload: defer until first use

Loading everything at startup is slow. `autoload` registers a stub that records
"function `F` lives in file `X`"; the first call to `F` loads `X` and then runs
the real definition. Packages mark autoloadable definitions with a magic comment:

```elisp
;;;###autoload
(defun my-command () (interactive) ...)
```

A build step scans these cookies and generates an autoloads file, so the
command is known and callable before its defining file is loaded.

## Startup: which files run, and when

Emacs reads a fixed sequence of files at startup:

- `early-init.el` (Emacs 27+) runs before the GUI and package system are
  initialized, for settings that must take effect early (frame parameters,
  disabling the default package manager).
- `init.el` (in `~/.emacs.d/` or `~/.config/emacs/`) is the main user
  configuration.
- Site files (`site-start.el`) let an administrator configure all users.

A modular configuration typically has `init.el` add a directory to `load-path`
and `require` one feature per module, which is exactly the shape ferrel targets:
one generated `.el` per module, each ending in `provide`.

## Packages: ELPA and MELPA

`package.el` (built in since Emacs 24) installs packages from archives such as
GNU ELPA and MELPA. `package-install` downloads a package, byte-compiles it,
generates its autoloads, and adds it to `load-path`. The `use-package` macro
(bundled since Emacs 29) wraps installation, deferred loading, keybindings, and
configuration of a package into one declarative block, which is why ferrel gives
it a dedicated typed builder.

## Two kinds of compilation

A `.el` file can be loaded as source, but Emacs offers two ahead-of-time
compilation steps that change only performance, not behavior:

- **Byte-compilation** (`byte-compile-file`) turns `.el` into `.elc`, a file of
  byte-code objects for the Emacs virtual machine. Byte-compilation also runs a
  useful static checker that warns about free variables, wrong argument counts,
  and unused lexicals. ferrel's CI byte-compiles its generated output with
  warnings treated as errors, as a correctness gate.
- **Native compilation** (Emacs 28+) goes further, compiling through libgccjit
  to native code in `.eln` files, cached per machine. It is transparent: you
  still load the same source or `.elc`, and Emacs substitutes native code when
  available.

The next chapter, [the .elc format](./elc-format.md), opens up what
byte-compilation actually produces.

## Why ferrel emits source, not compiled files

ferrel writes `.el`, then defers byte-compilation and native compilation to the
user's own Emacs. The reason is compatibility: the byte-code format is tied to
the Emacs version that produced it, while source is the stable, portable
contract. Generating source and letting each Emacs compile it yields
version-correct `.elc` and `.eln` for free. See
[the compiler internals](../internals/architecture.md) for how this fits the
"reuse the platform back-end" principle.

## Frequently asked questions

### What is the difference between load and require?

`load` always evaluates the file. `require` loads a file at most once, keyed by a
feature symbol that the file must `provide`. Use `require` for dependencies you
want loaded idempotently, `load` for unconditional evaluation.

### If I edit a .el but a .elc exists, which one runs?

Emacs prefers the `.elc`, but only if it is not older than the `.el`. If you edit
the source without recompiling, Emacs notices the source is newer and loads it,
so you do not silently run stale byte-code. Recompiling refreshes the `.elc`.

### Do I have to byte-compile my configuration?

No. Source loads and runs correctly. Byte-compilation speeds loading and
execution and surfaces static warnings; native compilation speeds execution
further. All three are optional and produce the same behavior.
