//! The untyped Emacs Lisp AST and its pretty-printer.
//!
//! [`Sexp`] is the single intermediate representation every typed builder
//! lowers into. The renderer turns it back into formatted `.el` text.

/// An Emacs Lisp s-expression.
///
/// This is deliberately small: it models the surface syntax of Elisp, not its
/// semantics. The typed layer in [`crate::typed`] is responsible for building
/// well-typed trees on top of these nodes.
#[derive(Debug, Clone, PartialEq)]
pub enum Sexp {
    /// The empty list / false: `nil`.
    Nil,
    /// Boolean truth: `t`.
    True,
    /// An integer literal: `42`.
    Int(i64),
    /// A floating point literal: `1.5`. Always rendered with a decimal point.
    Float(f64),
    /// A string literal. Rendered with quoting and escaping.
    Str(String),
    /// A character literal: `?x`.
    Char(char),
    /// A bare symbol (a variable reference or operator), e.g. `point`.
    Sym(String),
    /// A keyword symbol, e.g. `:type`.
    Keyword(String),
    /// A quoted form: `'x`.
    Quote(Box<Sexp>),
    /// A function-value form: `#'command`.
    Function(Box<Sexp>),
    /// A backquoted (quasiquote) form: `` `x ``.
    Backquote(Box<Sexp>),
    /// An unquote inside a backquote: `,x`.
    Unquote(Box<Sexp>),
    /// An unquote-splicing inside a backquote: `,@x`.
    Splice(Box<Sexp>),
    /// A proper list: `(a b c)`.
    List(Vec<Sexp>),
    /// An improper (dotted) list: `(a b . c)`. The `Vec` holds the leading
    /// elements (at least one) and the `Box` holds the tail after the dot.
    Dotted(Vec<Sexp>, Box<Sexp>),
    /// A vector literal: `[a b c]`.
    Vector(Vec<Sexp>),
    /// Raw, verbatim Elisp text. Escape hatch for things the AST cannot model.
    Raw(String),
}

impl Sexp {
    /// Build a bare symbol.
    pub fn sym(name: impl Into<String>) -> Sexp {
        Sexp::Sym(name.into())
    }

    /// Build a keyword symbol (a leading colon is added if absent).
    pub fn keyword(name: impl Into<String>) -> Sexp {
        let name = name.into();
        Sexp::Keyword(name.strip_prefix(':').unwrap_or(&name).to_string())
    }

    /// Build a list from an iterator of nodes.
    pub fn list<I: IntoIterator<Item = Sexp>>(items: I) -> Sexp {
        Sexp::List(items.into_iter().collect())
    }

    /// Build a call form `(head args...)`.
    pub fn call<I: IntoIterator<Item = Sexp>>(head: impl Into<String>, args: I) -> Sexp {
        let mut v = Vec::with_capacity(4);
        v.push(Sexp::Sym(head.into()));
        v.extend(args);
        Sexp::List(v)
    }

    /// Quote this node: `'self`.
    pub fn quoted(self) -> Sexp {
        Sexp::Quote(Box::new(self))
    }

    /// Render this node as formatted Elisp text (no trailing newline).
    pub fn render(&self) -> String {
        pretty(self, 0)
    }
}

impl From<i64> for Sexp {
    fn from(n: i64) -> Sexp {
        Sexp::Int(n)
    }
}

impl From<bool> for Sexp {
    fn from(b: bool) -> Sexp {
        if b { Sexp::True } else { Sexp::Nil }
    }
}

impl From<&str> for Sexp {
    fn from(s: &str) -> Sexp {
        Sexp::Str(s.to_string())
    }
}

impl From<String> for Sexp {
    fn from(s: String) -> Sexp {
        Sexp::Str(s)
    }
}

// --- Rendering ---------------------------------------------------------------

/// Target column at which lists are broken onto multiple lines.
const MAX_WIDTH: usize = 72;

