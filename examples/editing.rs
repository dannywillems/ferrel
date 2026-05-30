//! Port a real module from a hand-written Emacs configuration to ferrel.
//!
//! This reproduces the core of `editing.el` from the author's modular Emacs
//! setup: mode toggles, a few `setq`s, a `before-save-hook`, and the
//! interactive `my/insert-timestamp` command. It writes the result to
//! `examples/out/editing.el`.
//!
//! Run with: `cargo run --example editing`
//!
//! Note how little of this reaches for the untyped `call` escape hatch: mode
//! toggles use `enable_mode`, the timestamp uses the typed `format_time_string`
//! and `insert`. Each `setq` value is a typed `El<T>`, so a wrong-typed value
//! would not compile.
//!
//! The original uses `use-package` and `with-eval-after-load`, which are macros
//! ferrel does not model yet.

use ferrel::*;

fn main() -> std::io::Result<()> {
    // Section structure lives here, in the Rust source, as ordinary Rust
    // comments. The generated `.el` carries none of it: it is machine output
    // you never read by hand.
    let pkg = Package::new("editing", "General editing enhancements.")
        // Whitespace cleanup on save.
        .form(add_hook("before-save-hook", "whitespace-cleanup"))
        // Revert buffers when files change on disk.
        .form(enable_mode("global-auto-revert-mode"))
        .form(setq("global-auto-revert-non-file-buffers", t()))
        // Electric pair: keep parens balanced.
        .form(setq("electric-pair-preserve-balance", t()))
        // Delete selection on type.
        .form(enable_mode("delete-selection-mode"))
        // Save place: remember cursor position in files.
        .form(enable_mode("save-place-mode"))
        // Recent files.
        .form(enable_mode("recentf-mode"))
        .form(setq("recentf-max-saved-items", int(200)))
        // Timestamp insertion command.
        .defun(
            Defun::new("my/insert-timestamp")
                .doc("Insert current timestamp at point.")
                .interactive()
                .body([insert(format_time_string(string("%Y-%m-%d %H:%M:%S")))]),
        );

    let el = pkg.render();
    println!("{el}");

    let out = format!("{}/examples/out/editing.el", env!("CARGO_MANIFEST_DIR"));
    pkg.write(&out)?;
    eprintln!("wrote {out}");
    Ok(())
}
