//! Express a realistic `use-package` block with the typed [`UsePackage`]
//! builder (round 2). Compare with `use_package_attempt.rs`, which hand-builds
//! the same shape out of raw `Sexp` and flags the friction inline.
//!
//! Run with: `cargo run --example use_package`
//!
//! The builder reuses the existing typed building blocks end to end:
//! [`KeySeq`] + [`Command`] for `:bind` (validated key sequences), [`El<T>`]
//! for `:custom` values (kept typed until lowering), and [`Stmt`] / `stmts!`
//! for the `:init` and `:config` bodies. The per-keyword splice-vs-wrap rule
//! and the conventional keyword order live in `build()`, so an invalid macro
//! shape cannot be constructed.

use ferrel::*;

fn main() -> std::io::Result<()> {
    // The keybindings, as ordinary Rust data. Each key is validated when the
    // `KeySeq` is built (via `kbd!`), and the `Command` newtype keeps the key
    // and the command from being swapped.
    let bindings = [
        (kbd!("C-x g"), Command::new("magit-status")),
        (kbd!("C-c g b"), Command::new("magit-blame")),
    ];

    // One typed `(use-package magit ...)` declaration. The setters may be
    // called in any order; `build()` emits the keywords in the conventional
    // order: :ensure, :defer, :after, :commands, :bind, :mode, :hook, :custom,
    // :init, :config.
    let magit = UsePackage::new("magit")
        .ensure(true)
        .defer(Defer::Yes)
        .after("project")
        .binds(bindings)
        .hook(Hook::new("prog-mode", Command::new("magit-todos-mode")))
        .custom("magit-diff-refine-hunk", t())
        .custom("magit-save-repository-buffers", nil())
        .init(stmts![typed_setq(
            "magit-define-global-key-bindings",
            nil()
        )])
        .config(stmts![message(string("magit loaded"), Vec::<Sexp>::new())]);

    // A second declaration showing `:commands` autoloads and a `:mode`
    // filename association.
    let rust_mode = UsePackage::new("rust-mode")
        .ensure(true)
        .defer(Defer::Seconds(1))
        .command(Command::new("rust-mode"))
        .mode("\\.rs\\'", Command::new("rust-mode"))
        .config(stmts![typed_setq("rust-format-on-save", t())]);

    let pkg = Package::new("my-use-package-config", "use-package blocks, typed.")
        .emacs_version("28.1")
        .form(require("use-package"))
        .form(magit)
        .form(rust_mode);

    let el = pkg.render();
    println!("{el}");

    let out = format!(
        "{}/examples/out/ferrel-use-package.el",
        env!("CARGO_MANIFEST_DIR")
    );
    pkg.write(&out)?;
    eprintln!("wrote {out}");
    Ok(())
}
