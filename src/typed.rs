//! The typed expression layer.
//!
//! [`El<T>`] is a thin, zero-cost wrapper around [`Sexp`] carrying a phantom
//! type. The point is not runtime safety (the output is still dynamically typed
//! Elisp) but compile-time safety: the Rust compiler rejects programs like
//! `add(string("x"), int(2))` before any `.el` is ever written.
//!
//! Emacs has thousands of functions across thousands of packages; ferrel does
//! not try to type all of them. It types the core you reach for constantly
//! (arithmetic, strings, control flow, common builtins) and gives you an
//! ergonomic untyped escape hatch, [`call`], for everything else.

use std::marker::PhantomData;

use crate::sexp::Sexp;

// --- Type markers ------------------------------------------------------------

/// Marker for Elisp integers.
pub enum Int {}
/// Marker for Elisp floats.
pub enum Float {}
/// Marker for Elisp strings.
pub enum Str {}
/// Marker for Elisp booleans (`t` / `nil`).
pub enum Bool {}
/// Marker for Elisp symbols.
pub enum Symbol {}
/// Marker for Elisp lists.
pub enum List {}
/// Marker for an unknown / untyped value (the escape hatch result type).
pub enum Any {}

// --- The typed wrapper -------------------------------------------------------

/// A typed Elisp expression. `T` is a phantom marker; the value is the `Sexp`.
#[derive(Debug, Clone)]
pub struct El<T> {
    sexp: Sexp,
    _marker: PhantomData<fn() -> T>,
}

impl<T> El<T> {
    /// Wrap a raw `Sexp` with the type marker `T`.
    pub(crate) fn wrap(sexp: Sexp) -> El<T> {
        El {
            sexp,
            _marker: PhantomData,
        }
    }

    /// Borrow the underlying s-expression.
    pub fn sexp(&self) -> &Sexp {
        &self.sexp
    }

    /// Consume into the underlying s-expression.
    pub fn into_sexp(self) -> Sexp {
        self.sexp
    }

    /// Reinterpret this expression at a different type. Use when you know the
    /// runtime value but the static type was widened (e.g. after [`call`]).
    pub fn cast<U>(self) -> El<U> {
        El::wrap(self.sexp)
    }

    /// Render this expression as formatted Elisp text.
    pub fn render(&self) -> String {
        self.sexp.render()
    }
}

impl<T> From<El<T>> for Sexp {
    fn from(value: El<T>) -> Sexp {
        value.sexp
    }
}

impl From<&str> for El<Str> {
    fn from(s: &str) -> El<Str> {
        El::wrap(Sexp::Str(s.to_string()))
    }
}

impl From<String> for El<Str> {
    fn from(s: String) -> El<Str> {
        El::wrap(Sexp::Str(s))
    }
}

impl From<i64> for El<Int> {
    fn from(n: i64) -> El<Int> {
        El::wrap(Sexp::Int(n))
    }
}

impl From<f64> for El<Float> {
    fn from(f: f64) -> El<Float> {
        El::wrap(Sexp::Float(f))
    }
}

impl From<bool> for El<Bool> {
    fn from(b: bool) -> El<Bool> {
        El::wrap(b.into())
    }
}

// --- Argument lowering -------------------------------------------------------

/// Lowering of a value into an argument [`Sexp`].
///
/// This is the trait that lets argument-list APIs such as [`call`] accept both
/// typed expressions ([`El<T>`]) and already-lowered [`Sexp`] nodes in the same
/// iterator. Callers holding `El<T>` values no longer have to sprinkle
/// `.into_sexp()` on every argument.
///
/// ```
/// use ferrel::*;
///
/// // Mixing a typed string, `t`, and a raw Sexp in one argument list:
/// let e = call("set-frame-font", [string("Monospace 12").into_arg(), t().into_arg()]);
/// assert_eq!(e.render(), "(set-frame-font \"Monospace 12\" t)");
/// ```
pub trait IntoSexp {
    /// Lower `self` into an argument s-expression.
    fn into_arg(self) -> Sexp;
}

impl<T> IntoSexp for El<T> {
    fn into_arg(self) -> Sexp {
        self.sexp
    }
}

impl IntoSexp for Sexp {
    fn into_arg(self) -> Sexp {
        self
    }
}

