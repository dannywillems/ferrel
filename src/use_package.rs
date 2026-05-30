//! A typed builder for `use-package` declarations.
//!
//! `use-package` is the most common idiom in a modern Emacs configuration: a
//! single macro that declares a package and configures its autoloading,
//! keybindings, hooks, customize settings, and setup code. The macro body is a
//! property list whose keywords each have their own, inconsistent splice-vs-wrap
//! shape: `:bind` wants a list of cons cells, `:custom` wants a spliced sequence
//! of `(name value)` lists, `:init`/`:config` want a spliced body of forms, and
//! `:after`/`:commands` want a list of bare symbols. Getting any of these shapes
//! wrong is a load-time error in Emacs, not a compile error.
//!
//! [`UsePackage`] makes those shapes unrepresentable. It reuses the existing
//! typed building blocks end to end: [`KeySeq`] and [`Command`] for `:bind`,
//! [`El<T>`] for `:custom` values (kept typed until lowering), and [`Stmt`] for
//! `:init`/`:config` bodies (built with the [`stmts!`](crate::stmts) macro). The
//! single lowering to [`Sexp`] happens once, in [`UsePackage::build`], which
//! owns the per-keyword splice-vs-wrap rule and the conventional keyword order.

use crate::{
    key::{Command, KeySeq},
    sexp::Sexp,
    stmt::Stmt,
    typed::El,
};

/// How long `use-package` should defer loading the package.
///
/// Maps to the `:defer` keyword: [`Defer::Yes`] emits `:defer t` (load on
/// demand), and [`Defer::Seconds`] emits `:defer N` (load `N` seconds after
/// startup).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Defer {
    /// `:defer t`: load the package lazily, on first use.
    Yes,
    /// `:defer N`: load the package `N` seconds after Emacs starts.
    Seconds(u32),
}

/// One `:hook` entry: a mode to attach to and the function to run.
///
/// Renders as a `(mode . function)` cons cell with bare symbols, as
/// `use-package :hook` expects.
///
/// ```
/// use ferrel::*;
///
/// let h = Hook::new("prog-mode", Command::new("rainbow-delimiters-mode"));
/// let up = UsePackage::new("rainbow-delimiters").hook(h).build();
/// assert!(up.render().contains("(prog-mode . rainbow-delimiters-mode)"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hook {
    mode: String,
    command: Command,
}

impl Hook {
    /// Attach `command` to `mode` (for example `prog-mode`).
    pub fn new(mode: impl Into<String>, command: Command) -> Hook {
        Hook {
            mode: mode.into(),
            command,
        }
    }
}

/// A `use-package` declaration, built fluently and lowered once in
/// [`UsePackage::build`].
///
/// Each keyword is set with its own typed method, so an invalid macro shape
/// cannot be constructed. Only keywords that were set are emitted, and they are
/// emitted in a stable, conventional order regardless of the order the builder
/// methods were called: `:ensure`, `:defer`, `:after`, `:commands`, `:bind`,
/// `:mode`, `:hook`, `:custom`, `:init`, `:config`.
///
/// ```
/// use ferrel::*;
///
/// let up = UsePackage::new("magit")
///     .ensure(true)
///     .defer(Defer::Yes)
///     .bind(kbd!("C-x g"), Command::new("magit-status"))
///     .custom("magit-diff-refine-hunk", t())
///     .init(stmts![typed_setq("magit-define-global-key-bindings", nil())])
///     .config(stmts![message(string("magit loaded"), Vec::<Sexp>::new())])
///     .build();
/// let out = up.render();
/// assert!(out.starts_with("(use-package magit"));
/// assert!(out.contains(":ensure t"));
/// assert!(out.contains("(\"C-x g\" . magit-status)"));
/// ```
pub struct UsePackage {
    name: String,
    ensure: Option<bool>,
    defer: Option<Defer>,
    bindings: Vec<(KeySeq, Command)>,
    hooks: Vec<Hook>,
    custom: Vec<(String, Sexp)>,
    init: Vec<Stmt>,
    config: Vec<Stmt>,
    after: Vec<String>,
    commands: Vec<Command>,
    modes: Vec<(String, Command)>,
}

impl UsePackage {
    /// Start a `use-package` declaration for the package named `name`.
    pub fn new(name: impl Into<String>) -> UsePackage {
        UsePackage {
            name: name.into(),
            ensure: None,
            defer: None,
            bindings: Vec::new(),
            hooks: Vec::new(),
            custom: Vec::new(),
            init: Vec::new(),
            config: Vec::new(),
            after: Vec::new(),
            commands: Vec::new(),
            modes: Vec::new(),
        }
    }

