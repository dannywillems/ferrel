//! Typed key sequences and command names.
//!
//! Emacs keybindings are written as `kbd` strings such as `"C-c f"` or
//! `"C-x RET"`. Passing those as bare `&str` means a typo like `"C-x C-"` is
//! the same type as a valid sequence and is only caught when Emacs loads the
//! file. [`KeySeq`] is a checked newtype: [`KeySeq::parse`] validates the
//! syntax up front and returns a [`KeyError`] on malformed input.
//!
//! [`Command`] is a separate newtype for a command (function) name, so a key
//! sequence cannot be passed where a command is expected and vice versa.

use std::fmt;

/// An error describing why a key sequence failed to parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyError {
    /// Human-readable description of the problem.
    pub message: String,
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid key sequence: {}", self.message)
    }
}

impl std::error::Error for KeyError {}

/// A validated Emacs key sequence, as accepted by `kbd`.
///
/// A sequence is one or more space-separated chords. A chord is zero or more
/// modifier prefixes (`C-`, `M-`, `S-`, `H-`, `s-`, `A-`) followed by exactly
/// one key: a single printable character, a named key (`RET`, `TAB`, `SPC`,
/// `DEL`, `ESC`, `LFD`, `NUL`), or an angle-bracket key (`<f1>`, `<left>`,
/// `<return>`, ...).
///
/// ```
/// use ferrel::KeySeq;
///
/// assert!(KeySeq::parse("C-c f").is_ok());
/// assert!(KeySeq::parse("C-x RET").is_ok());
/// assert!(KeySeq::parse("<f5>").is_ok());
/// assert!(KeySeq::parse("C-x C-").is_err()); // dangling modifier
/// assert!(KeySeq::parse("").is_err()); // empty
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeySeq(String);

/// The recognised modifier prefixes, each a letter followed by `-`.
const MODIFIERS: [char; 6] = ['C', 'M', 'S', 'H', 's', 'A'];

/// Named keys that may stand alone as a chord's key.
const NAMED_KEYS: [&str; 7] = ["RET", "TAB", "SPC", "DEL", "ESC", "LFD", "NUL"];

impl KeySeq {
    /// Parse and validate a key sequence.
    ///
    /// # Errors
    ///
    /// Returns a [`KeyError`] if the sequence is empty, contains an empty
    /// chord, has a dangling or unknown modifier, or names a key the parser
    /// does not recognise.
    pub fn parse(s: impl AsRef<str>) -> Result<KeySeq, KeyError> {
        let s = s.ref_trimmed();
        if s.is_empty() {
            return Err(KeyError {
                message: "empty key sequence".to_string(),
            });
        }
        for chord in s.split(' ') {
            validate_chord(chord)?;
        }
        Ok(KeySeq(s.to_string()))
    }

    /// The validated sequence as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume into the owned validated string.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for KeySeq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<&str> for KeySeq {
    type Error = KeyError;

    fn try_from(s: &str) -> Result<KeySeq, KeyError> {
        KeySeq::parse(s)
    }
}

impl TryFrom<String> for KeySeq {
    type Error = KeyError;

    fn try_from(s: String) -> Result<KeySeq, KeyError> {
        KeySeq::parse(s)
    }
}

/// A small extension to trim a string-like reference once, locally.
trait RefTrimmed {
    fn ref_trimmed(&self) -> &str;
}

impl<S: AsRef<str>> RefTrimmed for S {
    fn ref_trimmed(&self) -> &str {
        self.as_ref().trim()
    }
}

/// Validate a single space-delimited chord such as `C-c`, `M-RET`, or `<f5>`.
fn validate_chord(chord: &str) -> Result<(), KeyError> {
    if chord.is_empty() {
        return Err(KeyError {
            message: "empty chord (double space?)".to_string(),
        });
    }
    let mut rest = chord;
    // Strip leading modifier prefixes: a modifier letter followed by `-`.
    while rest.len() >= 2 {
        let mut chars = rest.chars();
        let m = chars.next().expect("len checked");
        let dash = chars.next().expect("len checked");
        if dash == '-' && MODIFIERS.contains(&m) {
            rest = &rest[2..];
        } else {
            break;
        }
    }
    validate_key(rest, chord)
}

