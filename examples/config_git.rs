//! Step-by-step port of `modules/git.el` from a real Emacs config to ferrel.
//!
//! This shows moving a declarative `use-package` module to Rust with the typed
//! `UsePackage` builder. The clean parts (hooks, `:after`, simple `:config`
//! settings) map directly. The parts the builder does not model yet are called
//! out inline with `GAP:` so the migration is honest about what still needs the
//! escape hatch or new builder support.
//!
//! Run with: `cargo run --example config_git`

use ferrel::*;

fn main() -> std::io::Result<()> {
    let pkg = Package::new("git", "Git integration, ported from git.el.")
        // magit: full git porcelain. The simple settings port directly; the
        // quoted alist and the `with-eval-after-load` blame-styles block are
        // GAPs (no builder support for nested macros or large quoted data yet),
        // so they are emitted verbatim with `raw_form`.
        .form(
            UsePackage::new("magit")
                .config([
                    setq("magit-diff-expansion-threshold", float(0.1)),
                    setq(
                        "magit-display-buffer-function",
                        var::<Any>("#'magit-display-buffer-same-window-except-diff-v1"),
                    ),
                    // GAP: a quoted alist of (path . depth). Built as raw Sexp
                    // because the builder has no typed alist literal yet.
                    raw_setq_alist(),
                ])
                .build(),
        )
        // forge: GitHub PRs and issues. `:after` and a `:config` setq with a
        // computed path map cleanly.
        .form(
            UsePackage::new("forge")
                .after("magit")
                .config([setq(
                    "forge-database-file",
                    call(
                        "expand-file-name",
                        args![
                            string("forge-database.sqlite"),
                            var::<Any>("user-emacs-directory")
                        ],
                    )
                    .cast::<Str>(),
                )])
                .build(),
        )
        // magit-delta: per-language diff highlighting.
        // GAP: `:if (executable-find "delta")` has no builder method yet, so
        // only the `:after` and `:hook` are expressed here.
        .form(
            UsePackage::new("magit-delta")
                .after("magit")
                .hook(Hook::new("magit-mode", Command::new("magit-delta-mode")))
                .build(),
        )
        // diff-hl: highlight uncommitted changes in the fringe. Ports cleanly.
        .form(
            UsePackage::new("diff-hl")
                .hook(Hook::new("prog-mode", Command::new("diff-hl-mode")))
                .hook(Hook::new(
                    "magit-post-refresh",
                    Command::new("diff-hl-magit-post-refresh"),
                ))
                .build(),
        );

    let el = pkg.render();
    println!("{el}");
    let out = format!("{}/examples/out/git.el", env!("CARGO_MANIFEST_DIR"));
    pkg.write(&out)?;
    eprintln!("wrote {out}");
    Ok(())
}

/// Build `(setq magit-repository-directories '(("~/codes/" . 2)))` as raw Sexp.
///
/// GAP: there is no typed quoted-alist literal in the builder surface yet, so
/// this is assembled from `Sexp` nodes directly. A future `alist!`-style helper
/// would remove the need.
fn raw_setq_alist() -> Sexp {
    let entry = Sexp::Dotted(
        vec![Sexp::Str("~/codes/".to_string())],
        Box::new(Sexp::Int(2)),
    );
    let alist = Sexp::List(vec![entry]).quoted();
    Sexp::call("setq", [Sexp::sym("magit-repository-directories"), alist])
}
