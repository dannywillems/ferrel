//! Assembling top-level forms into a complete, loadable `.el` file.

use std::{io, path::Path};

use crate::{form::Defun, sexp::Sexp};

/// An Emacs Lisp package: a named collection of top-level forms plus the
/// metadata header that Emacs and `package.el` expect.
///
/// [`Package::render`] produces a complete file, including the
/// `lexical-binding` cookie, the `;;; Commentary:` / `;;; Code:` sections, the
/// trailing `(provide 'name)`, and the `ends here` footer.
pub struct Package {
    name: String,
    summary: String,
    author: Option<String>,
    version: Option<String>,
    emacs_version: Option<String>,
    requires: Vec<(String, String)>,
    keywords: Vec<String>,
    url: Option<String>,
    commentary: Option<String>,
    forms: Vec<Sexp>,
}

impl Package {
    /// Create a package named `name` (without the `.el` extension) with a
    /// one-line `summary`.
    pub fn new(name: impl Into<String>, summary: impl Into<String>) -> Package {
        Package {
            name: name.into(),
            summary: summary.into(),
            author: None,
            version: None,
            emacs_version: None,
            requires: Vec::new(),
            keywords: Vec::new(),
            url: None,
            commentary: None,
            forms: Vec::new(),
        }
    }

    /// Set the `;; Author:` header.
    pub fn author(mut self, author: impl Into<String>) -> Package {
        self.author = Some(author.into());
        self
    }

    /// Set the package `;; Version:`.
    pub fn version(mut self, version: impl Into<String>) -> Package {
        self.version = Some(version.into());
        self
    }

    /// Require a minimum Emacs version (adds `(emacs "X")` to
    /// `Package-Requires`).
    pub fn emacs_version(mut self, version: impl Into<String>) -> Package {
        self.emacs_version = Some(version.into());
        self
    }

    /// Add a `Package-Requires` dependency, e.g. `("dash", "2.19")`.
    pub fn requires(mut self, package: impl Into<String>, version: impl Into<String>) -> Package {
        self.requires.push((package.into(), version.into()));
        self
    }

    /// Add a `;; Keywords:` entry.
    pub fn keyword(mut self, keyword: impl Into<String>) -> Package {
        self.keywords.push(keyword.into());
        self
    }

    /// Set the `;; URL:` / homepage.
    pub fn url(mut self, url: impl Into<String>) -> Package {
        self.url = Some(url.into());
        self
    }

    /// Set the `;;; Commentary:` prose block.
    pub fn commentary(mut self, text: impl Into<String>) -> Package {
        self.commentary = Some(text.into());
        self
    }

    /// Append a raw top-level form.
    pub fn form(mut self, form: impl Into<Sexp>) -> Package {
        self.forms.push(form.into());
        self
    }

    /// Append a function definition.
    pub fn defun(self, defun: Defun) -> Package {
        self.form(defun.build())
    }

    /// Append a top-level form in place, taking `&mut self`.
    ///
    /// Unlike [`Package::form`], this does not consume and return the package,
    /// so it composes inside a loop without rebinding `pkg` each iteration.
    ///
    /// ```
    /// use ferrel::*;
    ///
    /// let mut pkg = Package::new("demo", "Demo.");
    /// for mode in ["recentf-mode", "save-place-mode"] {
    ///     pkg.push_form(enable_mode(mode));
    /// }
    /// assert!(pkg.render().contains("(recentf-mode 1)"));
    /// ```
    pub fn push_form(&mut self, form: impl Into<Sexp>) -> &mut Package {
        self.forms.push(form.into());
        self
    }

    /// Append many top-level forms from an iterator, consuming and returning
    /// the package so it chains with the other consuming builders.
    ///
    /// ```
    /// use ferrel::*;
    ///
    /// let modes = ["recentf-mode", "save-place-mode"];
    /// let pkg = Package::new("demo", "Demo.")
    ///     .extend_forms(modes.iter().map(|m| enable_mode(*m)));
    /// assert!(pkg.render().contains("(save-place-mode 1)"));
    /// ```
    pub fn extend_forms<S: Into<Sexp>, I: IntoIterator<Item = S>>(mut self, forms: I) -> Package {
        self.forms.extend(forms.into_iter().map(Into::into));
        self
    }

