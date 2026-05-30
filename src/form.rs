//! Top-level Elisp forms: the things that appear at the toplevel of a `.el`
//! file. These build [`Sexp`] trees directly (they are statements, not typed
//! expressions, so they are not wrapped in [`El`](crate::El)).

use crate::{
    key::{Command, KeySeq},
    sexp::Sexp,
    stmt::Stmt,
    typed::El,
};

/// How a command consumes its `(interactive)` arguments.
enum Interactive {
    /// `(interactive)`, no argument spec.
    Plain,
    /// `(interactive "spec")`, a raw arg descriptor string.
    Spec(String),
}

/// A function definition: `(defun name (args) "doc" (interactive ...) body...)`.
///
/// Built fluently, then lowered with [`Defun::build`] or pushed straight into a
/// [`Package`](crate::Package) with [`Package::defun`](crate::Package::defun).
pub struct Defun {
    name: String,
    params: Vec<String>,
    doc: Option<String>,
    interactive: Option<Interactive>,
    body: Vec<Sexp>,
}

impl Defun {
    /// Start a definition named `name`.
    pub fn new(name: impl Into<String>) -> Defun {
        Defun {
            name: name.into(),
            params: Vec::new(),
            doc: None,
            interactive: None,
            body: Vec::new(),
        }
    }

    /// Add a positional parameter. Read it in the body with
    /// [`var`](crate::var)`::<T>("name")`.
    pub fn param(mut self, name: impl Into<String>) -> Defun {
        self.params.push(name.into());
        self
    }

    /// Set the docstring.
    pub fn doc(mut self, text: impl Into<String>) -> Defun {
        self.doc = Some(text.into());
        self
    }

    /// Mark the function as an interactive command: `(interactive)`.
    pub fn interactive(mut self) -> Defun {
        self.interactive = Some(Interactive::Plain);
        self
    }

    /// Mark the function interactive with an argument spec, e.g. `"p"` for a
    /// numeric prefix: `(interactive "p")`.
    pub fn interactive_spec(mut self, spec: impl Into<String>) -> Defun {
        self.interactive = Some(Interactive::Spec(spec.into()));
        self
    }

    /// Set the body from a sequence of statements (each is evaluated in
    /// sequence; only the last value is returned).
    ///
    /// The element type is anything convertible into a [`Stmt`], which includes
    /// every [`El<T>`] regardless of `T` and a raw [`Sexp`]. A homogeneous
    /// `.body([expr, expr])` of one `El<T>` still compiles; for a body mixing
    /// result types (for example a `message` then an `insert`), build it with
    /// the [`stmts!`](crate::stmts) macro.
    ///
    /// ```
    /// use ferrel::*;
    ///
    /// let f = Defun::new("demo")
    ///     .body(stmts![
    ///         message(string("hi"), Vec::<Sexp>::new()),
    ///         insert(string("x")),
    ///     ])
    ///     .build();
    /// assert!(f.render().contains("(insert \"x\")"));
    /// ```
    pub fn body<S: Into<Stmt>, I: IntoIterator<Item = S>>(mut self, body: I) -> Defun {
        self.body = body.into_iter().map(|s| s.into().into_sexp()).collect();
        self
    }

    /// Append a single raw statement to the body.
    pub fn stmt(mut self, stmt: Sexp) -> Defun {
        self.body.push(stmt);
        self
    }

    /// Lower to a `defun` s-expression.
    pub fn build(self) -> Sexp {
        let mut form = vec![
            Sexp::sym("defun"),
            Sexp::sym(self.name),
            Sexp::List(self.params.into_iter().map(Sexp::Sym).collect()),
        ];
        if let Some(doc) = self.doc {
            form.push(Sexp::Str(doc));
        }
        match self.interactive {
            Some(Interactive::Plain) => form.push(Sexp::call::<[Sexp; 0]>("interactive", [])),
            Some(Interactive::Spec(spec)) => {
                form.push(Sexp::call("interactive", [Sexp::Str(spec)]))
            }
            None => {}
        }
        form.extend(self.body);
        Sexp::List(form)
    }
}

