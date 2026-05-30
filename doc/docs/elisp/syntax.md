---
sidebar_position: 2
---

# The full Emacs Lisp syntax

Emacs Lisp has almost no syntax in the usual sense. There is one rule: text is
read into data objects by a function called the reader, and code is just data
whose structure happens to be evaluable. This property is called homoiconicity.
Learning Elisp syntax means learning the reader's printed representations for
each kind of object. This page covers them all, and notes which ones ferrel's
parser currently reads.

## The reader, in one sentence

`read` consumes characters and returns one Lisp object; `print` does the inverse.
Whitespace and comments separate tokens but are otherwise insignificant. A
program file is just a sequence of objects that the loader reads and evaluates
one after another.

## Comments and whitespace

A semicolon begins a comment that runs to end of line. Convention assigns levels
by the number of semicolons:

```elisp
; inline comment, often aligned to the right
;; a comment describing the following line of code
;;; a section heading or top-level file comment
;;;; rare, for major sections
```

Emacs Lisp has no general block-comment syntax. The reader also recognizes a few
`#`-prefixed skip sequences in compiled files (such as `#@n` to skip a
byte-compiled docstring), which you will see in `.elc` but almost never write.

## Atoms

### Integers

Decimal by default, with an optional sign. Since Emacs 27 integers are
arbitrary precision (bignums) once they exceed the fixnum range.

```elisp
42      -7      +3
#xFF    ; hexadecimal, 255
#o17    ; octal, 15
#b1010  ; binary, 10
#16rFF  ; explicit radix N, written #NrDIGITS, here base 16
```

### Floats

A float must contain a decimal point or an exponent, with at least one digit
before the point:

```elisp
1.5     3.0     1e3     6.022e23     -0.5
```

### Characters

A character is just an integer (its code point), written with `?`:

```elisp
?A      ; the integer 65
?\n     ; newline
?\t ?\r ?\f ?\v ?\e ?\d   ; tab, return, formfeed, vtab, escape, delete
?\s     ; space
?\\     ; backslash
?\(     ; a literal open paren as a character
?\C-a   ; control-A
?\M-x   ; meta-x
?\^I    ; another way to write control-I (tab)
?\u00e9 ; Unicode code point by hex (small e with acute)
?\101   ; octal escape
```

### Strings

Double-quoted, with C-like escapes. Strings are sequences of characters and may
span lines literally. A string may carry text properties when written with the
propertized form below.

```elisp
"hello\nworld"
"a quote: \" and a backslash: \\"
"\x41\u00e9"
```

### Symbols

A symbol is a named object. Its print name may contain letters, digits, and most
punctuation; characters that would otherwise be special can be escaped with a
backslash. Symbols are case sensitive.

```elisp
foo        my-function     +     1+     <=     string-empty-p
\,weird    ; a symbol whose name contains a comma, via escaping
```

Three symbols are special to the reader and evaluator:

```elisp
nil   ; the empty list and falsehood
t     ; canonical truth
##    ; the symbol whose name is the empty string
```

Keywords are symbols whose name starts with a colon. They evaluate to
themselves, which makes them convenient as plist and function-argument tags:

```elisp
:type   :group   :ensure
```

Uninterned symbols, used by macros to avoid capture, are written with `#:`:

```elisp
#:g123
```

## Quoting forms

These are reader shorthands that expand into ordinary lists:

```elisp
'x      reads as   (quote x)
#'x     reads as   (function x)
`x      reads as   a backquote (quasiquote) template
,x      reads as   an unquote, valid only inside a backquote
,@x     reads as   an unquote-splicing
```

A backquote builds a structure that is mostly literal, with holes filled by
unquoted expressions:

```elisp
`(a b ,(+ 1 2) ,@(list 'x 'y))   ; evaluates to (a b 3 x y)
```

## Compound objects

### Lists and cons cells

The list is the fundamental structure. It is built from cons cells, each holding
a `car` and a `cdr`. A proper list ends in `nil`; a dotted pair shows the cdr
explicitly:

```elisp
(a b c)        ; a proper list, equivalent to (a . (b . (c . nil)))
(a . b)        ; a dotted pair (improper list)
(a b . c)      ; leading elements then a non-nil tail
()             ; the empty list, identical to nil
```

### Vectors

Square brackets denote a vector, a fixed-length array. Unlike lists, a vector is
self-evaluating: it evaluates to itself, not to a function call.

```elisp
[1 2 3]   ["a" b 3.0]
```

### Hash tables, byte-code, and other `#` forms

The reader has printed representations for several built-in object types, almost
all introduced by `#`:

```elisp
#s(hash-table test equal data (k1 v1 k2 v2))   ; a hash table
#("text" 0 4 (face bold))                       ; a string with text properties
#[arglist byte-code constants depth]            ; a byte-code object (see .elc)
#&8 "..."                                        ; a bool-vector
#^[...]                                          ; a char-table
```

You write hash tables and propertized strings occasionally; byte-code objects,
bool-vectors, and char-tables you almost only ever see in printed or compiled
output.

### Shared and circular structure

The reader can represent shared and circular structure with labels, where `#n=`
names an object and `#n#` refers back to it:

```elisp
#1=(a b . #1#)   ; an infinite circular list
(#1=(x) #1#)     ; a list whose two elements are the same cons cell
```

## What ferrel reads today

ferrel's parser ([the reader](../internals/architecture.md)) covers the core
you meet in ordinary source: comments, all the number forms above including
radixes, characters with common escapes, strings, symbols and keywords, the four
quoting shorthands, proper and dotted lists, and vectors. The advanced printed
forms (`#s(...)`, `#(...)`, `#[...]`, bool-vectors, char-tables, and `#n=`
shared-structure labels) are not yet read; ferrel's MELPA corpus test
deliberately samples real packages to find exactly these gaps and file them. See
[testing](../internals/testing.md).

## Frequently asked questions

### Why is `[1 2 3]` self-evaluating but `(1 2 3)` an error to evaluate?

A list is read as a function application: evaluating `(1 2 3)` tries to call the
function `1`, which fails. A vector is a data literal: it evaluates to itself. To
get a list as data, quote it: `'(1 2 3)`.

### Is a character a separate type from an integer?

No. A character literal like `?A` reads as the integer `65`. Characters are just
integers interpreted as code points; the `?` syntax is a readable way to write
them.

### Why do macros use `#:` symbols?

To avoid variable capture (hygiene). An uninterned symbol created with `make-symbol`,
printed as `#:g123`, is guaranteed not to collide with any symbol the user
wrote, so a macro can introduce temporaries safely.
