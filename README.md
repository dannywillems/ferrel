# ferrel

A typed Rust toolchain for Emacs Lisp: write Emacs plugins and configuration in
Rust, and generate ordinary `.el` files that Emacs loads exactly like
hand-written Elisp. There is no runtime, no native module, and nothing added to
your Emacs. The output is plain Elisp you can read, diff, and commit.

## Why

- You know Rust and its type system, and you would rather not hand-write
  parenthesis-dense, dynamically typed Elisp.
- You want the Rust compiler to reject mistakes, such as passing a string where
  an integer is expected, before any `.el` is written.
- You want generated output that is readable Elisp, not an opaque blob, so you
  can still inspect what Emacs actually runs.

## Architecture: one IR, many front-ends

ferrel is a small compiler built around a single intermediate representation,
the `Sexp` AST. Several front-ends lower into it and one back-end renders from
it:

```
Elisp text          --lexer--> tokens --parser--> Sexp
Rust eDSL (El<T>)   --lower-->                     Sexp  --codegen--> .el text
Rust source subset  --syn-----> Rust AST --lower-->Sexp   (transpiler, see below)
```

Because both directions share `Sexp`, the pipeline round-trips: parsing then
rendering reproduces an equivalent file, and rendering then parsing reproduces
an equal AST. This is checked as a property and against real configuration.

## Authoring: the typed eDSL

```rust
use ferrel::*;

let pkg = Package::new("ferrel-hello", "A greeting command.")
    .emacs_version("27.1")
    .defun(
        Defun::new("ferrel-hello")
            .doc("Greet the user.")
            .interactive()
            .body([message(string("Hello from Rust-generated Elisp!"), [])]),
    )
    .form(global_set_key("C-c h", "ferrel-hello"));
```

generates:

```elisp
(defun ferrel-hello ()
  "Greet the user."
  (interactive)
  (message "Hello from Rust-generated Elisp!"))

(global-set-key (kbd "C-c h") #'ferrel-hello)
```

The typed layer (`El<Int>`, `El<Str>`, `El<Bool>`, ...) makes wrong-typed
programs a compile error: `add(string("x"), int(2))` does not build. On top of
it are fluent builders for the forms a real config uses:

- `Defun` for functions and interactive commands,
- `Defcustom` (with a `CustomType` enum), `defvar`, `defconst`,
- a typed `UsePackage` builder for `use-package` blocks,
- typed key sequences (`KeySeq`, `Command`, `kbd!`),
- heterogeneous statement bodies (`Stmt`, `stmts!`),
- hooks, keybindings, and mode toggles.

Emacs has thousands of functions and every package adds more, so ferrel types
the core you reach for constantly and gives you an explicit escape hatch for the
rest:

```rust
// (projectile-project-root) returns a path; assert the type you expect.
let root: El<Str> = call_typed("projectile-project-root", Vec::<Sexp>::new());
```

## Reading: the Elisp parser

ferrel includes a lexer and reader for Emacs Lisp, so it can parse `.el` back
into the same `Sexp` AST it generates:

```rust
let forms = ferrel::parse("(defun f (a b) (+ a b))")?;
```

The reader covers the core surface syntax (numbers including radixes, characters,
strings, symbols and keywords, the quoting shorthands, proper and dotted lists,
and vectors) and is verified to round-trip a real modular Emacs config.

## Transpiling Rust to Elisp

A third front-end lets you write a defined subset of real, compilable Rust and
transpile it to Elisp. You write ordinary Rust functions; `rustc` type-checks
them, and `ferrel-transpile` lowers them to `.el`:

```rust
use ferrel::rt::*;

#[elisp(name = "1+")]
fn inc(n: i64) -> i64 { unreachable!() } // FFI: declares the Elisp `1+`

/// Insert NUM blank lines at point.
#[interactive("p")]
fn my_insert_blank_lines(num: i64) {
    let total = inc(num);
    for _i in 0..total {
        insert("\n");
    }
}
```

```sh
ferrel-transpile config.rs -o config.el            # .rs in, .el out
ferrel-transpile config.rs --byte-compile          # also drive Emacs to .elc
```

The design is fixed and enforced:

- The input is valid, type-checked Rust, so `rustc` is the type checker. The
  transpiler accepts only a subset of the grammar (`fn`, `let`, `if`/`match`,
  `while`, `for`, operators, calls, closures) and refuses anything outside it
  with a located `line:col` error, never a silent mistranslation.
- External Elisp is called through a typed foreign-function interface: declare
  functions with `#[elisp]` (names map `snake_case` to `kebab-case`, with an
  override for names like `1+`), and use the `sym`/`func`/`kbd`/`raw` intrinsics
  for symbols, function references, key sequences, and verbatim escapes. The
  `ferrel::rt` prelude provides safe stubs so your `.rs` compiles as ordinary
  Rust.
- It ships as a `.rs`-in `.el`-out CLI, with optional flags that drive Emacs to
  also produce byte-compiled (`.elc`) and native (`.eln`) output.

See `examples/sample_config.rs` for a realistic config in the subset; its
transpiled output byte-compiles cleanly in Emacs.

## Testing

- Round-trip: `parse(render(x)) == x` as a property.
- Differential against the reference implementation: generated `.el` is
  byte-compiled in real Emacs with warnings treated as errors.
- Corpus fuzzing: a scheduled job samples random `.el` files from MELPA and
  parses them (never evaluating them) to surface reader gaps as tracked issues.
  See `scripts/fetch-melpa-corpus.sh` and the `corpus` example.

## Documentation

The site under `doc/` is both a guide and a course. Alongside the ferrel
tutorials, it includes a graduate-level treatment of Emacs Lisp (history, the
full reader syntax, evaluation semantics, how Emacs loads `.el`, and the `.elc`
byte-code format and VM) and a tour of ferrel's compiler internals. It is
published at https://dannywillems.github.io/ferrel/.

## Commands

```
make build          # compile the crate
make test           # run tests
make example        # generate examples/out/ferrel-hello.el
make verify-elisp   # byte-compile and run the generated plugin in Emacs
make lint           # clippy
make check-format   # rustfmt --check (nightly)
make corpus-fetch   # download a random MELPA .el corpus (COUNT=50)
make corpus-test    # parse the corpus (no eval) and report failures
make doc-install    # install the documentation site dependencies
make doc-dev        # run the documentation site locally
```

## Contributing model

Two reusable agents under `.claude/agents/` encode a maintainer split: a
compiler engineer who owns `src/` and knows Emacs Lisp, and a Rust API reviewer
who only consumes the public API and reports ergonomics feedback. The API is
judged by the reviewer; correctness is owned by the engineer.

## Status

The core (AST, lexer, parser, renderer, typed layer, builders, package writer)
works and is verified against batch Emacs. The Rust-to-Elisp transpiler covers a
useful subset and refuses the rest with located errors; its sample output
byte-compiles cleanly. The typed builtin surface and the transpiler subset both
grow as real configuration is ported.

## License

MIT OR Apache-2.0.