/// Build a `Vec<Sexp>` argument list from heterogeneous typed and raw values.
///
/// A Rust array `[a, b, c]` requires a single element type, so an argument
/// list mixing `El<Str>`, `El<Bool>`, and a raw `Sexp` cannot be written as one
/// array. This macro lowers each element with [`IntoSexp`] and collects them,
/// so a mixed argument list "just works":
///
/// ```
/// use ferrel::*;
///
/// let e = call("set-frame-font", args![string("Monospace 12"), t(), t()]);
/// assert_eq!(e.render(), "(set-frame-font \"Monospace 12\" t t)");
/// ```
#[macro_export]
macro_rules! args {
    ($($e:expr),* $(,)?) => {
        ::std::vec![$($crate::IntoSexp::into_arg($e)),*]
    };
}

// --- Literals ----------------------------------------------------------------

/// An integer literal.
pub fn int(n: i64) -> El<Int> {
    El::wrap(Sexp::Int(n))
}

/// A float literal.
pub fn float(f: f64) -> El<Float> {
    El::wrap(Sexp::Float(f))
}

/// A string literal.
pub fn string(s: impl Into<String>) -> El<Str> {
    El::wrap(Sexp::Str(s.into()))
}

/// The boolean `t`.
pub fn t() -> El<Bool> {
    El::wrap(Sexp::True)
}

/// The boolean / empty value `nil`.
pub fn nil() -> El<Bool> {
    El::wrap(Sexp::Nil)
}

/// A boolean literal.
pub fn boolean(b: bool) -> El<Bool> {
    El::wrap(b.into())
}

/// A quoted symbol value: `'name`.
pub fn quoted_symbol(name: impl Into<String>) -> El<Symbol> {
    El::wrap(Sexp::sym(name).quoted())
}

/// A dotted pair (cons cell) `(car . cdr)` from two typed expressions.
///
/// Dotted pairs appear throughout Elisp configuration data: `:bind` and `:hook`
/// entries in `use-package`, `auto-mode-alist` rows, and association lists in
/// general. This lowers both sides via [`IntoSexp`] and builds a
/// [`Sexp::Dotted`], so a typed key and value can be paired without dropping to
/// the raw AST by hand.
///
/// ```
/// use ferrel::*;
///
/// let p = pair(string("\\.rs\\'"), quoted_symbol("rust-mode"));
/// assert_eq!(p.render(), "(\"\\\\.rs\\\\'\" . 'rust-mode)");
/// ```
pub fn pair<A: IntoSexp, B: IntoSexp>(car: A, cdr: B) -> Sexp {
    Sexp::Dotted(vec![car.into_arg()], Box::new(cdr.into_arg()))
}

/// A reference to a variable named `name`, asserted to have type `T`.
///
/// This is how function parameters and `defvar`/`defcustom` values are read
/// back inside a body. The type is your assertion; ferrel trusts it.
pub fn var<T>(name: impl Into<String>) -> El<T> {
    El::wrap(Sexp::sym(name))
}

// --- The untyped escape hatch ------------------------------------------------

/// Call an arbitrary Elisp function by name, with already-lowered arguments.
///
/// The result type is [`Any`]; use [`El::cast`] to narrow it when you know what
/// the function returns. This is the bridge to the parts of Emacs (and every
/// third-party package) that ferrel does not, and cannot, type.
///
/// Arguments are already-lowered [`Sexp`] nodes. To pass typed [`El<T>`] values
/// (even of differing types) without hand-writing `.into_sexp()` on each, use
/// the [`args!`](crate::args) macro, which lowers each element via [`IntoSexp`]:
///
/// ```
/// use ferrel::*;
///
/// let e = call("set-frame-font", args![string("Monospace 12"), t(), t()]);
/// assert_eq!(e.render(), "(set-frame-font \"Monospace 12\" t t)");
/// ```
///
/// ```ignore
/// // (projectile-project-root) returns a path string:
/// let root: El<Str> = call("projectile-project-root", []).cast();
/// ```
pub fn call<I: IntoIterator<Item = Sexp>>(name: impl Into<String>, args: I) -> El<Any> {
    El::wrap(Sexp::call(name, args))
}