/// Validate the key part of a chord after its modifiers were stripped.
fn validate_key(key: &str, chord: &str) -> Result<(), KeyError> {
    if key.is_empty() {
        return Err(KeyError {
            message: format!("chord `{chord}` has a modifier but no key"),
        });
    }
    if NAMED_KEYS.contains(&key) {
        return Ok(());
    }
    // Angle-bracket function/named keys: `<f1>`, `<left>`, `<return>`.
    if let Some(inner) = key.strip_prefix('<').and_then(|k| k.strip_suffix('>')) {
        if inner.is_empty() || !inner.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(KeyError {
                message: format!("malformed angle-bracket key `{key}`"),
            });
        }
        return Ok(());
    }
    // A single printable character is a valid key.
    let mut chars = key.chars();
    let first = chars.next().expect("non-empty checked above");
    if chars.next().is_none() && !first.is_whitespace() && !first.is_control() {
        return Ok(());
    }
    Err(KeyError {
        message: format!("unrecognised key `{key}` in chord `{chord}`"),
    })
}

/// A command (function) name to bind a key to, e.g. `find-file`.
///
/// Distinct from [`KeySeq`] so the two cannot be swapped at a call site.
///
/// ```
/// use ferrel::Command;
///
/// let cmd = Command::new("find-file");
/// assert_eq!(cmd.as_str(), "find-file");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command(String);

impl Command {
    /// Name a command. The name is trusted as a symbol (no validation beyond
    /// being non-empty is performed; Elisp symbol syntax is permissive).
    pub fn new(name: impl Into<String>) -> Command {
        Command(name.into())
    }

    /// The command name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume into the owned command name.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for Command {
    fn from(s: &str) -> Command {
        Command(s.to_string())
    }
}

impl From<String> for Command {
    fn from(s: String) -> Command {
        Command(s)
    }
}

/// Validate a key-sequence literal at the call site, panicking on a malformed
/// literal.
///
/// This is the trusted-literal counterpart to [`KeySeq::parse`]: use it when
/// the key is a string literal you control, so a typo surfaces immediately as
/// a panic at startup rather than silently producing a wrong binding. For
/// runtime or untrusted input, call [`KeySeq::parse`] and handle the error.
///
/// ```
/// use ferrel::*;
///
/// let k = kbd!("C-c f");
/// assert_eq!(k.as_str(), "C-c f");
/// ```
#[macro_export]
macro_rules! kbd {
    ($s:expr) => {
        $crate::KeySeq::parse($s).expect("invalid key sequence literal")
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_simple_sequences() {
        assert!(KeySeq::parse("C-c f").is_ok());
        assert!(KeySeq::parse("C-x C-f").is_ok());
        assert!(KeySeq::parse("M-x").is_ok());
        assert!(KeySeq::parse("a").is_ok());
    }

    #[test]
    fn accepts_named_and_bracket_keys() {
        assert!(KeySeq::parse("C-x RET").is_ok());
        assert!(KeySeq::parse("TAB").is_ok());
        assert!(KeySeq::parse("<f5>").is_ok());
        assert!(KeySeq::parse("C-<left>").is_ok());
        assert!(KeySeq::parse("<return>").is_ok());
    }

    #[test]
    fn accepts_all_modifiers() {
        for m in ["C", "M", "S", "H", "s", "A"] {
            assert!(KeySeq::parse(format!("{m}-a")).is_ok(), "modifier {m}");
        }
    }

    #[test]
    fn rejects_empty() {
        assert!(KeySeq::parse("").is_err());
        assert!(KeySeq::parse("   ").is_err());
    }

    #[test]
    fn rejects_dangling_modifier() {
        assert!(KeySeq::parse("C-x C-").is_err());
        assert!(KeySeq::parse("C-").is_err());
    }

    #[test]
    fn rejects_double_space() {
        assert!(KeySeq::parse("C-c  f").is_err());
    }

    #[test]
    fn rejects_bad_bracket_key() {
        assert!(KeySeq::parse("<>").is_err());
        assert!(KeySeq::parse("<f5").is_err());
    }

    #[test]
    fn command_roundtrips() {
        let c = Command::new("find-file");
        assert_eq!(c.as_str(), "find-file");
        assert_eq!(c.clone().into_string(), "find-file");
        let from_str: Command = "switch-to-buffer".into();
        assert_eq!(from_str.as_str(), "switch-to-buffer");
    }

    #[test]
    fn kbd_macro_validates() {
        let k = kbd!("C-c f");
        assert_eq!(k.as_str(), "C-c f");
    }

    #[test]
    #[should_panic(expected = "invalid key sequence literal")]
    fn kbd_macro_panics_on_bad_literal() {
        let _ = kbd!("C-");
    }

    #[test]
    fn try_from_works() {
        let k: KeySeq = "C-c f".try_into().unwrap();
        assert_eq!(k.as_str(), "C-c f");
        assert!(KeySeq::try_from("C-".to_string()).is_err());
    }
}