/// How a compound form indents its arguments when it does not fit on one line.
enum Indent {
    /// Special form: keep `n` distinguished args on the head line, indent the
    /// rest of the body by two spaces. Matches `lisp-indent-function`.
    Block(usize),
    /// Function call: align continuation arguments under the first argument.
    Call,
}

/// Look up the indentation style for a list whose head is `head`.
fn indent_of(head: &Sexp) -> Indent {
    let Sexp::Sym(name) = head else {
        return Indent::Call;
    };
    match name.as_str() {
        "defun" | "defmacro" | "defsubst" | "cl-defun" | "defconst" | "defcustom" => {
            Indent::Block(2)
        }
        "condition-case" => Indent::Block(2),
        "lambda"
        | "let"
        | "let*"
        | "when"
        | "unless"
        | "while"
        | "dolist"
        | "dotimes"
        | "if"
        | "prog1"
        | "with-current-buffer"
        | "defvar"
        | "use-package" => Indent::Block(1),
        "progn"
        | "save-excursion"
        | "save-current-buffer"
        | "with-temp-buffer"
        | "cond"
        | "interactive" => Indent::Block(0),
        _ => Indent::Call,
    }
}

/// Render `s` on a single line, ignoring width.
fn inline(s: &Sexp) -> String {
    match s {
        Sexp::Nil => "nil".to_string(),
        Sexp::True => "t".to_string(),
        Sexp::Int(n) => n.to_string(),
        Sexp::Float(f) => render_float(*f),
        Sexp::Str(text) => render_string(text),
        Sexp::Char(c) => render_char(*c),
        Sexp::Sym(name) => name.clone(),
        Sexp::Keyword(name) => format!(":{name}"),
        Sexp::Quote(inner) => format!("'{}", inline(inner)),
        Sexp::Function(inner) => format!("#'{}", inline(inner)),
        Sexp::Backquote(inner) => format!("`{}", inline(inner)),
        Sexp::Unquote(inner) => format!(",{}", inline(inner)),
        Sexp::Splice(inner) => format!(",@{}", inline(inner)),
        Sexp::Raw(text) => text.clone(),
        Sexp::List(items) => {
            let parts: Vec<String> = items.iter().map(inline).collect();
            format!("({})", parts.join(" "))
        }
        Sexp::Dotted(items, tail) => {
            let mut parts: Vec<String> = items.iter().map(inline).collect();
            parts.push(".".to_string());
            parts.push(inline(tail));
            format!("({})", parts.join(" "))
        }
        Sexp::Vector(items) => {
            let parts: Vec<String> = items.iter().map(inline).collect();
            format!("[{}]", parts.join(" "))
        }
    }
}

/// Render `s` starting at column `indent`, breaking lists that overflow.
fn pretty(s: &Sexp, indent: usize) -> String {
    let flat = inline(s);
    if indent + visual_len(&flat) <= MAX_WIDTH && !flat.contains('\n') {
        return flat;
    }
    match s {
        Sexp::List(items) if !items.is_empty() => break_list(items, indent),
        Sexp::Vector(items) if !items.is_empty() => break_vector(items, indent),
        Sexp::Quote(inner) => format!("'{}", pretty(inner, indent + 1)),
        Sexp::Function(inner) => format!("#'{}", pretty(inner, indent + 2)),
        Sexp::Backquote(inner) => format!("`{}", pretty(inner, indent + 1)),
        Sexp::Unquote(inner) => format!(",{}", pretty(inner, indent + 1)),
        Sexp::Splice(inner) => format!(",@{}", pretty(inner, indent + 2)),
        // Atoms, dotted pairs, and overlong strings are kept on one line.
        _ => flat,
    }
}