/// Call an arbitrary Elisp function and assert the result type `T` inline.
///
/// This is [`call`] with the asserted return type chosen at the call site
/// instead of as a trailing [`El::cast`]. Prefer this over `call(...).cast()`
/// so the asserted type is a deliberate, greppable choice rather than an easy
/// to misplace afterthought.
///
/// ```
/// use ferrel::*;
///
/// // (projectile-project-root) returns a path string:
/// let root: El<Str> = call_typed("projectile-project-root", Vec::<Sexp>::new());
/// assert_eq!(root.render(), "(projectile-project-root)");
///
/// // With typed arguments, use the `args!` macro:
/// let n: El<Int> = call_typed("string-width", args![string("hi")]);
/// assert_eq!(n.render(), "(string-width \"hi\")");
/// ```
pub fn call_typed<T, I: IntoIterator<Item = Sexp>>(name: impl Into<String>, args: I) -> El<T> {
    El::wrap(Sexp::call(name, args))
}

// --- Arithmetic --------------------------------------------------------------

/// `(+ a b)`.
pub fn add(a: El<Int>, b: El<Int>) -> El<Int> {
    El::wrap(Sexp::call("+", [a.sexp, b.sexp]))
}

/// `(- a b)`.
pub fn sub(a: El<Int>, b: El<Int>) -> El<Int> {
    El::wrap(Sexp::call("-", [a.sexp, b.sexp]))
}

/// `(* a b)`.
pub fn mul(a: El<Int>, b: El<Int>) -> El<Int> {
    El::wrap(Sexp::call("*", [a.sexp, b.sexp]))
}

/// `(+ x y z ...)` over any number of integers.
pub fn sum<I: IntoIterator<Item = El<Int>>>(xs: I) -> El<Int> {
    El::wrap(Sexp::call("+", xs.into_iter().map(El::into_sexp)))
}

// --- Comparison --------------------------------------------------------------

/// `(= a b)`.
pub fn num_eq(a: El<Int>, b: El<Int>) -> El<Bool> {
    El::wrap(Sexp::call("=", [a.sexp, b.sexp]))
}

/// `(< a b)`.
pub fn lt(a: El<Int>, b: El<Int>) -> El<Bool> {
    El::wrap(Sexp::call("<", [a.sexp, b.sexp]))
}

/// `(> a b)`.
pub fn gt(a: El<Int>, b: El<Int>) -> El<Bool> {
    El::wrap(Sexp::call(">", [a.sexp, b.sexp]))
}

/// `(equal a b)` for any two values of the same type.
pub fn equal<T>(a: El<T>, b: El<T>) -> El<Bool> {
    El::wrap(Sexp::call("equal", [a.sexp, b.sexp]))
}

/// `(not x)`.
pub fn not(x: El<Bool>) -> El<Bool> {
    El::wrap(Sexp::call("not", [x.sexp]))
}

// --- Strings -----------------------------------------------------------------

/// `(concat a b c ...)`.
pub fn concat<I: IntoIterator<Item = El<Str>>>(xs: I) -> El<Str> {
    El::wrap(Sexp::call("concat", xs.into_iter().map(El::into_sexp)))
}

/// `(format fmt args...)`. Pass typed arguments with the [`args!`](crate::args)
/// macro: `format(string("%s"), args![string("x")])`.
pub fn format<I: IntoIterator<Item = Sexp>>(fmt: El<Str>, args: I) -> El<Str> {
    let mut v = vec![fmt.sexp];
    v.extend(args);
    El::wrap(Sexp::call("format", v))
}

/// `(message fmt args...)`. Returns the formatted string. Pass typed arguments
/// with the [`args!`](crate::args) macro: `message(string("%s"), args![n])`.
pub fn message<I: IntoIterator<Item = Sexp>>(fmt: El<Str>, args: I) -> El<Str> {
    let mut v = vec![fmt.sexp];
    v.extend(args);
    El::wrap(Sexp::call("message", v))
}

// --- Common editor builtins --------------------------------------------------

/// `(insert s)`.
pub fn insert(s: El<Str>) -> El<Any> {
    El::wrap(Sexp::call("insert", [s.sexp]))
}

/// `(point)`.
pub fn point() -> El<Int> {
    El::wrap(Sexp::call::<[Sexp; 0]>("point", []))
}

/// `(buffer-name)`.
pub fn buffer_name() -> El<Str> {
    El::wrap(Sexp::call::<[Sexp; 0]>("buffer-name", []))
}

