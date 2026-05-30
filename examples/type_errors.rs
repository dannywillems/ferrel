//! Confirm that ferrel's typed layer rejects type mistakes at COMPILE time.
//!
//! Run with: `cargo run --example type_errors`
//!
//! This file COMPILES. Each block below pairs a correct line that builds with
//! a commented-out line that does NOT. I verified every commented line fails
//! `cargo build` (the exact rustc errors are quoted next to each one). To
//! re-check, uncomment any single `// WONT COMPILE` line and run
//! `cargo build --example type_errors`.

use ferrel::*;

fn main() {
    // 1. Arithmetic is typed: `add` wants two `El<Int>`.
    let ok_sum = add(int(2), int(3));
    println!("ok: {}", ok_sum.render());
    // WONT COMPILE: string where Int expected.
    //   error[E0308]: mismatched types
    //   expected `El<Int>`, found `El<Str>`
    // let bad = add(int(2), string("three"));

    // 2. `equal` is homogeneous: both sides must share `T`.
    let ok_eq = equal(int(1), int(1));
    println!("ok: {}", ok_eq.render());
    // WONT COMPILE: comparing Int with Str.
    //   error[E0308]: mismatched types
    //   expected `El<Int>`, found `El<Str>`
    // let bad = equal(int(1), string("1"));

    // 3. `not` wants a Bool, not an Int.
    let ok_not = not(gt(int(2), int(1)));
    println!("ok: {}", ok_not.render());
    // WONT COMPILE: Int is not Bool.
    //   error[E0308]: mismatched types
    //   expected `El<Bool>`, found `El<Int>`
    // let bad = not(int(0));

    // 4. `if_` requires both branches to have the same type `T`.
    let ok_if = if_(t(), string("yes"), string("no"));
    println!("ok: {}", ok_if.render());
    // WONT COMPILE: branches disagree (Str vs Int).
    //   error[E0308]: mismatched types
    //   expected `El<Str>`, found `El<Int>`
    // let bad = if_(t(), string("yes"), int(0));

    // 5. `insert` wants an `El<Str>`, not an `El<Int>`.
    let ok_insert = insert(string("hi"));
    println!("ok: {}", ok_insert.render());
    // WONT COMPILE: insert of an Int.
    //   error[E0308]: mismatched types
    //   expected `El<Str>`, found `El<Int>`
    // let bad = insert(int(7));

    // 6. `concat` wants an iterator of `El<Str>`.
    let ok_concat = concat([string("a"), string("b")]);
    println!("ok: {}", ok_concat.render());
    // WONT COMPILE: an Int in the El<Str> iterator.
    //   error[E0308]: mismatched types
    //   expected `El<Str>`, found `El<Int>`
    // let bad = concat([string("a"), int(1)]);

    // FRICTION (Medium): the escape hatch is *too* powerful and silently
    // erases types. `call(...).cast::<Str>()` lets me assert ANY type with
    // zero checking, so a wrong `.cast` is never caught:
    let pretend_int: El<Int> = call("buffer-size", []).cast(); // no error, ever
    println!("unchecked cast: {}", pretend_int.render());
    // There is no compile-time signal distinguishing a justified cast from a
    // bug. A `try`-style or doc-annotated cast would not help at compile time,
    // but at least a `call_typed::<Str>(name, args)` returning `El<Str>`
    // directly would make the asserted type a deliberate, greppable choice
    // rather than a trailing `.cast()` that is easy to get wrong.
}
