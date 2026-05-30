//! Attempt to express a realistic `use-package` block using ONLY the current
//! public ferrel API (round 2). The goal is to feel where the API stops being
//! Rust and turns into hand-assembled `Sexp`.
//!
//! Run with: `cargo run --example use_package_attempt`
//!
//! `use-package` is the single most common idiom in a real Emacs config. A
//! typical block looks like:
//!
//! ```elisp
//! (use-package magit
//!   :ensure t
//!   :defer t
//!   :bind (("C-x g" . magit-status)
//!          ("C-c g b" . magit-blame))
//!   :hook (prog-mode . magit-todos-mode)
//!   :custom (magit-diff-refine-hunk t)
//!           (magit-save-repository-buffers nil)
//!   :init (setq magit-define-global-key-bindings nil)
//!   :config (message "magit loaded"))
//! ```
//!
//! ferrel has no `use-package` builder, so every part below is hand-built out
//! of raw `Sexp` nodes. Friction is flagged inline with `FRICTION:` comments so
//! the API feedback is anchored in real code.

use ferrel::*;

/// A keybinding I want inside the `:bind` block. Plain Rust data; in a real
/// config this is what a user already has.
struct Binding {
    key: &'static str,
    command: &'static str,
}

/// Build one `("key" . command)` cons cell for the `:bind` list.
///
/// FRICTION (High): there is no typed representation of a `:bind` pair. A
/// keybinding is conceptually `(KeySeq . Command)`, and ferrel already HAS both
/// `KeySeq` and `Command` newtypes plus `bind_key`. But `bind_key` only emits a
/// top-level `(global-set-key ...)` form; it cannot produce the `("key" .
/// cmd)` cons cell that `use-package :bind` wants. So I have to drop all the
/// way down to `Sexp::Dotted`, re-quote the kbd string by hand, and rebuild the
/// `#'`-free symbol myself. The KeySeq validation I get for free elsewhere is
/// unavailable here unless I call it manually.
fn bind_pair(b: &Binding) -> Sexp {
    // Validate the key with the typed newtype, then throw the type away because
    // the cons cell wants a raw string. The newtype buys nothing structural
    // here; it is just a manual assertion I have to remember to make.
    let key = kbd!(b.key);
    Sexp::Dotted(
        vec![Sexp::Str(key.into_string())],
        Box::new(Sexp::sym(b.command)),
    )
}

/// Build one `(var value)` pair for the `:custom` block.
///
/// FRICTION (High): `:custom` settings are conceptually the same data as a
/// `Defcustom` (a name and a typed value), but there is no way to reuse that.
/// `setq` / `Defcustom` both emit whole top-level forms, never the bare
/// `(name value)` pair that lives inside `:custom`. So the typed `El<T>` value
/// is built, then immediately lowered to `Sexp` and stuffed into a list by
/// hand. The type of the value (`El<Bool>` vs `El<Int>`) is lost the instant it
/// enters this list.
fn custom_pair<T>(name: &str, value: El<T>) -> Sexp {
    Sexp::List(vec![Sexp::sym(name), value.into_sexp()])
}