fn break_list(items: &[Sexp], indent: usize) -> String {
    let head = &items[0];
    let head_str = inline(head);
    match indent_of(head) {
        Indent::Block(distinguished) => {
            let mut out = format!("({head_str}");
            let mut i = 1;
            // Distinguished arguments share the head line.
            while i <= distinguished && i < items.len() {
                out.push(' ');
                let col = indent + last_line_len(&out);
                out.push_str(&pretty(&items[i], col));
                i += 1;
            }
            // Remaining elements: one per line, plist pairs kept together.
            let body_indent = indent + 2;
            while i < items.len() {
                out.push('\n');
                out.push_str(&" ".repeat(body_indent));
                i = emit_element(&mut out, items, i, body_indent);
            }
            out.push(')');
            out
        }
        Indent::Call => {
            let mut out = format!("({head_str}");
            let arg_col = indent + visual_len(&head_str) + 2; // '(' + head + ' '
            // First argument shares the head line; the rest align under it.
            let mut i = 1;
            let mut first = true;
            while i < items.len() {
                if first {
                    out.push(' ');
                    first = false;
                } else {
                    out.push('\n');
                    out.push_str(&" ".repeat(arg_col));
                }
                i = emit_element(&mut out, items, i, arg_col);
            }
            out.push(')');
            out
        }
    }
}

/// Emit the element at `i` into `out` at column `col`, keeping a
/// `:keyword value` plist pair on the same line. Returns the next index.
fn emit_element(out: &mut String, items: &[Sexp], i: usize, col: usize) -> usize {
    if let Sexp::Keyword(name) = &items[i]
        && i + 1 < items.len()
    {
        let prefix = format!(":{name} ");
        out.push_str(&prefix);
        let value_col = col + visual_len(&prefix);
        out.push_str(&pretty(&items[i + 1], value_col));
        return i + 2;
    }
    out.push_str(&pretty(&items[i], col));
    i + 1
}

fn break_vector(items: &[Sexp], indent: usize) -> String {
    let mut out = String::from("[");
    let col = indent + 1;
    for (k, item) in items.iter().enumerate() {
        if k > 0 {
            out.push('\n');
            out.push_str(&" ".repeat(col));
        }
        out.push_str(&pretty(item, col));
    }
    out.push(']');
    out
}

fn render_float(f: f64) -> String {
    let s = format!("{f}");
    if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") {
        s
    } else {
        format!("{s}.0")
    }
}

fn render_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

fn render_char(c: char) -> String {
    match c {
        ' ' => "?\\s".to_string(),
        '\n' => "?\\n".to_string(),
        '\t' => "?\\t".to_string(),
        '\\' => "?\\\\".to_string(),
        _ => format!("?{c}"),
    }
}

/// Visual length of a single-line string (character count).
fn visual_len(s: &str) -> usize {
    s.chars().count()
}

/// Number of characters after the last newline in `s`.
fn last_line_len(s: &str) -> usize {
    match s.rfind('\n') {
        Some(idx) => s[idx + 1..].chars().count(),
        None => s.chars().count(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_atoms() {
        assert_eq!(Sexp::Int(42).render(), "42");
        assert_eq!(Sexp::Float(1.0).render(), "1.0");
        assert_eq!(Sexp::Str("hi".into()).render(), "\"hi\"");
        assert_eq!(Sexp::True.render(), "t");
        assert_eq!(Sexp::Nil.render(), "nil");
        assert_eq!(Sexp::keyword(":type").render(), ":type");
        assert_eq!(Sexp::Char('a').render(), "?a");
    }

    #[test]
    fn escapes_strings() {
        assert_eq!(Sexp::Str("a\"b\\c".into()).render(), "\"a\\\"b\\\\c\"");
    }

    #[test]
    fn renders_short_list_inline() {
        let s = Sexp::call("+", [Sexp::Int(1), Sexp::Int(2)]);
        assert_eq!(s.render(), "(+ 1 2)");
    }

    #[test]
    fn breaks_long_call_aligned() {
        let long = Sexp::Str("x".repeat(80));
        let s = Sexp::call("message", [long.clone(), long]);
        let out = s.render();
        assert!(out.contains('\n'), "expected a multi-line render: {out}");
        // Continuation aligns under the first argument: "(message " is 9 chars,
        // so exactly 9 spaces of indent, not 8 or 10.
        let cont = out.lines().nth(1).unwrap();
        assert!(cont.starts_with("         ") && !cont.starts_with("          "));
    }
}
