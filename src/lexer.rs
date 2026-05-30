//! Lexer for Emacs Lisp reader syntax.
//!
//! The lexer turns source text into a flat stream of [`Spanned`] tokens. It
//! handles the lexical layer only: delimiters, reader-macro prefixes, string
//! and character literals with escapes, numbers (including `#x`/`#o`/`#b` and
//! `#NrDIGITS` radixes), symbols, and keywords. Comments and whitespace are
//! discarded. Structure (lists, dotted pairs, quoting) is the parser's job.

use std::fmt;

/// A lexical token.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `'` reader macro.
    Quote,
    /// `` ` `` reader macro.
    Backquote,
    /// `,` reader macro.
    Unquote,
    /// `,@` reader macro.
    Splice,
    /// `#'` reader macro.
    Function,
    /// A lone `.`, used for dotted pairs.
    Dot,
    /// An integer literal.
    Int(i64),
    /// A floating point literal.
    Float(f64),
    /// A string literal (already unescaped).
    Str(String),
    /// A character literal (already unescaped).
    Char(char),
    /// The symbol `nil`.
    Nil,
    /// The symbol `t`.
    True,
    /// A keyword symbol (without the leading colon).
    Keyword(String),
    /// Any other symbol.
    Symbol(String),
}

/// A token together with its byte span `[start, end)` in the source.
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned {
    pub token: Token,
    pub start: usize,
    pub end: usize,
}

/// A lexing error, with the byte offset at which it occurred.
#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub message: String,
    pub offset: usize,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "lex error at byte {}: {}", self.offset, self.message)
    }
}

impl std::error::Error for LexError {}

/// Tokenize `input` into a vector of spanned tokens.
///
/// # Errors
///
/// Returns a [`LexError`] on an unterminated string, an unsupported reader
/// macro, or any other character the lexer cannot classify.
pub fn tokenize(input: &str) -> Result<Vec<Spanned>, LexError> {
    Lexer::new(input).run()
}

/// True for characters that terminate a symbol or number token.
fn is_delimiter(c: char) -> bool {
    c.is_whitespace() || matches!(c, '(' | ')' | '[' | ']' | '"' | ';' | '\'' | '`' | ',')
}

