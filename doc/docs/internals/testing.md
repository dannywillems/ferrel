---
sidebar_position: 4
---

# How a compiler like this is tested

A compiler is a function from programs to programs, which makes it unusually
testable: you can check structural properties, compare against a reference
implementation, and throw real-world inputs at it in bulk. ferrel uses all three,
and they map onto named techniques from compiler engineering. This page explains
them, because the testing strategy is as much a part of the design as the code.

## Round-trip (metamorphic) testing

The strongest invariant ferrel has is a property that must hold for every input:
parsing then rendering then parsing again yields an equal tree,
`parse(render(x)) == x`. This is a metamorphic test: rather than asserting a
specific output for a specific input, it asserts a relationship that holds
universally. It catches an entire class of bugs (a renderer that drops a node, a
parser that mis-reads one) without anyone enumerating cases. ferrel checks it on
its own generated forms and on real configuration files.

## Differential testing against a reference

The definitive question for generated Elisp is not "does it look right" but "does
real Emacs accept it." ferrel's CI byte-compiles its output with the real Emacs
byte-compiler and treats warnings as errors. Emacs is the reference
implementation, and agreeing with it is the only definition of correct output
that matters. This is differential testing: compare your tool's behavior against
an independent oracle. It is the same idea behind tools like CSmith, which test C
compilers by checking that independent compilers agree on random programs.

## Corpus and fuzz testing

Hand-written tests cover the cases you imagined. Real-world inputs find the ones
you did not. ferrel's parser-corpus job downloads a random sample of `.el` source
files from MELPA and parses every one (it never evaluates them), reporting any
file the reader cannot handle. This is corpus-driven fuzzing: real, diverse
inputs exercise paths no unit test would. It is how ferrel learned that the
advanced reader forms `#(...)` (propertized strings) and `##` (the empty symbol)
were missing, with the exact files as ready-made regression cases. The job runs
on a schedule and files an issue when it finds a gap, turning the discovery
straight into tracked work.

## Why these three compose

Each technique catches a different failure mode. Round-trip testing catches
internal inconsistency between reader and printer. Differential testing against
Emacs catches output that is internally consistent but wrong by the platform's
rules. Corpus testing catches inputs outside the imagined envelope. Together they
form a net with few holes, and crucially they require little manual case
authoring, which matters for a project whose surface keeps growing.

## A note on the transpiler

The same playbook extends to the Rust-to-Elisp transpiler. The natural oracle
there is a differential one across two runtimes: take a small Rust function with
a known result, transpile it, byte-compile and run the Elisp in Emacs, and check
that the Elisp result matches the Rust result. When the two runtimes agree on a
broad corpus of snippets, the lowering rules are trustworthy; when they diverge,
the divergence is a precise bug report.