/// `(format-time-string fmt)`. Returns the formatted time string.
pub fn format_time_string(fmt: El<Str>) -> El<Str> {
    El::wrap(Sexp::call("format-time-string", [fmt.sexp]))
}

/// `(expand-file-name name)`. Returns an absolute path string.
pub fn expand_file_name(name: El<Str>) -> El<Str> {
    El::wrap(Sexp::call("expand-file-name", [name.sexp]))
}

/// `(executable-find program)`. Returns the path string, or `nil`.
pub fn executable_find(program: El<Str>) -> El<Str> {
    El::wrap(Sexp::call("executable-find", [program.sexp]))
}

// --- Control flow ------------------------------------------------------------

/// `(progn body...)`. Build a mixed-type body with the
/// [`stmts!`](crate::stmts) macro and map into `Sexp`.
pub fn progn<I: IntoIterator<Item = Sexp>>(body: I) -> El<Any> {
    El::wrap(Sexp::call("progn", body))
}

/// `(if cond then else)`. Both branches share a result type.
pub fn if_<T>(cond: El<Bool>, then: El<T>, els: El<T>) -> El<T> {
    El::wrap(Sexp::call("if", [cond.sexp, then.sexp, els.sexp]))
}

/// `(when cond body...)`. Body elements are `Sexp`; lower a mixed-type body
/// with the [`stmts!`](crate::stmts) macro.
pub fn when<I: IntoIterator<Item = Sexp>>(cond: El<Bool>, body: I) -> El<Any> {
    let mut v = vec![cond.sexp];
    v.extend(body);
    El::wrap(Sexp::call("when", v))
}

/// `(unless cond body...)`. Body elements are `Sexp`; lower a mixed-type body
/// with the [`stmts!`](crate::stmts) macro.
pub fn unless<I: IntoIterator<Item = Sexp>>(cond: El<Bool>, body: I) -> El<Any> {
    let mut v = vec![cond.sexp];
    v.extend(body);
    El::wrap(Sexp::call("unless", v))
}

/// `(setq name value)`. Returns the assigned value.
pub fn setq<T>(name: impl Into<String>, value: El<T>) -> El<T> {
    El::wrap(Sexp::call("setq", [Sexp::sym(name), value.sexp]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_is_typed() {
        let e = add(int(1), mul(int(2), int(3)));
        assert_eq!(e.render(), "(* 2 3)".replace("(* 2 3)", "(+ 1 (* 2 3))"));
    }

    #[test]
    fn message_with_args() {
        let e = message(string("hi %s"), [string("world").into()]);
        assert_eq!(e.render(), "(message \"hi %s\" \"world\")");
    }

    #[test]
    fn escape_hatch_casts() {
        let root: El<Str> = call("projectile-project-root", []).cast();
        assert_eq!(root.render(), "(projectile-project-root)");
    }

    #[test]
    fn call_typed_asserts_inline() {
        let n: El<Int> = call_typed("buffer-size", []);
        assert_eq!(n.render(), "(buffer-size)");
    }

    #[test]
    fn args_macro_mixes_typed_values() {
        let e = call("set-frame-font", args![string("Monospace 12"), t(), t()]);
        assert_eq!(e.render(), "(set-frame-font \"Monospace 12\" t t)");
    }

    #[test]
    fn into_arg_lowers_typed_and_raw() {
        assert_eq!(string("x").into_arg(), Sexp::Str("x".into()));
        assert_eq!(Sexp::Int(3).into_arg(), Sexp::Int(3));
    }

    #[test]
    fn pair_builds_a_dotted_cons() {
        let p = pair(string("k"), Sexp::sym("v"));
        assert_eq!(
            p,
            Sexp::Dotted(vec![Sexp::Str("k".into())], Box::new(Sexp::sym("v")))
        );
        assert_eq!(p.render(), "(\"k\" . v)");
    }

    #[test]
    fn el_literal_conversions() {
        let s: El<Str> = "hi".into();
        assert_eq!(s.render(), "\"hi\"");
        let s2: El<Str> = String::from("yo").into();
        assert_eq!(s2.render(), "\"yo\"");
        let n: El<Int> = 7_i64.into();
        assert_eq!(n.render(), "7");
        let f: El<Float> = 1.5_f64.into();
        assert_eq!(f.render(), "1.5");
        let b: El<Bool> = true.into();
        assert_eq!(b.render(), "t");
    }
}
