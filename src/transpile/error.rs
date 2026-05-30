//! The transpiler error type.
//!
//! Every refusal in the transpiler carries a source location so the user can
//! find the offending construct. The honesty rule of the transpiler (every
//! construct lowers faithfully or is refused) is only useful if the refusal
//! points at the line and column it is talking about.

use std::fmt;

use syn::spanned::Spanned;

/// An error raised while lowering a Rust subset into the `Sexp` IR.
///
/// The `line` and `col` fields locate the offending construct in the source
/// file (1-based line, 0-based column, matching `proc_macro2::LineColumn`).
///
/// ```
/// use ferrel::transpile::transpile_str;
///
/// // `?` is outside the supported subset, so this is refused with a location.
/// let err = transpile_str("fn f() -> i64 { g()? }").unwrap_err();
/// assert!(err.message().contains("unsupported"));
/// assert!(err.line() >= 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranspileError {
    message: String,
    line: usize,
    col: usize,
}

impl TranspileError {
    /// Build an error at an explicit line and column.
    pub(crate) fn at(message: impl Into<String>, line: usize, col: usize) -> TranspileError {
        TranspileError {
            message: message.into(),
            line,
            col,
        }
    }

    /// Build an error located at the span of any `syn` node.
    ///
    /// This is the common constructor: pass the node that cannot be lowered and
    /// a message describing why, and the span is read off the node.
    pub(crate) fn spanned(node: &impl Spanned, message: impl Into<String>) -> TranspileError {
        let start = node.span().start();
        TranspileError {
            message: message.into(),
            line: start.line,
            col: start.column,
        }
    }

    /// The human-readable reason the construct was refused.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The 1-based source line of the offending construct.
    pub fn line(&self) -> usize {
        self.line
    }

    /// The 0-based source column of the offending construct.
    pub fn col(&self) -> usize {
        self.col
    }
}

impl fmt::Display for TranspileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col + 1, self.message)
    }
}

impl std::error::Error for TranspileError {}

impl From<syn::Error> for TranspileError {
    fn from(e: syn::Error) -> TranspileError {
        let start = e.span().start();
        TranspileError {
            message: format!("Rust parse error: {e}"),
            line: start.line,
            col: start.column,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_uses_one_based_column() {
        let err = TranspileError::at("nope", 3, 5);
        assert_eq!(err.to_string(), "3:6: nope");
        assert_eq!(err.line(), 3);
        assert_eq!(err.col(), 5);
        assert_eq!(err.message(), "nope");
    }
}
