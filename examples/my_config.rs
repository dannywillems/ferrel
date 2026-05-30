//! A realistic chunk of an Emacs configuration written with ferrel, pushing
//! the public API the way an ordinary Rust user would: computing values with
//! plain Rust (a `Vec` of keybindings, iterators, `format!`, a small loop) and
//! feeding the results into ferrel forms.
//!
//! Run with: `cargo run --example my_config`
//!
//! This example is also where I (a Rust user, not an Emacs user) probe the
//! ergonomics of the typed surface. Where I had to reach for the untyped
//! `call` escape hatch, or where I had to fight the builder, is called out in
//! comments prefixed with `FRICTION:` so the API feedback is anchored in real
//! code.

use ferrel::*;

/// A keybinding I want to compute in Rust and then emit. Nothing
/// ferrel-specific: this is the kind of plain data a user already has lying
/// around in their own config code.
struct Binding {
    key: &'static str,
    command: &'static str,
}

fn main() -> std::io::Result<()> {
    // ---- Compute configuration values with ordinary Rust ----------------

    // A table of keybindings. In a real config this might come from a file,
    // an env var, or be generated. Here it is just a Vec I iterate over.
    let bindings = vec![
        Binding {
            key: "C-c f",
            command: "find-file",
        },
        Binding {
            key: "C-c b",
            command: "switch-to-buffer",
        },
        Binding {
            key: "C-c k",
            command: "kill-buffer",
        },
        Binding {
            key: "C-c g",
            command: "my/insert-greeting",
        },
    ];

    // A list of minor modes to enable, again as plain Rust data.
    let modes_on = ["global-auto-revert-mode", "save-place-mode", "recentf-mode"];

    // Compute a numeric value with real Rust arithmetic, then lift it into a
    // typed El<Int>. This is the "compute in Rust, feed into ferrel" flow.
    let recent_items: i64 = (0..4).map(|_| 50_i64).sum(); // 200
    let fill_column: i64 = 80;

    // Build a greeting string with std formatting, then lift it with `string`.
    let user = std::env::var("USER").unwrap_or_else(|_| "stranger".to_string());
    let greeting = format!("Hello, {user}, from Rust-generated Elisp!");

    // ---- Assemble the package -------------------------------------------

    let mut pkg = Package::new("my-config", "A slice of my Emacs config, in Rust.")
        .author("ferrel user")
        .version("0.1.0")
        .emacs_version("28.1")
        .keyword("convenience")
        .form(setq("fill-column", int(fill_column)))
        .form(setq("recentf-max-saved-items", int(recent_items)))
        // A customizable greeting computed above.
        .form(defcustom(
            "my/greeting",
            string(greeting),
            "Greeting shown by `my/insert-greeting'.",
            "string",
            "my-config",
        ));

    // Enable each mode by iterating the plain-Rust list. `Package` is consumed
    // and returned by `form`, so I rebind `pkg` in the loop.
    for mode in modes_on {
        pkg = pkg.form(enable_mode(mode));
    }

    // The interactive command that inserts the greeting at point.
    pkg = pkg.defun(
        Defun::new("my/insert-greeting")
            .doc("Insert the configured greeting at point.")
            .interactive()
            .body([insert(var::<Str>("my/greeting"))]),
    );

    // A command whose body mixes effect types: a `message`, then an `insert`.
    //
    // FRICTION (High): `Defun::body` is `body<T, I: IntoIterator<Item = El<T>>>`,
    // so every element of a single `.body([...])` call must share one `T`.
    // `message(...)` is `El<Str>` and `insert(...)` is `El<Any>`, so passing
    // them in the same array does NOT type-check. A statement body is a
    // *sequence of effects* whose individual result types are irrelevant, yet
    // the type system forces me to erase them by hand. My workarounds below
    // are both ugly:
    //   1. wrap each expr in `progn([... .into_sexp()])` -> back to Sexp anyway,
    //   2. call `.stmt(expr.into_sexp())` once per statement.
    // I use (2). A `Defun::stmt`-only body reads like assembly, not Rust.
    pkg = pkg.defun(
        Defun::new("my/announce")
            .doc("Announce, then insert, the greeting.")
            .interactive()
            .stmt(message(var::<Str>("my/greeting"), []).into_sexp())
            .stmt(insert(var::<Str>("my/greeting")).into_sexp()),
    );

    // A pure typed-arithmetic helper, to show the compiler does check numbers.
    // (a + b) * 2 -- swapping any of these for a `string(...)` fails to compile.
    pkg = pkg.defun(
        Defun::new("my/double-sum")
            .doc("Return twice the sum of A and B.")
            .param("a")
            .param("b")
            .body([mul(add(var::<Int>("a"), var::<Int>("b")), int(2))]),
    );

    // Bind every key from the computed table.
    //
    // FRICTION (Medium): `global_set_key` takes `impl Into<String>` for the
    // key, so I can pass `b.key` directly, which is nice. But there is no
    // typed `KeySequence` newtype: "C-c f" and a typo like "C-x C-" are the
    // same type, so a malformed key sequence is not caught until Emacs loads
    // the file. For a crate whose pitch is "catch mistakes at compile time",
    // the keybinding DSL is the place I most expected a typed wrapper.
    for b in &bindings {
        pkg = pkg.form(global_set_key(b.key, b.command));
    }

    // ---- A few escape-hatch calls a real user hits immediately ----------

    // FRICTION (High): none of these common builtins are typed, so I drop to
    // `call`. Each one also forces me to hand-build `Sexp` arguments rather
    // than typed `El<T>`, because `call` takes `IntoIterator<Item = Sexp>`,
    // not `Item = El<T>`. So I cannot pass `string("...")` directly; I must
    // write `string("...").into_sexp()`. That conversion noise is everywhere.
    pkg = pkg
        // (set-frame-font "Monospace 12" t t)
        .form(
            call(
                "set-frame-font",
                [
                    string("Monospace 12").into_sexp(),
                    t().into_sexp(),
                    t().into_sexp(),
                ],
            )
            .into_sexp(),
        )
        // (add-to-list 'auto-mode-alist '("\\.rs\\'" . rust-mode))
        // I cannot express the cons cell or the quoted list with typed helpers,
        // so this is fully untyped Sexp via parse_one of a literal string.
        .form(
            parse_one(r#"(add-to-list 'auto-mode-alist '("\\.rs\\'" . rust-mode))"#)
                .expect("literal elisp parses"),
        );

    let el = pkg.render();
    println!("{el}");

    let out = format!("{}/examples/out/my-config.el", env!("CARGO_MANIFEST_DIR"));
    pkg.write(&out)?;
    eprintln!("wrote {out}");
    Ok(())
}