impl From<Defun> for Sexp {
    fn from(d: Defun) -> Sexp {
        d.build()
    }
}

/// `(defvar name value "doc")`.
pub fn defvar<T>(name: impl Into<String>, value: El<T>, doc: impl Into<String>) -> Sexp {
    Sexp::List(vec![
        Sexp::sym("defvar"),
        Sexp::sym(name),
        value.into_sexp(),
        Sexp::Str(doc.into()),
    ])
}

/// `(defconst name value "doc")`.
pub fn defconst<T>(name: impl Into<String>, value: El<T>, doc: impl Into<String>) -> Sexp {
    Sexp::List(vec![
        Sexp::sym("defconst"),
        Sexp::sym(name),
        value.into_sexp(),
        Sexp::Str(doc.into()),
    ])
}

/// A `defcustom` customize `:type`.
///
/// These map to the common Emacs customize type symbols. Using an enum instead
/// of a free-form string removes the chance of a typo such as `"strnig"` and
/// documents the supported set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomType {
    /// `'string`.
    String,
    /// `'integer`.
    Integer,
    /// `'number`.
    Number,
    /// `'boolean`.
    Boolean,
    /// `'hook`.
    Hook,
    /// `'symbol`.
    Symbol,
    /// `'file`.
    File,
    /// `'directory`.
    Directory,
    /// `'list`.
    List,
}

impl CustomType {
    /// The customize type symbol name, without the leading quote.
    pub fn symbol(self) -> &'static str {
        match self {
            CustomType::String => "string",
            CustomType::Integer => "integer",
            CustomType::Number => "number",
            CustomType::Boolean => "boolean",
            CustomType::Hook => "hook",
            CustomType::Symbol => "symbol",
            CustomType::File => "file",
            CustomType::Directory => "directory",
            CustomType::List => "list",
        }
    }
}

/// `(defcustom name value "doc" :type 'type :group 'group)`.
///
/// `el_type` is a quoted customize type symbol such as `string`, `integer`,
/// `boolean`, or `hook`. For a checked type and a builder that avoids the
/// positional-argument ordering hazard, see [`CustomType`] and [`Defcustom`].
pub fn defcustom<T>(
    name: impl Into<String>,
    value: El<T>,
    doc: impl Into<String>,
    el_type: impl Into<String>,
    group: impl Into<String>,
) -> Sexp {
    Sexp::List(vec![
        Sexp::sym("defcustom"),
        Sexp::sym(name),
        value.into_sexp(),
        Sexp::Str(doc.into()),
        Sexp::keyword("type"),
        Sexp::sym(el_type).quoted(),
        Sexp::keyword("group"),
        Sexp::sym(group).quoted(),
    ])
}

/// A `defcustom` form, built fluently to avoid the ordering hazard of the
/// five positional arguments of [`defcustom`].
///
/// The `:type` is a [`CustomType`]; if not set explicitly it is inferred from
/// the value's static type (`El<Str>` to [`CustomType::String`], `El<Int>` to
/// [`CustomType::Integer`], and so on). The `:group` defaults to the variable
/// name if not set.
///
/// ```
/// use ferrel::*;
///
/// let form = Defcustom::new("my/greeting", string("hi"))
///     .doc("Greeting shown to the user.")
///     .group("my-config")
///     .build();
/// let out = form.render();
/// assert!(out.contains("(defcustom my/greeting \"hi\""));
/// assert!(out.contains(":type 'string"));
/// assert!(out.contains(":group 'my-config"));
/// ```
pub struct Defcustom {
    name: String,
    value: Sexp,
    doc: Option<String>,
    el_type: CustomType,
    group: Option<String>,
}

impl Defcustom {
    /// Start a `defcustom` for `name` with the given typed default `value`.
    ///
    /// The `:type` is inferred from `T` (overridable with [`Defcustom::custom_type`]).
    pub fn new<T: InferCustomType>(name: impl Into<String>, value: El<T>) -> Defcustom {
        Defcustom {
            name: name.into(),
            value: value.into_sexp(),
            doc: None,
            el_type: <T as InferCustomType>::CUSTOM_TYPE,
            group: None,
        }
    }

