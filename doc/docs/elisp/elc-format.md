---
sidebar_position: 5
---

# The .elc format and the Emacs virtual machine

An `.elc` file is byte-compiled Emacs Lisp: the output of `byte-compile-file`,
in which each function body has been translated into byte-code for the Emacs
Lisp virtual machine. This chapter explains what is actually inside an `.elc`,
how the VM runs it, how native compilation extends it, and why ferrel emits
source and lets Emacs produce the compiled forms.

## What an .elc file is, concretely

An `.elc` is not an object file like ELF. It is closer to a `.el` file in which
the function bodies have been replaced by compiled objects. It consists of:

1. **A header.** The first line carries the magic token `;ELC` followed by a
   version byte, then a few `;;;` comment lines recording the Emacs version that
   compiled it. The remainder is a sequence of printed Lisp forms, read and
   evaluated at load time just like source.

2. **Byte-code objects**, printed with the reader syntax `#[ ... ]`. A compiled
   function is one of these objects, with these slots:
   - **arglist**: a parameter list, or, for lexically compiled functions, an
     integer that packs the counts of mandatory, optional, and rest arguments as
     a bitfield.
   - **byte-code**: a unibyte string whose bytes are VM opcodes.
   - **constants**: a vector that the byte-code indexes into for the symbols and
     literals the function references (a constant pool).
   - **max stack depth**: the operand-stack size the function needs.
   - optional **docstring** and **interactive** specification.

This is the same `#[` form discussed in [the syntax chapter](./syntax.md); it is
ubiquitous in `.elc` and rare in source.

## The virtual machine

The byte-code is executed by a stack machine implemented in C (`bytecode.c`).
Each opcode operates on an operand stack: some push (a constant, a variable's
value), some pop and combine (call a function with N arguments), some branch (a
conditional or unconditional jump to a byte offset). A small sketch of the model:

```
opcode            effect on the stack
constant K        push constants[K]
varref S          push the value of variable S
call N            pop N arguments and a function, push the result
goto L            jump to offset L
return            pop the top value and return it
```

Compiling `(+ a b)` produces roughly: push the value of `a`, push the value of
`b`, apply the addition opcode, leaving the sum on the stack. Running this VM is
faster than walking the source list and re-dispatching on each subform, which is
why byte-compiled code loads and runs faster than interpreted source.

## Two layers of "compiled": .elc and .eln

There are two distinct compilation outputs, and they are often conflated:

- **Byte-compilation** produces `.elc`: byte-code for the VM above. Available in
  every Emacs.
- **Native compilation** (Emacs 28+, via libgccjit) produces `.eln`: real native
  machine code in a shared object, cached per machine. It compiles the byte-code
  representation down through GCC. It is transparent at the source level; Emacs
  substitutes native code when a `.eln` is available.

So the pipeline is layered: source to byte-code to native code, each step
optional and behavior-preserving.

```
foo.el  --byte-compile-->  foo.elc  --native-compile-->  foo.eln
```

## Why the format is not a stable target

The byte-code format is version specific and semi-internal. The opcode set
evolves between Emacs releases, and a `.elc` compiled by one major version is not
guaranteed to load in another. Emacs does not promise `.elc` portability; the
stable, supported contract is at the source level. This is why packages ship
`.el` and byte-compile at install time on the user's own Emacs.

## Could a tool emit .elc directly?

Yes, in principle: emitting `.elc` means becoming a code generator for the Emacs
VM, which entails instruction selection, a constant pool, stack-depth analysis,
and jump resolution. It is a genuine compiler back-end, and a fine exercise. But
for a configuration and plugin tool it is the wrong trade, for two reasons: the
format is version-unstable, and Emacs already contains a correct, maintained,
version-matched byte-compiler and native compiler.

ferrel therefore emits `.el` and, when you want compiled output, drives the
user's Emacs to produce the `.elc` (and `.eln`) with a single subprocess call.
This is an instance of a general compiler strategy: target a higher-level
language and reuse the platform's mature back-end, the same reason many compilers
emit C or LLVM IR rather than machine code. The principle is discussed further in
[the compiler internals](../internals/architecture.md).

## Frequently asked questions

### Is .elc human readable?

Partly. The header and the overall form structure are text, and you can open an
`.elc` in a buffer, but the function bodies are byte-code objects whose
byte-string of opcodes is not meant to be read. You read the `.el`; the `.elc` is
for the machine.

### Does byte-compiling change what my code does?

No. Byte-compilation and native compilation are behavior preserving; they change
performance and, for byte-compilation, surface static warnings. If compiled code
behaves differently from source, that is a bug, which is exactly why compiling
the output in real Emacs is a strong correctness test.

### What is the relationship between .elc and dynamic modules?

They are unrelated. A `.elc` is byte-code for the Emacs VM, produced from Elisp.
A dynamic module (Emacs 25+) is a compiled shared library written in C or Rust
that exposes functions to Elisp through a C ABI. The Rust `emacs` crate builds
the latter; ferrel produces the former's source.
