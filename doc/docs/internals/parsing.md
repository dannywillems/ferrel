---
sidebar_position: 2
---

# Lexing and parsing

A parser turns flat text into a tree. ferrel has two parsing problems, and they
are interestingly asymmetric: reading Emacs Lisp is almost trivial, while reading
the Rust subset for the transpiler is hard enough that ferrel does not do it by
hand. This page explains both and the techniques behind them.

## The two-stage split

The classic structure is a lexer followed by a parser:

- The **lexer** (`src/lexer.rs`) turns characters into tokens: parentheses,
  numbers, strings, symbols, the reader-macro prefixes. It is a hand-written
  scanner that tracks byte spans, so errors can point at an exact offset.
- The **parser** (`src/parser.rs`) turns tokens into the `Sexp` tree.

Separating the two keeps each simple: the lexer worries about characters and
escapes, the parser worries about structure.

## Why Lisp is the easy case

Emacs Lisp is homoiconic: the surface syntax is the data structure. There is no
operator precedence, no statement grammar, no distinction between expressions and
declarations. A list is just elements between parentheses. So ferrel's "parser"
is really a reader: a short recursive-descent walk that, on each token, either
starts a list, closes one, wraps the next form in a quote node, or returns an
atom. The reader macros (`'`, `` ` ``, `,`, `,@`, `#'`) are one-token prefixes
that wrap the following form. Dotted pairs are the only structural subtlety.

That is the whole parser. The simplicity is a property of the language, not of
ferrel.

## Why Rust is the hard case

Rust has a rich grammar: operator precedence, generics, patterns, closures,
macros, and lifetimes. Writing a correct parser for it by hand is a large, ongoing
effort, and getting precedence and associativity right alone calls for techniques
like recursive descent with precedence climbing (Pratt parsing). ferrel does not
attempt this. The transpiler uses the `syn` crate, the de-facto standard Rust
parser, via `syn::parse_file`. `syn` returns a faithful Rust AST and absorbs all
the grammar complexity, so the transpiler only has to lower an already-built
tree, never to parse characters. This is also why the dependency is justified:
re-implementing a Rust parser would be far more code and far more fragile than
taking the standard one.

## Spans and error reporting

Both the lexer and the reader carry byte offsets. A good error in a compiler is
not "syntax error" but a message attached to a location, and ferrel's parse
errors report the byte offset of the offending token. The same discipline is what
lets the MELPA corpus harness say not just that a file failed but where, so a
failure becomes an actionable test case. See [testing](./testing.md).

## Round-trip as a correctness contract

Because the reader and the pretty-printer share the `Sexp` IR, parsing then
rendering should reproduce an equivalent file, and rendering then parsing should
reproduce an equal tree. ferrel asserts the second as a property,
`parse(render(x)) == x`, and tests it on real configuration. Keeping reader
macros and dotted pairs as explicit nodes (rather than desugaring them) is what
makes this hold; nothing about the surface is silently normalized away.
