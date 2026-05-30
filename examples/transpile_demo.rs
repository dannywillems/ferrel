//! Transpile an embedded Rust snippet to Elisp and print it.
//!
//! This shows the transpiler front-end end to end: a string of supported Rust
//! goes in, a sequence of `Sexp` forms comes out, and each is rendered to `.el`.
//!
//! Run with: `cargo run --example transpile_demo`

use ferrel::transpile_str;

/// A small but representative chunk of the supported Rust subset. Note that
/// this is the *transpiler input* shape: it would compile on its own with
/// `use ferrel::rt::*;` at the top (the stubs supply `message`, `sym`, etc.).
const SOURCE: &str = r#"
/// The maximum number of recent files to remember.
const RECENTF_MAX_SAVED_ITEMS: i64 = 200;

/// Toggle a line-numbering convenience.
#[interactive]
fn my_toggle_line_numbers() {
    if display_line_numbers_mode() {
        display_line_numbers_mode(-1);
    } else {
        display_line_numbers_mode(1);
    }
}

/// Return a greeting for NAME, choosing the form by the length of the name.
fn my_greeting(name: i64) -> i64 {
    match name {
        0 => message("anonymous"),
        1 | 2 => message("short name"),
        n if n > 10 => message("long name"),
        _ => message("a name"),
    }
}

/// Insert NUM blank lines at point.
fn my_insert_blanks(num: i64) {
    for _i in 0..num {
        insert("\n");
    }
}
"#;

fn main() {
    let forms = match transpile_str(SOURCE) {
        Ok(forms) => forms,
        Err(e) => {
            eprintln!("transpile error at {e}");
            std::process::exit(1);
        }
    };

    println!(";; Transpiled from Rust by ferrel:\n");
    for form in &forms {
        println!("{}\n", form.render());
    }
}