struct Lexer<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Lexer { src, pos: 0 }
    }

    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn run(mut self) -> Result<Vec<Spanned>, LexError> {
        let mut tokens = Vec::new();
        while let Some(c) = self.peek() {
            // Whitespace.
            if c.is_whitespace() {
                self.bump();
                continue;
            }
            // Line comments.
            if c == ';' {
                while let Some(ch) = self.peek() {
                    if ch == '\n' {
                        break;
                    }
                    self.bump();
                }
                continue;
            }
            let start = self.pos;
            let token = self.lex_token(c)?;
            tokens.push(Spanned {
                token,
                start,
                end: self.pos,
            });
        }
        Ok(tokens)
    }

    fn lex_token(&mut self, c: char) -> Result<Token, LexError> {
        match c {
            '(' => {
                self.bump();
                Ok(Token::LParen)
            }
            ')' => {
                self.bump();
                Ok(Token::RParen)
            }
            '[' => {
                self.bump();
                Ok(Token::LBracket)
            }
            ']' => {
                self.bump();
                Ok(Token::RBracket)
            }
            '\'' => {
                self.bump();
                Ok(Token::Quote)
            }
            '`' => {
                self.bump();
                Ok(Token::Backquote)
            }
            ',' => {
                self.bump();
                if self.peek() == Some('@') {
                    self.bump();
                    Ok(Token::Splice)
                } else {
                    Ok(Token::Unquote)
                }
            }
            '"' => self.lex_string(),
            '?' => self.lex_char(),
            '#' => self.lex_hash(),
            _ => self.lex_atom(),
        }
    }

    fn lex_string(&mut self) -> Result<Token, LexError> {
        let open = self.pos;
        self.bump(); // opening quote
        let mut out = String::new();
        loop {
            match self.bump() {
                None => {
                    return Err(LexError {
                        message: "unterminated string".to_string(),
                        offset: open,
                    });
                }
                Some('"') => return Ok(Token::Str(out)),
                Some('\\') => {
                    let esc = self.bump().ok_or_else(|| LexError {
                        message: "unterminated escape in string".to_string(),
                        offset: self.pos,
                    })?;
                    out.push(unescape(esc));
                }
                Some(ch) => out.push(ch),
            }
        }
    }

    fn lex_char(&mut self) -> Result<Token, LexError> {
        let start = self.pos;
        self.bump(); // '?'
        let c = self.bump().ok_or_else(|| LexError {
            message: "unterminated character literal".to_string(),
            offset: start,
        })?;
        if c == '\\' {
            let esc = self.bump().ok_or_else(|| LexError {
                message: "unterminated character escape".to_string(),
                offset: start,
            })?;
            Ok(Token::Char(unescape(esc)))
        } else {
            Ok(Token::Char(c))
        }
    }

    /// Lex a token that begins with `#`: `#'` (function quote) or a radix
    /// number (`#x`, `#o`, `#b`, `#NrDIGITS`).
    fn lex_hash(&mut self) -> Result<Token, LexError> {
        let start = self.pos;
        self.bump(); // '#'
        match self.peek() {
            Some('\'') => {
                self.bump();
                Ok(Token::Function)
            }
            Some('x') | Some('X') => self.lex_radix(start, 16),
            Some('o') | Some('O') => self.lex_radix(start, 8),
            Some('b') | Some('B') => self.lex_radix(start, 2),
            Some(d) if d.is_ascii_digit() => self.lex_explicit_radix(start),
            other => Err(LexError {
                message: format!("unsupported reader macro #{}", other.unwrap_or(' ')),
                offset: start,
            }),
        }
    }

    fn lex_radix(&mut self, start: usize, radix: u32) -> Result<Token, LexError> {
        self.bump(); // radix letter
        let digits = self.take_while(|c| !is_delimiter(c));
        i64::from_str_radix(&digits, radix)
            .map(Token::Int)
            .map_err(|_| LexError {
                message: format!("invalid base-{radix} integer `{digits}`"),
                offset: start,
            })
    }

    /// Lex `#NrDIGITS`, an integer in an explicit radix `N` (2..=36).
    fn lex_explicit_radix(&mut self, start: usize) -> Result<Token, LexError> {
        let radix_str = self.take_while(|c| c.is_ascii_digit());
        if self.peek() != Some('r') && self.peek() != Some('R') {
            return Err(LexError {
                message: "expected `r` in explicit-radix literal".to_string(),
                offset: start,
            });
        }
        self.bump(); // 'r'
        let radix: u32 = radix_str.parse().map_err(|_| LexError {
            message: format!("invalid radix `{radix_str}`"),
            offset: start,
        })?;
        if !(2..=36).contains(&radix) {
            return Err(LexError {
                message: format!("radix {radix} out of range 2..=36"),
                offset: start,
            });
        }
        let digits = self.take_while(|c| !is_delimiter(c));
        i64::from_str_radix(&digits, radix)
            .map(Token::Int)
            .map_err(|_| LexError {
                message: format!("invalid base-{radix} integer `{digits}`"),
                offset: start,
            })
    }

    /// Lex a symbol or number: a run of non-delimiter characters, classified
    /// after the fact.
    fn lex_atom(&mut self) -> Result<Token, LexError> {
        let text = self.take_symbol();
        debug_assert!(!text.is_empty());
        Ok(classify_atom(&text))
    }

    /// Consume a symbol, honoring `\` escapes (which let symbols contain
    /// delimiters).
    fn take_symbol(&mut self) -> String {
        let mut out = String::new();
        while let Some(c) = self.peek() {
            if c == '\\' {
                self.bump();
                if let Some(escaped) = self.bump() {
                    out.push(escaped);
                }
                continue;
            }
            if is_delimiter(c) {
                break;
            }
            out.push(c);
            self.bump();
        }
        out
    }

    fn take_while(&mut self, pred: impl Fn(char) -> bool) -> String {
        let mut out = String::new();
        while let Some(c) = self.peek() {
            if pred(c) {
                out.push(c);
                self.bump();
            } else {
                break;
            }
        }
        out
    }
}