    /// Set `:ensure t` (or `:ensure nil`) to control automatic installation.
    pub fn ensure(mut self, yes: bool) -> UsePackage {
        self.ensure = Some(yes);
        self
    }

    /// Set `:defer` to load the package lazily or after a delay.
    pub fn defer(mut self, defer: Defer) -> UsePackage {
        self.defer = Some(defer);
        self
    }

    /// Add one `:bind` entry: a validated key sequence bound to a command.
    ///
    /// Renders inside `:bind` as a `("key" . command)` cons cell. The key is a
    /// string (use-package calls `kbd` on it internally) and the command is a
    /// bare symbol (use-package `:bind` takes a symbol, not a `#'` form).
    pub fn bind(mut self, key: KeySeq, command: Command) -> UsePackage {
        self.bindings.push((key, command));
        self
    }

    /// Add many `:bind` entries from an iterator of `(key, command)` pairs.
    pub fn binds<I: IntoIterator<Item = (KeySeq, Command)>>(mut self, pairs: I) -> UsePackage {
        self.bindings.extend(pairs);
        self
    }

    /// Add one `:hook` entry, a `(mode . function)` attachment.
    pub fn hook(mut self, hook: Hook) -> UsePackage {
        self.hooks.push(hook);
        self
    }

    /// Add one `:custom` setting: a customize variable and its typed value.
    ///
    /// The value keeps its static type (`El<Bool>`, `El<Int>`, ...) until it is
    /// lowered in [`UsePackage::build`], rendering as a `(name value)` list.
    pub fn custom<T>(mut self, name: impl Into<String>, value: El<T>) -> UsePackage {
        self.custom.push((name.into(), value.into_sexp()));
        self
    }

    /// Set the `:init` body: forms run before the package is loaded.
    ///
    /// The body is any iterator of values convertible into [`Stmt`], so it
    /// reuses [`stmts!`](crate::stmts) and the same statement plumbing as
    /// [`Defun::body`](crate::Defun::body). Each form is spliced after `:init`
    /// (a body, not a single wrapped form).
    pub fn init<S: Into<Stmt>, I: IntoIterator<Item = S>>(mut self, body: I) -> UsePackage {
        self.init = body.into_iter().map(Into::into).collect();
        self
    }

    /// Set the `:config` body: forms run after the package is loaded.
    ///
    /// Like [`UsePackage::init`], the body is built from anything convertible
    /// into [`Stmt`] and is spliced after `:config`.
    pub fn config<S: Into<Stmt>, I: IntoIterator<Item = S>>(mut self, body: I) -> UsePackage {
        self.config = body.into_iter().map(Into::into).collect();
        self
    }

    /// Add an `:after` feature: defer loading until `feature` has loaded.
    pub fn after(mut self, feature: impl Into<String>) -> UsePackage {
        self.after.push(feature.into());
        self
    }

    /// Add a `:commands` autoload entry for `command`.
    pub fn command(mut self, command: Command) -> UsePackage {
        self.commands.push(command);
        self
    }

    /// Add a `:mode` entry: associate a filename pattern with a command.
    ///
    /// Renders inside `:mode` as a `("pattern" . command)` cons cell, mirroring
    /// an `auto-mode-alist` entry.
    pub fn mode(mut self, pattern: impl Into<String>, command: Command) -> UsePackage {
        self.modes.push((pattern.into(), command));
        self
    }

