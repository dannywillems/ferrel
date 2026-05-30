//! Name mapping between Rust and Elisp conventions.
//!
//! Rust identifiers are `snake_case`; Elisp symbols are `kebab-case`. The
//! transpiler maps every Rust name it emits through [`kebab_case`] so generated
//! symbols read like hand-written Elisp. Names that do not transform cleanly
//! (for example `1+` or `string-empty-p`) are handled by an explicit override
//! attribute on the FFI declaration; see [`crate::transpile`].

/// Convert a Rust identifier to an Elisp `kebab-case` symbol.
///
/// Internal underscores become hyphens and ASCII letters are lowercased, so
/// both `snake_case` variables and `SCREAMING_SNAKE_CASE` constants land on the
/// all-lowercase, hyphenated convention that Elisp uses: `MAX_ITEMS` becomes
/// `max-items` and `my_function` becomes `my-function`. A trailing `_p`
/// predicate spelling becomes `-p`, the Elisp predicate convention.
///
/// Leading underscores are preserved, because both Rust and Elisp use a
/// leading underscore to mark an intentionally unused binding: a Rust `_i` loop
/// variable becomes the Elisp `_i`, which the byte-compiler accepts without an
/// unused-variable warning. A bare `_` stays `_`.
///
/// ```
/// use ferrel::transpile::kebab_case;
///
/// assert_eq!(kebab_case("my_function"), "my-function");
/// assert_eq!(kebab_case("MAX_ITEMS"), "max-items");
/// assert_eq!(kebab_case("expand_file_name"), "expand-file-name");
/// assert_eq!(kebab_case("already-kebab"), "already-kebab");
/// assert_eq!(kebab_case("_i"), "_i");
/// ```
#[must_use]
pub fn kebab_case(name: &str) -> String {
    let leading = name.len() - name.trim_start_matches('_').len();
    let mut out = String::with_capacity(name.len());
    for _ in 0..leading {
        out.push('_');
    }
    for c in name[leading..].chars() {
        out.push(if c == '_' {
            '-'
        } else {
            c.to_ascii_lowercase()
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_snake_to_kebab() {
        assert_eq!(kebab_case("foo_bar_baz"), "foo-bar-baz");
        assert_eq!(kebab_case("point"), "point");
        assert_eq!(kebab_case("string_empty_p"), "string-empty-p");
    }

    #[test]
    fn leaves_existing_hyphens_alone() {
        assert_eq!(kebab_case("with-current-buffer"), "with-current-buffer");
    }
}