    /// Set the docstring.
    pub fn doc(mut self, text: impl Into<String>) -> Defcustom {
        self.doc = Some(text.into());
        self
    }

    /// Override the inferred customize `:type`.
    pub fn custom_type(mut self, el_type: CustomType) -> Defcustom {
        self.el_type = el_type;
        self
    }

    /// Set the customize `:group` (defaults to the variable name).
    pub fn group(mut self, group: impl Into<String>) -> Defcustom {
        self.group = Some(group.into());
        self
    }

    /// Lower to a `defcustom` s-expression.
    pub fn build(self) -> Sexp {
        let group = self.group.unwrap_or_else(|| self.name.clone());
        Sexp::List(vec![
            Sexp::sym("defcustom"),
            Sexp::sym(self.name),
            self.value,
            Sexp::Str(self.doc.unwrap_or_default()),
            Sexp::keyword("type"),
            Sexp::sym(self.el_type.symbol()).quoted(),
            Sexp::keyword("group"),
            Sexp::sym(group).quoted(),
        ])
    }
}

impl From<Defcustom> for Sexp {
    fn from(d: Defcustom) -> Sexp {
        d.build()
    }
}

/// Maps a typed-expression marker to its inferred [`CustomType`].
///
/// Implemented for the marker types that have an obvious customize type. Other
/// markers fall back to [`CustomType::Symbol`]; set the type explicitly with
/// [`Defcustom::custom_type`] when the inference is not what you want.
pub trait InferCustomType {
    /// The customize type inferred for this marker.
    const CUSTOM_TYPE: CustomType;
}

impl InferCustomType for crate::typed::Str {
    const CUSTOM_TYPE: CustomType = CustomType::String;
}

impl InferCustomType for crate::typed::Int {
    const CUSTOM_TYPE: CustomType = CustomType::Integer;
}

impl InferCustomType for crate::typed::Float {
    const CUSTOM_TYPE: CustomType = CustomType::Number;
}

impl InferCustomType for crate::typed::Bool {
    const CUSTOM_TYPE: CustomType = CustomType::Boolean;
}

impl InferCustomType for crate::typed::Symbol {
    const CUSTOM_TYPE: CustomType = CustomType::Symbol;
}

impl InferCustomType for crate::typed::List {
    const CUSTOM_TYPE: CustomType = CustomType::List;
}

impl InferCustomType for crate::typed::Any {
    const CUSTOM_TYPE: CustomType = CustomType::Symbol;
}

/// `(require 'feature)`.
pub fn require(feature: impl Into<String>) -> Sexp {
    Sexp::call("require", [Sexp::sym(feature).quoted()])
}

/// Top-level `(setq name value)`.
pub fn setq<T>(name: impl Into<String>, value: El<T>) -> Sexp {
    Sexp::call("setq", [Sexp::sym(name), value.into_sexp()])
}

/// `(add-hook 'hook #'function)`.
pub fn add_hook(hook: impl Into<String>, function: impl Into<String>) -> Sexp {
    Sexp::call(
        "add-hook",
        [
            Sexp::sym(hook).quoted(),
            Sexp::Function(Box::new(Sexp::sym(function))),
        ],
    )
}

/// `(global-set-key (kbd "key") #'command)`.
///
/// This accepts plain strings for both arguments and does not validate the key
/// syntax. For a key sequence checked at construction time, use a [`KeySeq`]
/// and [`bind_key`].
pub fn global_set_key(key: impl Into<String>, command: impl Into<String>) -> Sexp {
    Sexp::call(
        "global-set-key",
        [
            Sexp::call("kbd", [Sexp::Str(key.into())]),
            Sexp::Function(Box::new(Sexp::sym(command))),
        ],
    )
}

