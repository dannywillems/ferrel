//! Read an existing `.el` file, parse it to the `Sexp` AST, re-render it, and
//! check that parsing the re-rendered text yields an identical AST.
//!
//! This exercises the read side of the pipeline (lexer + parser) against real
//! Emacs Lisp and proves the round-trip is stable.
//!
//! Run with: `cargo run --example roundtrip -- path/to/file.el [more.el ...]`

use ferrel::*;

fn main() -> std::process::ExitCode {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: roundtrip <file.el> [file.el ...]");
        return std::process::ExitCode::FAILURE;
    }

    let mut ok = true;
    for path in &paths {
        match check(path) {
            Ok(count) => println!("OK    {path}: {count} forms, round-trip stable"),
            Err(msg) => {
                println!("FAIL  {path}: {msg}");
                ok = false;
            }
        }
    }

    if ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    }
}

fn check(path: &str) -> Result<usize, String> {
    let src = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let forms = parse(&src).map_err(|e| e.to_string())?;
    let rerendered = forms
        .iter()
        .map(Sexp::render)
        .collect::<Vec<_>>()
        .join("\n\n");
    let reparsed = parse(&rerendered).map_err(|e| format!("reparse: {e}"))?;
    if forms == reparsed {
        Ok(forms.len())
    } else {
        Err("AST changed after re-render".to_string())
    }
}