    /// Lower the declaration to a single `(use-package NAME ...)` s-expression.
    ///
    /// This is the only place the typed inputs become [`Sexp`]. Keywords are
    /// emitted in the conventional order and only when set; each keyword's
    /// splice-vs-wrap shape is applied here so the output is always a valid
    /// `use-package` form.
    pub fn build(self) -> Sexp {
        let mut form = vec![Sexp::sym("use-package"), Sexp::sym(self.name)];

        // :ensure t / :ensure nil
        if let Some(yes) = self.ensure {
            form.push(Sexp::keyword("ensure"));
            form.push(if yes { Sexp::True } else { Sexp::Nil });
        }

        // :defer t / :defer N
        if let Some(defer) = self.defer {
            form.push(Sexp::keyword("defer"));
            form.push(match defer {
                Defer::Yes => Sexp::True,
                Defer::Seconds(n) => Sexp::Int(i64::from(n)),
            });
        }

        // :after (f ...) -- a list of bare feature symbols.
        if !self.after.is_empty() {
            form.push(Sexp::keyword("after"));
            form.push(Sexp::List(self.after.into_iter().map(Sexp::Sym).collect()));
        }

        // :commands (c1 c2 ...) -- a list of bare command symbols.
        if !self.commands.is_empty() {
            form.push(Sexp::keyword("commands"));
            form.push(Sexp::List(
                self.commands
                    .into_iter()
                    .map(|c| Sexp::Sym(c.into_string()))
                    .collect(),
            ));
        }

        // :bind (("key" . cmd) ...) -- a list of cons cells, key as a string.
        if !self.bindings.is_empty() {
            form.push(Sexp::keyword("bind"));
            form.push(Sexp::List(
                self.bindings.into_iter().map(bind_pair).collect(),
            ));
        }

        // :mode (("pattern" . cmd) ...) -- a list of cons cells.
        if !self.modes.is_empty() {
            form.push(Sexp::keyword("mode"));
            form.push(Sexp::List(self.modes.into_iter().map(mode_pair).collect()));
        }

        // :hook ((mode . fn) ...) -- a list of cons cells, bare symbols.
        if !self.hooks.is_empty() {
            form.push(Sexp::keyword("hook"));
            form.push(Sexp::List(self.hooks.into_iter().map(hook_pair).collect()));
        }

        // :custom ((var value) ...) -- a list of (name value) lists.
        if !self.custom.is_empty() {
            form.push(Sexp::keyword("custom"));
            form.push(Sexp::List(
                self.custom.into_iter().map(custom_pair).collect(),
            ));
        }

        // :init body... -- the body is spliced, not wrapped.
        if !self.init.is_empty() {
            form.push(Sexp::keyword("init"));
            form.extend(self.init.into_iter().map(Stmt::into_sexp));
        }

        // :config body... -- the body is spliced, not wrapped.
        if !self.config.is_empty() {
            form.push(Sexp::keyword("config"));
            form.extend(self.config.into_iter().map(Stmt::into_sexp));
        }

        Sexp::List(form)
    }
}

impl From<UsePackage> for Sexp {
    fn from(up: UsePackage) -> Sexp {
        up.build()
    }
}

/// Lower one `:bind` entry to `("key" . command)`.
///
/// The car is the validated key sequence as a string (use-package applies
/// `kbd` itself); the cdr is the command as a bare symbol (not a `#'` form).
fn bind_pair((key, command): (KeySeq, Command)) -> Sexp {
    Sexp::Dotted(
        vec![Sexp::Str(key.into_string())],
        Box::new(Sexp::Sym(command.into_string())),
    )
}

/// Lower one `:mode` entry to `("pattern" . command)`.
fn mode_pair((pattern, command): (String, Command)) -> Sexp {
    Sexp::Dotted(
        vec![Sexp::Str(pattern)],
        Box::new(Sexp::Sym(command.into_string())),
    )
}

/// Lower one `:hook` entry to `(mode . function)` with bare symbols.
fn hook_pair(hook: Hook) -> Sexp {
    Sexp::Dotted(
        vec![Sexp::Sym(hook.mode)],
        Box::new(Sexp::Sym(hook.command.into_string())),
    )
}