/// `(global-set-key (kbd "key") #'command)` from a validated [`KeySeq`] and a
/// [`Command`].
///
/// The [`KeySeq`] is checked when it is built (via [`KeySeq::parse`] or the
/// [`kbd!`](crate::kbd) macro), so a malformed key sequence cannot reach this
/// function, and the distinct [`Command`] type prevents passing the key and
/// command in the wrong order.
///
/// ```
/// use ferrel::*;
///
/// let form = bind_key(kbd!("C-c f"), Command::new("find-file"));
/// assert_eq!(form.render(), "(global-set-key (kbd \"C-c f\") #'find-file)");
/// ```
pub fn bind_key(key: KeySeq, command: Command) -> Sexp {
    Sexp::call(
        "global-set-key",
        [
            Sexp::call("kbd", [Sexp::Str(key.into_string())]),
            Sexp::Function(Box::new(Sexp::sym(command.into_string()))),
        ],
    )
}

/// Enable a minor mode: `(some-mode 1)`.
///
/// Emacs minor-mode commands take a numeric argument where a positive number
/// enables and a non-positive number disables. This names that convention so
/// config reads as intent rather than a bare `1`.
pub fn enable_mode(mode: impl Into<String>) -> Sexp {
    Sexp::call(mode, [Sexp::Int(1)])
}

/// Disable a minor mode: `(some-mode -1)`.
pub fn disable_mode(mode: impl Into<String>) -> Sexp {
    Sexp::call(mode, [Sexp::Int(-1)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typed::{int, message, string, var};

    #[test]
    fn builds_interactive_defun() {
        let f = Defun::new("ferrel-hello")
            .doc("Say hello.")
            .interactive()
            .body([message(string("hi"), [])])
            .build();
        let out = f.render();
        assert!(out.starts_with("(defun ferrel-hello ()"));
        assert!(out.contains("\"Say hello.\""));
        assert!(out.contains("(interactive)"));
        assert!(out.contains("(message \"hi\")"));
    }

    #[test]
    fn body_mixes_statement_types() {
        let f = Defun::new("demo")
            .interactive()
            .body(crate::stmts![
                message(string("hi"), Vec::<Sexp>::new()),
                crate::typed::insert(string("there")),
            ])
            .build();
        let out = f.render();
        assert!(out.contains("(message \"hi\")"));
        assert!(out.contains("(insert \"there\")"));
    }

    #[test]
    fn bind_key_renders_checked_binding() {
        let form = bind_key(crate::kbd!("C-c f"), Command::new("find-file"));
        assert_eq!(
            form.render(),
            "(global-set-key (kbd \"C-c f\") #'find-file)"
        );
    }

    #[test]
    fn custom_type_symbols() {
        assert_eq!(CustomType::String.symbol(), "string");
        assert_eq!(CustomType::Integer.symbol(), "integer");
        assert_eq!(CustomType::Boolean.symbol(), "boolean");
        assert_eq!(CustomType::Hook.symbol(), "hook");
    }

    #[test]
    fn defcustom_builder_infers_type() {
        let form = Defcustom::new("my/greeting", string("hi"))
            .doc("Greeting.")
            .group("my-config")
            .build();
        let out = form.render();
        assert!(out.contains("(defcustom my/greeting \"hi\""));
        assert!(out.contains(":type 'string"));
        assert!(out.contains(":group 'my-config"));
    }

    #[test]
    fn defcustom_builder_infers_int_and_defaults_group() {
        let form = Defcustom::new("my/count", int(3)).build();
        let out = form.render();
        assert!(out.contains(":type 'integer"));
        // Group defaults to the variable name.
        assert!(out.contains(":group 'my/count"));
    }

    #[test]
    fn defcustom_builder_type_override() {
        let form = Defcustom::new("my/hook", crate::typed::nil())
            .custom_type(CustomType::Hook)
            .build();
        assert!(form.render().contains(":type 'hook"));
    }

    #[test]
    fn typed_param_roundtrips() {
        let f = Defun::new("ferrel-add")
            .param("a")
            .param("b")
            .body([crate::typed::add(
                var::<crate::Int>("a"),
                var::<crate::Int>("b"),
            )])
            .build();
        assert!(f.render().contains("(+ a b)"));
        let _ = int(0); // keep import used across configs
    }
}
