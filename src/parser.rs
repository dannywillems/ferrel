//! Reader (parser) for Emacs Lisp.
//!
//! The parser consumes the token stream from [`crate::lexer`] and produces the
//! [`Sexp`] AST, the same AST the code generator renders from. That shared AST
//! is what makes the pipeline round-trip: `parse` then `render` reproduces an
//! equivalent file, and `render` then `parse` reproduces an equal AST.
//!
//! Reader macros (`'`, `` ` ``, `,`, `,@`, `#'`) become their dedicated AST
//! nodes, and dotted pairs `(a . b)` become [`Sexp::Dotted`], so the surface
//! syntax survives the round-trip rather than being desugared away.

use std::fmt;

use crate::{
    lexer::{LexError, Spanned, Token, tokenize},
    sexp::Sexp,
};

/// A parse error, with the byte offset at which it occurred.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub offset: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error at byte {}: {}", self.offset, self.message)
    }
}

impl std::error::Error for ParseError {}

impl From<LexError> for ParseError {
    fn from(e: LexError) -> ParseError {
        ParseError {
            message: e.message,
            offset: e.offset,
        }
    }
}

/// Parse every top-level form in `input`.
///
/// # Errors
///
/// Returns a [`ParseError`] on any lexing or structural error (unbalanced
/// parentheses, a misplaced dot, an unexpected closing delimiter).
pub fn parse(input: &str) -> Result<Vec<Sexp>, ParseError> {
    let tokens = tokenize(input)?;
    let mut parser = Parser::new(tokens, input.len());
    let mut forms = Vec::new();
    while parser.peek().is_some() {
        forms.push(parser.read_form()?);
    }
    Ok(forms)
}

/// Parse exactly one top-level form, erroring if the input is empty or has
/// trailing forms.
///
/// # Errors
///
/// Returns a [`ParseError`] as [`parse`] does, plus an error if `input` does
/// not contain exactly one form.
pub fn parse_one(input: &str) -> Result<Sexp, ParseError> {
    let tokens = tokenize(input)?;
    let eof = input.len();
    let mut parser = Parser::new(tokens, eof);
    if parser.peek().is_none() {
        return Err(ParseError {
            message: "expected a form, found empty input".to_string(),
            offset: 0,
        });
    }
    let form = parser.read_form()?;
    if let Some(extra) = parser.peek() {
        return Err(ParseError {
            message: "unexpected trailing form".to_string(),
            offset: extra.start,
        });
    }
    Ok(form)
}

struct Parser {
    tokens: Vec<Spanned>,
    pos: usize,
    eof: usize,
}

impl Parser {
    fn new(tokens: Vec<Spanned>, eof: usize) -> Self {
        Parser {
            tokens,
            pos: 0,
            eof,
        }
    }

    fn peek(&self) -> Option<&Spanned> {
        self.tokens.get(self.pos)
    }