    /// Render the complete `.el` file as a string.
    pub fn render(&self) -> String {
        let mut out = String::new();

        // Header line with the lexical-binding cookie.
        out.push_str(&format!(
            ";;; {}.el --- {} -*- lexical-binding: t; -*-\n\n",
            self.name, self.summary
        ));

        if let Some(author) = &self.author {
            out.push_str(&format!(";; Author: {author}\n"));
        }
        if let Some(version) = &self.version {
            out.push_str(&format!(";; Version: {version}\n"));
        }
        if self.emacs_version.is_some() || !self.requires.is_empty() {
            let mut reqs: Vec<String> = Vec::new();
            if let Some(ev) = &self.emacs_version {
                reqs.push(format!("(emacs \"{ev}\")"));
            }
            for (pkg, ver) in &self.requires {
                reqs.push(format!("({pkg} \"{ver}\")"));
            }
            out.push_str(&format!(";; Package-Requires: ({})\n", reqs.join(" ")));
        }
        if !self.keywords.is_empty() {
            out.push_str(&format!(";; Keywords: {}\n", self.keywords.join(", ")));
        }
        if let Some(url) = &self.url {
            out.push_str(&format!(";; URL: {url}\n"));
        }

        // Commentary section.
        out.push_str("\n;;; Commentary:\n");
        if let Some(text) = &self.commentary {
            for line in text.lines() {
                if line.is_empty() {
                    out.push_str(";;\n");
                } else {
                    out.push_str(&format!(";; {line}\n"));
                }
            }
        } else {
            out.push_str(&format!(";; {}\n", self.summary));
        }

        // Code section.
        out.push_str("\n;;; Code:\n\n");
        for form in &self.forms {
            out.push_str(&form.render());
            out.push_str("\n\n");
        }

        // Footer.
        out.push_str(&format!("(provide '{})\n", self.name));
        out.push_str(&format!(";;; {}.el ends here\n", self.name));
        out
    }

    /// Write the rendered package to `path`.
    pub fn write(&self, path: impl AsRef<Path>) -> io::Result<()> {
        std::fs::write(path, self.render())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        form::global_set_key,
        typed::{message, string},
    };

    #[test]
    fn renders_loadable_skeleton() {
        let pkg = Package::new("ferrel-demo", "A demo.")
            .author("Test")
            .emacs_version("27.1")
            .defun(
                Defun::new("ferrel-demo-hi")
                    .interactive()
                    .body([message(string("hi"), [])]),
            )
            .form(global_set_key("C-c h", "ferrel-demo-hi"));
        let out = pkg.render();
        assert!(out.starts_with(";;; ferrel-demo.el --- A demo. -*- lexical-binding: t; -*-"));
        assert!(out.contains(";; Package-Requires: ((emacs \"27.1\"))"));
        assert!(out.contains("(provide 'ferrel-demo)"));
        assert!(out.trim_end().ends_with(";;; ferrel-demo.el ends here"));
    }

    #[test]
    fn push_form_appends_in_place() {
        use crate::form::enable_mode;
        let mut pkg = Package::new("demo", "Demo.");
        for mode in ["recentf-mode", "save-place-mode"] {
            pkg.push_form(enable_mode(mode));
        }
        let out = pkg.render();
        assert!(out.contains("(recentf-mode 1)"));
        assert!(out.contains("(save-place-mode 1)"));
    }

    #[test]
    fn extend_forms_consumes_an_iterator() {
        use crate::form::enable_mode;
        let modes = ["recentf-mode", "save-place-mode"];
        let pkg = Package::new("demo", "Demo.").extend_forms(modes.iter().map(|m| enable_mode(*m)));
        let out = pkg.render();
        assert!(out.contains("(recentf-mode 1)"));
        assert!(out.contains("(save-place-mode 1)"));
    }
}