/// Lower one `:custom` entry to a `(name value)` list (not a dotted pair).
fn custom_pair((name, value): (String, Sexp)) -> Sexp {
    Sexp::List(vec![Sexp::Sym(name), value])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        parser::parse_one,
        typed::{int, message, nil, string, t},
    };

    #[test]
    fn empty_block_is_just_the_form_head() {
        let up = UsePackage::new("magit").build();
        assert_eq!(up.render(), "(use-package magit)");
    }

    #[test]
    fn ensure_and_defer_render() {
        let out = UsePackage::new("magit")
            .ensure(true)
            .defer(Defer::Yes)
            .build()
            .render();
        assert!(out.contains(":ensure t"));
        assert!(out.contains(":defer t"));
    }

    #[test]
    fn ensure_nil_and_defer_seconds() {
        let out = UsePackage::new("flycheck")
            .ensure(false)
            .defer(Defer::Seconds(2))
            .build()
            .render();
        assert!(out.contains(":ensure nil"));
        assert!(out.contains(":defer 2"));
    }

    #[test]
    fn bind_reuses_keyseq_and_command() {
        let out = UsePackage::new("magit")
            .bind(crate::kbd!("C-x g"), Command::new("magit-status"))
            .bind(crate::kbd!("C-c g b"), Command::new("magit-blame"))
            .build()
            .render();
        // Cons cell with the key as a STRING and the command as a bare symbol.
        assert!(out.contains("(\"C-x g\" . magit-status)"));
        assert!(out.contains("(\"C-c g b\" . magit-blame)"));
        assert!(!out.contains("#'magit-status"));
    }

    #[test]
    fn binds_takes_an_iterator() {
        let pairs = vec![
            (crate::kbd!("C-c a"), Command::new("cmd-a")),
            (crate::kbd!("C-c b"), Command::new("cmd-b")),
        ];
        let out = UsePackage::new("demo").binds(pairs).build().render();
        assert!(out.contains("(\"C-c a\" . cmd-a)"));
        assert!(out.contains("(\"C-c b\" . cmd-b)"));
    }

    #[test]
    fn hook_pairs_are_bare_symbols() {
        let out = UsePackage::new("magit")
            .hook(Hook::new("prog-mode", Command::new("magit-todos-mode")))
            .hook(Hook::new("text-mode", Command::new("flyspell-mode")))
            .build()
            .render();
        assert!(out.contains("(prog-mode . magit-todos-mode)"));
        assert!(out.contains("(text-mode . flyspell-mode)"));
    }

    #[test]
    fn custom_entries_are_lists_keeping_typed_values() {
        let out = UsePackage::new("magit")
            .custom("magit-diff-refine-hunk", t())
            .custom("magit-save-repository-buffers", nil())
            .custom("magit-log-margin-width", int(40))
            .build()
            .render();
        assert!(out.contains("(magit-diff-refine-hunk t)"));
        assert!(out.contains("(magit-save-repository-buffers nil)"));
        assert!(out.contains("(magit-log-margin-width 40)"));
    }

    #[test]
    fn init_and_config_splice_bodies() {
        let out = UsePackage::new("magit")
            .init(crate::stmts![
                crate::typed::setq("a", nil()),
                crate::typed::setq("b", t()),
            ])
            .config(crate::stmts![message(string("loaded"), Vec::<Sexp>::new())])
            .build()
            .render();
        assert!(out.contains(":init"));
        assert!(out.contains("(setq a nil)"));
        assert!(out.contains("(setq b t)"));
        assert!(out.contains(":config"));
        assert!(out.contains("(message \"loaded\")"));
    }

    #[test]
    fn after_and_commands_are_symbol_lists() {
        let out = UsePackage::new("magit")
            .after("project")
            .after("seq")
            .command(Command::new("magit-status"))
            .command(Command::new("magit-blame"))
            .build()
            .render();
        assert!(out.contains(":after (project seq)"));
        assert!(out.contains(":commands (magit-status magit-blame)"));
    }

    #[test]
    fn mode_entries_are_cons_cells() {
        let out = UsePackage::new("rust-mode")
            .mode("\\.rs\\'", Command::new("rust-mode"))
            .build()
            .render();
        assert!(out.contains("(\"\\\\.rs\\\\'\" . rust-mode)"));
    }

    #[test]
    fn keyword_order_is_stable_regardless_of_call_order() {
        // Call the setters in a scrambled order; the output must still be in
        // the conventional order: ensure, defer, after, commands, bind, mode,
        // hook, custom, init, config.
        let out = UsePackage::new("magit")
            .config(crate::stmts![message(string("done"), Vec::<Sexp>::new())])
            .custom("v", t())
            .bind(crate::kbd!("C-x g"), Command::new("magit-status"))
            .defer(Defer::Yes)
            .ensure(true)
            .build()
            .render();
        let ensure = out.find(":ensure").unwrap();
        let defer = out.find(":defer").unwrap();
        let bind = out.find(":bind").unwrap();
        let custom = out.find(":custom").unwrap();
        let config = out.find(":config").unwrap();
        assert!(ensure < defer);
        assert!(defer < bind);
        assert!(bind < custom);
        assert!(custom < config);
    }

    #[test]
    fn into_sexp_lets_package_form_accept_it() {
        // `From<UsePackage> for Sexp` must exist so Package::form(up) works.
        let up = UsePackage::new("magit").ensure(true);
        let s: Sexp = up.into();
        assert!(s.render().contains("(use-package magit"));
    }

    #[test]
    fn round_trips_through_parser() {
        // build() emits only existing Sexp variants, so parse(render(form))
        // must reproduce an equal AST.
        let form = UsePackage::new("magit")
            .ensure(true)
            .defer(Defer::Seconds(1))
            .after("project")
            .command(Command::new("magit-status"))
            .bind(crate::kbd!("C-x g"), Command::new("magit-status"))
            .mode("\\.gitconfig\\'", Command::new("magit-mode"))
            .hook(Hook::new("prog-mode", Command::new("magit-todos-mode")))
            .custom("magit-diff-refine-hunk", t())
            .init(crate::stmts![crate::typed::setq("a", nil())])
            .config(crate::stmts![message(string("loaded"), Vec::<Sexp>::new())])
            .build();
        let rendered = form.render();
        let reparsed = parse_one(&rendered).unwrap();
        assert_eq!(form, reparsed, "round-trip mismatch:\n{rendered}");
    }
}