    fn bump(&mut self) -> Option<Spanned> {
        let t = self.tokens.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    /// Offset to report for an error at the current position.
    fn here(&self) -> usize {
        self.peek().map_or(self.eof, |s| s.start)
    }

    fn read_form(&mut self) -> Result<Sexp, ParseError> {
        let spanned = self.bump().ok_or_else(|| ParseError {
            message: "expected a form, found end of input".to_string(),
            offset: self.eof,
        })?;
        match spanned.token {
            Token::LParen => self.read_list(),
            Token::LBracket => self.read_vector(),
            Token::Quote => Ok(Sexp::Quote(Box::new(self.read_form()?))),
            Token::Backquote => Ok(Sexp::Backquote(Box::new(self.read_form()?))),
            Token::Unquote => Ok(Sexp::Unquote(Box::new(self.read_form()?))),
            Token::Splice => Ok(Sexp::Splice(Box::new(self.read_form()?))),
            Token::Function => Ok(Sexp::Function(Box::new(self.read_form()?))),
            Token::Int(n) => Ok(Sexp::Int(n)),
            Token::Float(f) => Ok(Sexp::Float(f)),
            Token::Str(s) => Ok(Sexp::Str(s)),
            Token::Char(c) => Ok(Sexp::Char(c)),
            Token::Nil => Ok(Sexp::Nil),
            Token::True => Ok(Sexp::True),
            Token::Keyword(k) => Ok(Sexp::Keyword(k)),
            Token::Symbol(s) => Ok(Sexp::Sym(s)),
            Token::RParen => Err(ParseError {
                message: "unexpected `)`".to_string(),
                offset: spanned.start,
            }),
            Token::RBracket => Err(ParseError {
                message: "unexpected `]`".to_string(),
                offset: spanned.start,
            }),
            Token::Dot => Err(ParseError {
                message: "unexpected `.` outside a list".to_string(),
                offset: spanned.start,
            }),
        }
    }

    /// Read the rest of a list after its `(` has been consumed.
    fn read_list(&mut self) -> Result<Sexp, ParseError> {
        let mut items = Vec::new();
        loop {
            match self.peek().map(|s| &s.token) {
                None => {
                    return Err(ParseError {
                        message: "unclosed list, expected `)`".to_string(),
                        offset: self.eof,
                    });
                }
                Some(Token::RParen) => {
                    self.bump();
                    return Ok(Sexp::List(items));
                }
                Some(Token::Dot) => {
                    let dot_at = self.here();
                    self.bump(); // '.'
                    if items.is_empty() {
                        return Err(ParseError {
                            message: "`.` must follow at least one element".to_string(),
                            offset: dot_at,
                        });
                    }
                    let tail = self.read_form()?;
                    match self.bump().map(|s| s.token) {
                        Some(Token::RParen) => {
                            return Ok(Sexp::Dotted(items, Box::new(tail)));
                        }
                        _ => {
                            return Err(ParseError {
                                message: "expected `)` after dotted tail".to_string(),
                                offset: self.here(),
                            });
                        }
                    }
                }
                Some(_) => items.push(self.read_form()?),
            }
        }
    }

    /// Read the rest of a vector after its `[` has been consumed.
    fn read_vector(&mut self) -> Result<Sexp, ParseError> {
        let mut items = Vec::new();
        loop {
            match self.peek().map(|s| &s.token) {
                None => {
                    return Err(ParseError {
                        message: "unclosed vector, expected `]`".to_string(),
                        offset: self.eof,
                    });
                }
                Some(Token::RBracket) => {
                    self.bump();
                    return Ok(Sexp::Vector(items));
                }
                Some(Token::Dot) => {
                    return Err(ParseError {
                        message: "`.` is not allowed in a vector".to_string(),
                        offset: self.here(),
                    });
                }
                Some(_) => items.push(self.read_form()?),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_call() {
        let form = parse_one("(+ 1 2)").unwrap();
        assert_eq!(
            form,
            Sexp::List(vec![Sexp::Sym("+".into()), Sexp::Int(1), Sexp::Int(2)])
        );
    }

    #[test]
    fn parses_reader_macros() {
        assert_eq!(
            parse_one("'(a b)").unwrap(),
            Sexp::Quote(Box::new(Sexp::List(vec![
                Sexp::Sym("a".into()),
                Sexp::Sym("b".into()),
            ])))
        );
        assert_eq!(
            parse_one("#'foo").unwrap(),
            Sexp::Function(Box::new(Sexp::Sym("foo".into())))
        );
    }

    #[test]
    fn parses_dotted_pair() {
        assert_eq!(
            parse_one("(a . b)").unwrap(),
            Sexp::Dotted(vec![Sexp::Sym("a".into())], Box::new(Sexp::Sym("b".into())))
        );
    }

    #[test]
    fn parses_backquote_with_unquote() {
        // `(a ,b ,@c)
        let form = parse_one("`(a ,b ,@c)").unwrap();
        assert_eq!(
            form,
            Sexp::Backquote(Box::new(Sexp::List(vec![
                Sexp::Sym("a".into()),
                Sexp::Unquote(Box::new(Sexp::Sym("b".into()))),
                Sexp::Splice(Box::new(Sexp::Sym("c".into()))),
            ])))
        );
    }

    #[test]
    fn round_trips_through_render() {
        let sources = [
            "(defun f (a b) (+ a b))",
            "(setq x 1 y 2.5)",
            "'(1 2 3)",
            "`(a ,b ,@c)",
            "(a . b)",
            "(message \"hello %s\" name)",
            "[1 2 3]",
            "(global-set-key (kbd \"C-c h\") #'cmd)",
        ];
        for src in sources {
            let parsed = parse_one(src).unwrap();
            let rendered = parsed.render();
            let reparsed = parse_one(&rendered).unwrap();
            assert_eq!(parsed, reparsed, "round-trip mismatch for `{src}`");
        }
    }

    #[test]
    fn reports_unbalanced_parens() {
        let err = parse_one("(+ 1 2").unwrap_err();
        assert!(err.message.contains("unclosed"));
    }
}