fn main() -> std::io::Result<()> {
    // ---- The data, in ordinary Rust -------------------------------------

    let package_name = "magit";

    let bindings = [
        Binding {
            key: "C-x g",
            command: "magit-status",
        },
        Binding {
            key: "C-c g b",
            command: "magit-blame",
        },
    ];

    // ---- Hand-assemble the (use-package ...) form -----------------------

    // FRICTION (Critical): everything from here down is me being the compiler.
    // I build a flat Vec<Sexp> and push the macro keywords and their payloads
    // in the exact textual order Emacs expects. The Vec is heterogeneous,
    // untyped, and order-sensitive. There is no type that says "this is a
    // use-package block"; it is just a list I hope is shaped right.
    let mut form: Vec<Sexp> = vec![Sexp::sym("use-package"), Sexp::sym(package_name)];

    // :ensure t
    //
    // FRICTION (Medium): `:ensure` is a boolean flag, but I express it as two
    // separate pushes (the keyword, then the value). Nothing stops me from
    // pushing the keyword and forgetting the value, or pushing two values, or
    // misspelling "ensure" as a bare symbol. The plist invariant (alternating
    // keyword/value) is entirely on me.
    form.push(Sexp::keyword("ensure"));
    form.push(Sexp::True);

    // :defer t
    form.push(Sexp::keyword("defer"));
    form.push(Sexp::True);

    // :bind (("C-x g" . magit-status) ("C-c g b" . magit-blame))
    //
    // FRICTION (High): the value of `:bind` is itself a list of cons cells. I
    // map my Rust Vec into `Sexp::Dotted` nodes (see `bind_pair`) and wrap them
    // in one more `Sexp::List`. Two levels of manual list nesting, and the
    // KeySeq/Command newtypes that exist in the crate cannot flow through here.
    form.push(Sexp::keyword("bind"));
    form.push(Sexp::List(bindings.iter().map(bind_pair).collect()));

    // :hook (prog-mode . magit-todos-mode)
    //
    // FRICTION (High): `add_hook` exists and is typed-ish, but it emits a
    // top-level `(add-hook 'h #'f)` form, not the `(mode . fn)` cons that
    // `:hook` wants. So again I hand-build a dotted pair. The hook name and the
    // function name are both bare `&str` with no validation and no distinct
    // types, so I can swap them by accident.
    form.push(Sexp::keyword("hook"));
    form.push(Sexp::Dotted(
        vec![Sexp::sym("prog-mode")],
        Box::new(Sexp::sym("magit-todos-mode")),
    ));

    // :custom (magit-diff-refine-hunk t) (magit-save-repository-buffers nil)
    //
    // FRICTION (High): `:custom` takes a *sequence* of `(name value)` pairs,
    // spliced directly after the keyword (NOT wrapped in an outer list, unlike
    // `:bind`). So the calling convention differs from `:bind` for no reason I
    // can see from the Rust side, and I have to remember which keyword splices
    // and which wraps. I build each pair with `custom_pair`, then push them one
    // by one. Get the wrapping wrong and Emacs errors at load time, not here.
    form.push(Sexp::keyword("custom"));
    form.push(custom_pair("magit-diff-refine-hunk", t()));
    form.push(custom_pair("magit-save-repository-buffers", nil()));

    // :init (setq magit-define-global-key-bindings nil)
    //
    // FRICTION (Medium): `:init` and `:config` take a BODY: a sequence of forms
    // run before / after load. This is exactly a `stmts!` / `Defun::body`
    // situation, but there is no plumbing to feed a `Vec<Stmt>` into a
    // use-package body. A single form can be pushed directly; more than one
    // would have to be hand-wrapped or splice-pushed, and `:init (a b)` is
    // WRONG in elisp (it would call `a`), so multiple forms must each be a
    // separate top-level element after `:init`. I have to know that.
    form.push(Sexp::keyword("init"));
    form.push(typed_setq("magit-define-global-key-bindings", nil()).into_sexp());

    // :config (message "magit loaded")
    form.push(Sexp::keyword("config"));
    form.push(message(string("magit loaded"), Vec::<Sexp>::new()).into_sexp());

    let use_package = Sexp::List(form);

    // ---- Drop it into a package -----------------------------------------

    // `use-package` itself must be required (or it is a build-time autoload).
    let pkg = Package::new("my-use-package-config", "use-package block, hand-built.")
        .emacs_version("28.1")
        .form(require("use-package"))
        .form(use_package);

    let el = pkg.render();
    println!("{el}");

    let out = format!(
        "{}/examples/out/use-package-attempt.el",
        env!("CARGO_MANIFEST_DIR")
    );
    pkg.write(&out)?;
    eprintln!("wrote {out}");
    Ok(())
}
