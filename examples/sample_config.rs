//! A small but realistic Emacs configuration written in the supported Rust
//! subset.
//!
//! This file is BOTH valid, compilable Rust AND valid transpiler input. The
//! `use ferrel::rt::*;` line brings the zero-cost stubs into scope so `rustc`
//! type-checks the configuration as ordinary safe Rust, and the `#[elisp]`
//! foreign declarations supply the few builtins not in the starter prelude.
//!
//! Transpile it with the CLI:
//!
//! ```text
//!   cargo run --bin ferrel-transpile -- \
//!     examples/sample_config.rs -o examples/out/sample-config.el
//! ```
//!
//! Running this example as a program just confirms it compiles and exercises a
//! couple of the functions; the real output is the transpiled `.el`.

use ferrel::rt::*;

// --- Foreign Elisp declarations (the FFI contract) ---------------------------
//
// These declare Elisp functions the transpiler maps by name. They carry real
// bodies so the file compiles; the transpiler discards the bodies and emits the
// mapped symbol. `#[elisp(name = "..")]` overrides the auto kebab-case mapping
// for names that are not valid Rust identifiers.

/// Toggle a global minor mode by its command name.
#[elisp]
fn global_display_line_numbers_mode(_arg: i64) -> Elisp {
    Elisp
}

/// Enable `recentf-mode`.
#[elisp]
fn recentf_mode(_arg: i64) -> Elisp {
    Elisp
}

/// Add one. Declared with a name override because `1+` is the real Elisp symbol
/// and is not a valid Rust identifier.
#[elisp(name = "1+")]
fn inc(n: i64) -> i64 {
    n + 1
}

// --- Configuration variables -------------------------------------------------

/// How many recent files this configuration asks `recentf` to remember.
const MY_RECENTF_MAX: i64 = 200;

/// Whether the configuration enables line numbers by default.
static MY_LINE_NUMBERS_ENABLED: bool = true;

// --- Commands ----------------------------------------------------------------

/// Insert the current timestamp at point.
#[interactive]
fn my_insert_timestamp() {
    insert(format_time_string("%Y-%m-%d %H:%M:%S"));
}

/// Toggle global line numbering on or off.
#[interactive]
fn my_toggle_line_numbers() {
    if MY_LINE_NUMBERS_ENABLED {
        global_display_line_numbers_mode(-1);
    } else {
        global_display_line_numbers_mode(1);
    }
}

/// Greet the user, choosing wording by the length category CATEGORY.
fn my_greeting(category: i64) -> String {
    match category {
        0 => message("Welcome."),
        1 | 2 => message("Hello again."),
        n if n > 10 => message("You have been here a while."),
        _ => message("Hello."),
    }
}

/// Insert NUM blank lines at point, plus one trailing line.
#[interactive("p")]
fn my_insert_blank_lines(num: i64) {
    let total = inc(num);
    for _i in 0..total {
        insert("\n");
    }
}

/// Run the one-time setup: enable recentf, register a save hook, and bind the
/// timestamp command.
///
/// The `raw` escape emits verbatim Elisp for things outside the subset; here it
/// declares and sets the builtin `recentf-max-saved-items`, whose name has no
/// Rust binding to assign to. Assigning to a Rust `let` local lowers to `setq`
/// directly; see `my_running_total` below.
fn my_setup() {
    recentf_mode(1);
    raw("(defvar recentf-max-saved-items)");
    raw("(setq recentf-max-saved-items my-recentf-max)");
    add_hook(sym("before-save-hook"), func("whitespace-cleanup"));
    global_set_key(kbd("C-c t"), func("my-insert-timestamp"));
}

/// Sum the integers from 1 to N, demonstrating a mutable local and assignment.
///
/// The transpiler supports plain assignment `x = e;` (lowering to `setq`), but
/// not the `+=` compound form, so the long `total = total + ..` spelling is the
/// one that transpiles; the clippy lint that prefers `+=` is allowed here on
/// purpose.
#[allow(clippy::assign_op_pattern)]
fn my_running_total(n: i64) -> i64 {
    let mut total = 0;
    for i in 0..n {
        total = total + inc(i);
    }
    total
}

fn main() {
    // Exercise the stubs so this compiles and runs as a plain Rust program.
    my_setup();
    my_insert_timestamp();
    my_toggle_line_numbers();
    my_insert_blank_lines(3);
    let _ = my_greeting(1);
    let _ = my_running_total(5);
    let _ = MY_RECENTF_MAX;
    let _ = MY_LINE_NUMBERS_ENABLED;
    let _ = inc(2);
    println!("sample_config compiled and ran");
}