/// Translate a backslash escape character to its value. Unknown escapes map to
/// the character itself, matching the Elisp reader for the common cases.
fn unescape(c: char) -> char {
    match c {
        'n' => '\n',
        't' => '\t',
        'r' => '\r',
        'f' => '\u{0c}',
        'v' => '\u{0b}',
        'e' => '\u{1b}',
        'a' => '\u{07}',
        'b' => '\u{08}',
        'd' => '\u{7f}',
        's' => ' ',
        '0' => '\0',
        other => other,
    }
}

/// Classify a finished atom string as a number, keyword, `t`/`nil`, or symbol.
fn classify_atom(text: &str) -> Token {
    if text == "." {
        return Token::Dot;
    }
    if text == "nil" {
        return Token::Nil;
    }
    if text == "t" {
        return Token::True;
    }
    if let Some(rest) = text.strip_prefix(':') {
        return Token::Keyword(rest.to_string());
    }
    if let Some(tok) = parse_number(text) {
        return tok;
    }
    Token::Symbol(text.to_string())
}

/// Try to parse `text` as an integer or float. Returns `None` for symbols.
fn parse_number(text: &str) -> Option<Token> {
    // Integers, allowing a leading `+`.
    let int_src = text.strip_prefix('+').unwrap_or(text);
    if let Ok(n) = int_src.parse::<i64>() {
        return Some(Token::Int(n));
    }
    // Floats must start with a digit or sign-then-digit and contain a `.` or
    // exponent, so that symbols like `1+`, `+`, `inf`, or `nan` stay symbols.
    let looks_floaty = text.contains('.') || text.contains('e') || text.contains('E');
    let first_meaningful = text.trim_start_matches(['+', '-']).chars().next();
    if looks_floaty
        && first_meaningful.is_some_and(|c| c.is_ascii_digit())
        && let Ok(f) = text.parse::<f64>()
    {
        return Some(Token::Float(f));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(input: &str) -> Vec<Token> {
        tokenize(input)
            .unwrap()
            .into_iter()
            .map(|s| s.token)
            .collect()
    }

    #[test]
    fn lexes_a_simple_call() {
        assert_eq!(
            toks("(+ 1 2.5)"),
            vec![
                Token::LParen,
                Token::Symbol("+".into()),
                Token::Int(1),
                Token::Float(2.5),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn lexes_reader_macros() {
        assert_eq!(toks("'a"), vec![Token::Quote, Token::Symbol("a".into())]);
        assert_eq!(
            toks("#'f"),
            vec![Token::Function, Token::Symbol("f".into())]
        );
        assert_eq!(
            toks(",@xs"),
            vec![Token::Splice, Token::Symbol("xs".into())]
        );
        assert_eq!(
            toks("`x"),
            vec![Token::Backquote, Token::Symbol("x".into())]
        );
    }

    #[test]
    fn lexes_strings_chars_keywords() {
        assert_eq!(toks(r#""a\nb""#), vec![Token::Str("a\nb".into())]);
        assert_eq!(toks("?\\n"), vec![Token::Char('\n')]);
        assert_eq!(toks("?a"), vec![Token::Char('a')]);
        assert_eq!(toks(":type"), vec![Token::Keyword("type".into())]);
        assert_eq!(toks("t nil"), vec![Token::True, Token::Nil]);
    }

    #[test]
    fn symbols_that_look_numeric_stay_symbols() {
        assert_eq!(toks("1+"), vec![Token::Symbol("1+".into())]);
        assert_eq!(toks("-"), vec![Token::Symbol("-".into())]);
        assert_eq!(toks("foo-bar"), vec![Token::Symbol("foo-bar".into())]);
    }

    #[test]
    fn lexes_radix_integers() {
        assert_eq!(toks("#xff"), vec![Token::Int(255)]);
        assert_eq!(toks("#o17"), vec![Token::Int(15)]);
        assert_eq!(toks("#b1010"), vec![Token::Int(10)]);
        assert_eq!(toks("#16rFF"), vec![Token::Int(255)]);
    }

    #[test]
    fn skips_comments() {
        assert_eq!(toks("; a comment\n42"), vec![Token::Int(42)]);
    }

    #[test]
    fn dotted_pair_dot_is_a_token() {
        assert_eq!(
            toks("(a . b)"),
            vec![
                Token::LParen,
                Token::Symbol("a".into()),
                Token::Dot,
                Token::Symbol("b".into()),
                Token::RParen,
            ]
        );
    }
}
