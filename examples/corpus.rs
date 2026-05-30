//! Parse a directory of `.el` files (a downloaded MELPA corpus) and report
//! which ones the lexer/parser cannot read or do not round-trip.
//!
//! SECURITY: files are ONLY parsed. They are never loaded, evaluated, or
//! byte-compiled.
//!
//! Usage: `cargo run --release --example corpus -- [DIR]`  (DIR defaults to
//! `corpus`). A Markdown failure report is written to `$CORPUS_REPORT`
//! (default `corpus-report.md`) for CI to use as an issue body.
//!
//! Exit codes: 0 = all good, 2 = parse/round-trip failures, 3 = no input.

use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use ferrel::*;

/// One failing file and why it failed.
struct Failure {
    file: String,
    kind: &'static str,
    detail: String,
    snippet: String,
}

fn main() -> ExitCode {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "corpus".to_string());
    let report_path =
        std::env::var("CORPUS_REPORT").unwrap_or_else(|_| "corpus-report.md".to_string());

    let mut files = Vec::new();
    collect_el(Path::new(&dir), &mut files);
    if files.is_empty() {
        eprintln!("no .el files found under {dir}");
        return ExitCode::from(3);
    }

    let mut failures = Vec::new();
    let mut parsed = 0usize;
    for path in &files {
        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        // Real packages are not all UTF-8; read lossily so latin-1 files are
        // still exercised rather than skipped.
        let src = String::from_utf8_lossy(&bytes);
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        match parse(&src) {
            Err(e) => failures.push(Failure {
                file: name,
                kind: "parse",
                detail: e.message,
                snippet: snippet_at(&src, e.offset),
            }),
            Ok(forms) => {
                parsed += 1;
                let rerendered = forms
                    .iter()
                    .map(Sexp::render)
                    .collect::<Vec<_>>()
                    .join("\n\n");
                match parse(&rerendered) {
                    Ok(reparsed) if reparsed == forms => {}
                    Ok(_) => failures.push(Failure {
                        file: name,
                        kind: "roundtrip",
                        detail: "AST changed after re-render".to_string(),
                        snippet: String::new(),
                    }),
                    Err(e) => failures.push(Failure {
                        file: name,
                        kind: "reparse",
                        detail: e.message,
                        snippet: snippet_at(&rerendered, e.offset),
                    }),
                }
            }
        }
    }

    let total = files.len();
    println!(
        "corpus: {total} files, {parsed} parsed, {} failure(s)",
        failures.len()
    );
    for f in failures.iter().take(20) {
        println!("FAIL [{}] {}: {}", f.kind, f.file, f.detail);
    }

    let _ = fs::write(&report_path, build_report(total, parsed, &failures));

    if failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

/// Recursively collect `.el` files under `dir`.
fn collect_el(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_el(&path, out);
        } else if path.extension().is_some_and(|e| e == "el") {
            out.push(path);
        }
    }
}

/// A short, single-line, ASCII-only snippet around byte `offset`.
fn snippet_at(src: &str, offset: usize) -> String {
    let start = offset.saturating_sub(30).min(src.len());
    let end = (offset + 30).min(src.len());
    // Snap to char boundaries.
    let start = (start..=offset.min(src.len()))
        .find(|&i| src.is_char_boundary(i))
        .unwrap_or(0);
    let end = (end..=src.len())
        .find(|&i| src.is_char_boundary(i))
        .unwrap_or(src.len());
    let mut out = String::new();
    for c in src[start..end].chars() {
        match c {
            '\n' | '\r' | '\t' => out.push(' '),
            c if c.is_ascii_graphic() || c == ' ' => out.push(c),
            _ => out.push('.'),
        }
    }
    out
}

/// Build the Markdown failure report handed to CI.
fn build_report(total: usize, parsed: usize, failures: &[Failure]) -> String {
    let mut out = String::new();
    out.push_str("# MELPA parser corpus report\n\n");
    out.push_str(&format!(
        "- files scanned: {total}\n- parsed cleanly: {parsed}\n- failures: {}\n\n",
        failures.len()
    ));
    if failures.is_empty() {
        out.push_str("All sampled files parsed and round-tripped.\n");
        return out;
    }
    out.push_str(
        "These files exposed gaps in the lexer/parser. Each is a candidate test \
         case. Files are sampled randomly from MELPA, so the set changes per run.\n\n",
    );
    let cap = 60;
    for f in failures.iter().take(cap) {
        out.push_str(&format!("## `{}` ({})\n\n", f.file, f.kind));
        out.push_str(&format!("- error: {}\n", f.detail));
        if !f.snippet.is_empty() {
            out.push_str(&format!("- near: `{}`\n", f.snippet));
        }
        out.push('\n');
    }
    if failures.len() > cap {
        out.push_str(&format!("...and {} more.\n", failures.len() - cap));
    }
    out
}
