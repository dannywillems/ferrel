//! Type-erased statements for effectful bodies.
//!
//! A function body, a `progn`, or a `when`/`unless` body is a *sequence of
//! effects*. The result type of each statement is irrelevant: only the last
//! value is returned, and bodies are usually run for their side effects. Yet a
//! single Rust array forces one element type, so mixing `message(...)`
//! (`El<Str>`) and `insert(...)` (`El<Any>`) in one body does not type-check
//! when the body element type is `El<T>`.
//!
//! [`Stmt`] erases the phantom type so heterogeneous effects compose. Any
//! [`El<T>`] converts into a [`Stmt`] (via [`From`]), as does a raw [`Sexp`],
//! so existing homogeneous bodies keep working while mixed bodies now compile.

use crate::{sexp::Sexp, typed::El};

/// A single statement in an effectful body, with its result type erased.
///
/// Build a body from a mix of typed expressions with the [`stmts!`] macro or
/// any `IntoIterator` of values that are `Into<Stmt>`:
///
/// ```
/// use ferrel::*;
///
/// let f = Defun::new("demo")
///     .interactive()
///     .body(stmts![
///         message(string("hi"), Vec::<Sexp>::new()),
///         insert(string("there")),
///     ])
///     .build();
/// assert!(f.render().contains("(message \"hi\")"));
/// assert!(f.render().contains("(insert \"there\")"));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Stmt(Sexp);

impl Stmt {
    /// Wrap a raw [`Sexp`] as a statement.
    pub fn new(sexp: Sexp) -> Stmt {
        Stmt(sexp)
    }

    /// Borrow the underlying s-expression.
    pub fn sexp(&self) -> &Sexp {
        &self.0
    }

    /// Consume into the underlying s-expression.
    pub fn into_sexp(self) -> Sexp {
        self.0
    }
}

impl<T> From<El<T>> for Stmt {
    fn from(value: El<T>) -> Stmt {
        Stmt(value.into_sexp())
    }
}

impl From<Sexp> for Stmt {
    fn from(value: Sexp) -> Stmt {
        Stmt(value)
    }
}

impl From<Stmt> for Sexp {
    fn from(value: Stmt) -> Sexp {
        value.0
    }
}

/// Build a [`Vec<Stmt>`] from a comma-separated list of effects.
///
/// Each element is converted with [`Into<Stmt>`], so typed expressions of
/// different result types may be freely mixed. This is the ergonomic way to
/// build a [`Defun::body`](crate::Defun::body):
///
/// ```
/// use ferrel::*;
///
/// let body = stmts![
///     message(string("hello"), Vec::<Sexp>::new()),
///     insert(string("world")),
/// ];
/// assert_eq!(body.len(), 2);
/// ```
#[macro_export]
macro_rules! stmts {
    ($($e:expr),* $(,)?) => {
        ::std::vec![$($crate::Stmt::from($e)),*]
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typed::{insert, message, string};

    #[test]
    fn el_of_any_type_becomes_stmt() {
        let s: Stmt = message(string("hi"), Vec::<Sexp>::new()).into();
        assert_eq!(s.sexp().render(), "(message \"hi\")");
        let s2: Stmt = insert(string("x")).into();
        assert_eq!(s2.into_sexp().render(), "(insert \"x\")");
    }

    #[test]
    fn sexp_becomes_stmt_and_back() {
        let s: Stmt = Sexp::Int(1).into();
        let back: Sexp = s.into();
        assert_eq!(back, Sexp::Int(1));
    }

    #[test]
    fn stmts_macro_mixes_types() {
        let body = stmts![
            message(string("hi"), Vec::<Sexp>::new()),
            insert(string("there")),
        ];
        assert_eq!(body.len(), 2);
        assert_eq!(body[0].sexp().render(), "(message \"hi\")");
        assert_eq!(body[1].sexp().render(), "(insert \"there\")");
    }
}
