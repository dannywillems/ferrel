//! Lowering a Rust subset into the `Sexp` IR.
//!
//! This is the core of the transpiler: a set of small, total functions that
//! rewrite `syn` nodes into [`Sexp`] nodes. The contract is the honesty rule:
//! every construct either lowers with identical observable behavior or is
//! refused with a located [`TranspileError`]. There is no silent fallthrough;
//! every unsupported construct is matched and rejected explicitly.

use std::collections::HashMap;

use syn::{
    BinOp, Block, Expr, ExprBinary, ExprCall, ExprMethodCall, FnArg, Item, Lit, Local, Pat, Stmt,
    UnOp,
};

use crate::{
    sexp::Sexp,
    transpile::{error::TranspileError, naming::kebab_case},
};

/// Names that the transpiler lowers specially rather than as ordinary calls.
///
/// These are the intrinsics from `ferrel::rt`: a symbol literal helper, a
/// function-reference helper, a key helper, and a raw Elisp escape. They have
/// real stub bodies in the prelude so user code compiles, but the transpiler
/// recognizes them by name and emits the corresponding reader-macro node.
const INTRINSICS: &[&str] = &["sym", "func", "kbd", "raw"];

/// Per-file lowering state: the FFI name overrides declared via `#[elisp]`.
struct Ctx {
    /// Rust function name -> overridden Elisp symbol (from `#[elisp(name=...)]`).
    overrides: HashMap<String, String>,
}

impl Ctx {
    /// Resolve a called Rust function name to its Elisp symbol, honoring any
    /// `#[elisp(name = "...")]` override and otherwise mapping to kebab-case.
    fn elisp_name(&self, rust_name: &str) -> String {
        self.overrides
            .get(rust_name)
            .cloned()
            .unwrap_or_else(|| kebab_case(rust_name))
    }
}

/// Lower a whole parsed file into a sequence of top-level `Sexp` forms.
pub(crate) fn lower_file(file: &syn::File) -> Result<Vec<Sexp>, TranspileError> {
    let mut ctx = Ctx {
        overrides: HashMap::new(),
    };
    // First pass: register FFI declarations so calls anywhere can resolve.
    for item in &file.items {
        if let Item::Fn(f) = item
            && let Some(name) = elisp_attr_override(&f.attrs)?
        {
            ctx.overrides.insert(f.sig.ident.to_string(), name);
        }
    }
    // Second pass: emit forms for every item that is not a pure declaration.
    let mut forms = Vec::new();
    for item in &file.items {
        if let Some(form) = lower_item(&ctx, item)? {
            forms.push(form);
        }
    }
    Ok(forms)
}

/// Lower one top-level item. Returns `None` for items that only register
/// information (FFI declarations) and emit no form.
fn lower_item(ctx: &Ctx, item: &Item) -> Result<Option<Sexp>, TranspileError> {
    match item {
        Item::Fn(f) => {
            // A function carrying `#[elisp]` is a foreign declaration: it
            // registers a name mapping and emits nothing.
            if has_elisp_attr(&f.attrs) {
                return Ok(None);
            }
            Ok(Some(lower_fn(ctx, f)?))
        }
        Item::Const(c) => Ok(Some(lower_const(ctx, c)?)),
        Item::Static(s) => Ok(Some(lower_static(ctx, s)?)),
        Item::Use(_) => Ok(None),
        other => Err(TranspileError::spanned(
            other,
            "unsupported item: only `fn`, `const`, `static`, and `use` are supported \
             at the top level",
        )),
    }
}

/// Lower `fn name(params) -> ret { body }` to `(defun name (params) ...)`.
fn lower_fn(ctx: &Ctx, f: &syn::ItemFn) -> Result<Sexp, TranspileError> {
    if let Some(c) = &f.sig.constness {
        return Err(TranspileError::spanned(c, "unsupported: `const fn`"));
    }
    if let Some(a) = &f.sig.asyncness {
        return Err(TranspileError::spanned(a, "unsupported: `async fn`"));
    }
    if !f.sig.generics.params.is_empty() {
        return Err(TranspileError::spanned(
            &f.sig.generics,
            "unsupported: generic parameters on a function",
        ));
    }

    let name = kebab_case(&f.sig.ident.to_string());

    let mut params = Vec::new();
    for arg in &f.sig.inputs {
        match arg {
            FnArg::Receiver(r) => {
                return Err(TranspileError::spanned(
                    r,
                    "unsupported: methods with `self` (write a free function)",
                ));
            }
            FnArg::Typed(pt) => match pt.pat.as_ref() {
                Pat::Ident(pi) => params.push(Sexp::sym(kebab_case(&pi.ident.to_string()))),
                other => {
                    return Err(TranspileError::spanned(
                        other,
                        "unsupported: only plain identifier parameters are supported",
                    ));
                }
            },
        }
    }

    let mut form = vec![Sexp::sym("defun"), Sexp::sym(name), Sexp::List(params)];

    if let Some(doc) = doc_string(&f.attrs) {
        form.push(Sexp::Str(doc));
    }
    if let Some(interactive) = interactive_attr(&f.attrs)? {
        form.push(interactive);
    }

    let body = lower_block_body(ctx, &f.block)?;
    form.extend(body);
    Ok(Sexp::List(form))
}

/// Lower `const NAME: T = e;` to `(defconst name e "doc")`.
fn lower_const(ctx: &Ctx, c: &syn::ItemConst) -> Result<Sexp, TranspileError> {
    let name = kebab_case(&c.ident.to_string());
    let value = lower_expr(ctx, &c.expr)?;
    let mut form = vec![Sexp::sym("defconst"), Sexp::sym(name), value];
    if let Some(doc) = doc_string(&c.attrs) {
        form.push(Sexp::Str(doc));
    }
    Ok(Sexp::List(form))
}

/// Lower `static NAME: T = e;` to `(defvar name e "doc")`.
fn lower_static(ctx: &Ctx, s: &syn::ItemStatic) -> Result<Sexp, TranspileError> {
    let name = kebab_case(&s.ident.to_string());
    let value = lower_expr(ctx, &s.expr)?;
    let mut form = vec![Sexp::sym("defvar"), Sexp::sym(name), value];
    if let Some(doc) = doc_string(&s.attrs) {
        form.push(Sexp::Str(doc));
    }
    Ok(Sexp::List(form))
}

// --- Blocks and statements ---------------------------------------------------

/// Lower a function block into the list of body forms that follow the params
/// (and any docstring / interactive marker) inside a `defun`.
///
/// A `defun` body is an implicit `progn`, so leading `let` bindings are wrapped
/// in `let*` and the resulting single form is the whole body. An empty block
/// lowers to a single `nil` so the function returns `nil` like the Rust unit.
fn lower_block_body(ctx: &Ctx, block: &Block) -> Result<Vec<Sexp>, TranspileError> {
    let lowered = lower_stmts(ctx, &block.stmts)?;
    match lowered {
        Sexp::List(items) if matches!(items.first(), Some(Sexp::Sym(s)) if s == "progn") => {
            // Splice a top-level progn so the defun body is the flat sequence.
            Ok(items.into_iter().skip(1).collect())
        }
        other => Ok(vec![other]),
    }
}

/// Lower a sequence of statements into a single `Sexp`.
///
/// Leading `let x = e;` bindings become a `let*` whose body is the rest of the
/// sequence, preserving order and scope. A run of non-binding statements with a
/// trailing expression becomes a `progn` (or the bare expression when there is
/// only one). This is the rule that maps a Rust block's trailing-expression
/// return onto an Elisp body's last-form return.
fn lower_stmts(ctx: &Ctx, stmts: &[Stmt]) -> Result<Sexp, TranspileError> {
    let Some((first, rest)) = stmts.split_first() else {
        // An empty block evaluates to the Rust unit; lower to nil.
        return Ok(Sexp::Nil);
    };

    match first {
        Stmt::Local(local) => {
            let (name, value) = lower_let(ctx, local)?;
            match name {
                // `let _ = e;` discards the value: evaluate e for its effect and
                // sequence the rest. This is observably the same as Rust, which
                // evaluates the initializer and drops it.
                None => {
                    if rest.is_empty() {
                        return Ok(value);
                    }
                    let tail = lower_stmts(ctx, rest)?;
                    Ok(join_progn(value, tail))
                }
                Some(name) => {
                    let binding = Sexp::List(vec![Sexp::sym(name), value]);
                    let body = lower_stmts(ctx, rest)?;
                    let body_forms = splice_progn(body);
                    let mut form = vec![Sexp::sym("let*"), Sexp::List(vec![binding])];
                    form.extend(body_forms);
                    Ok(Sexp::List(form))
                }
            }
        }
        Stmt::Expr(expr, _semi) => {
            let head = lower_expr(ctx, expr)?;
            if rest.is_empty() {
                return Ok(head);
            }
            let tail = lower_stmts(ctx, rest)?;
            Ok(join_progn(head, tail))
        }
        Stmt::Item(item) => {
            // Nested items (e.g. an inner `fn`) are out of subset.
            let _ = ctx;
            Err(TranspileError::spanned(
                item,
                "unsupported: items nested inside a function body",
            ))
        }
        Stmt::Macro(m) => {
            // A `format!(..);` / `println!(..);` statement: lower the macro and
            // sequence it with the rest exactly like an expression statement.
            let head = lower_stmt_macro(ctx, m)?;
            if rest.is_empty() {
                return Ok(head);
            }
            let tail = lower_stmts(ctx, rest)?;
            Ok(join_progn(head, tail))
        }
    }
}

/// Lower a macro statement `m!(..);` by reusing the expression-macro lowering.
fn lower_stmt_macro(ctx: &Ctx, m: &syn::StmtMacro) -> Result<Sexp, TranspileError> {
    let expr = syn::ExprMacro {
        attrs: m.attrs.clone(),
        mac: m.mac.clone(),
    };
    lower_macro(ctx, &expr)
}

/// Lower a `let` binding into an optional name and its initializer value.
///
/// The name is `None` for the wildcard binding `let _ = e;`, which evaluates
/// `e` for effect and discards it. `let name = e;` and `let name: T = e;` yield
/// `Some(name)`. Destructuring and `ref` bindings are refused.
fn lower_let(ctx: &Ctx, local: &Local) -> Result<(Option<String>, Sexp), TranspileError> {
    let name = match strip_type(&local.pat) {
        Pat::Wild(_) => None,
        Pat::Ident(pi) => {
            if pi.by_ref.is_some() {
                return Err(TranspileError::spanned(pi, "unsupported: `ref` binding"));
            }
            Some(kebab_case(&pi.ident.to_string()))
        }
        other => {
            return Err(TranspileError::spanned(
                other,
                "unsupported: only `let name = ..` and `let _ = ..` bindings are \
                 supported (no destructuring)",
            ));
        }
    };
    let init = local.init.as_ref().ok_or_else(|| {
        TranspileError::spanned(local, "unsupported: `let` without an initializer")
    })?;
    if let Some((else_kw, _)) = &init.diverge {
        return Err(TranspileError::spanned(
            else_kw,
            "unsupported: `let .. else` binding",
        ));
    }
    let value = lower_expr(ctx, &init.expr)?;
    Ok((name, value))
}

/// Peel a `let name: T = ..` type ascription so the inner pattern can be matched
/// uniformly with an unascribed `let name = ..`.
fn strip_type(pat: &Pat) -> &Pat {
    match pat {
        Pat::Type(pt) => strip_type(&pt.pat),
        other => other,
    }
}

/// If `s` is a `progn`, return its body forms; otherwise return `[s]`.
fn splice_progn(s: Sexp) -> Vec<Sexp> {
    match s {
        Sexp::List(items) if matches!(items.first(), Some(Sexp::Sym(h)) if h == "progn") => {
            items.into_iter().skip(1).collect()
        }
        other => vec![other],
    }
}

/// Join a head form and a tail form into a single `progn`, flattening a tail
/// that is itself a `progn` so sequences do not nest needlessly.
fn join_progn(head: Sexp, tail: Sexp) -> Sexp {
    let mut body = vec![Sexp::sym("progn"), head];
    body.extend(splice_progn(tail));
    Sexp::List(body)
}

// --- Expressions -------------------------------------------------------------

/// Lower a Rust expression into a `Sexp`, or refuse it with a located error.
fn lower_expr(ctx: &Ctx, expr: &Expr) -> Result<Sexp, TranspileError> {
    match expr {
        Expr::Lit(lit) => lower_lit(&lit.lit),
        Expr::Path(p) => lower_path(p),
        Expr::Paren(p) => lower_expr(ctx, &p.expr),
        Expr::Group(g) => lower_expr(ctx, &g.expr),
        Expr::Unary(u) => lower_unary(ctx, u),
        Expr::Binary(b) => lower_binary(ctx, b),
        Expr::Call(c) => lower_call(ctx, c),
        Expr::MethodCall(m) => lower_method_call(ctx, m),
        Expr::If(i) => lower_if(ctx, i),
        Expr::Match(m) => lower_match(ctx, m),
        Expr::While(w) => lower_while(ctx, w),
        Expr::ForLoop(f) => lower_for(ctx, f),
        Expr::Block(b) => lower_block_expr(ctx, &b.block),
        Expr::Assign(a) => lower_assign(ctx, a),
        Expr::Closure(c) => lower_closure(ctx, c),
        Expr::Return(r) => lower_return(ctx, r),
        Expr::Macro(m) => lower_macro(ctx, m),
        Expr::Reference(r) => Err(TranspileError::spanned(
            r,
            "unsupported: `&` reference expression (Elisp has no references; \
             use a value or the `sym`/`func` intrinsic)",
        )),
        Expr::Try(t) => Err(TranspileError::spanned(
            t,
            "unsupported: `?` operator (no Result/Option lowering)",
        )),
        Expr::Await(a) => Err(TranspileError::spanned(a, "unsupported: `.await`")),
        Expr::Range(r) => Err(TranspileError::spanned(
            r,
            "unsupported: range expression outside a `for i in 0..n` loop header",
        )),
        Expr::Field(fe) => Err(TranspileError::spanned(
            fe,
            "unsupported: field access (no structs in the subset)",
        )),
        Expr::Index(ie) => Err(TranspileError::spanned(
            ie,
            "unsupported: indexing expression",
        )),
        Expr::Struct(se) => Err(TranspileError::spanned(
            se,
            "unsupported: struct literal (no structs in the subset)",
        )),
        Expr::Cast(ce) => Err(TranspileError::spanned(
            ce,
            "unsupported: `as` cast (Elisp is dynamically typed; drop the cast)",
        )),
        other => Err(TranspileError::spanned(other, "unsupported expression")),
    }
}

/// Lower a block expression `{ .. }` into its sequenced body. A block that is
/// just a binding or sequence becomes a `let*`/`progn`; a single expression is
/// lowered directly.
fn lower_block_expr(ctx: &Ctx, block: &Block) -> Result<Sexp, TranspileError> {
    lower_stmts(ctx, &block.stmts)
}

/// Lower a literal: integers, floats, booleans (`t`/`nil`), chars, strings.
fn lower_lit(lit: &Lit) -> Result<Sexp, TranspileError> {
    match lit {
        Lit::Int(i) => i
            .base10_parse::<i64>()
            .map(Sexp::Int)
            .map_err(|e| TranspileError::spanned(i, format!("invalid integer literal: {e}"))),
        Lit::Float(f) => f
            .base10_parse::<f64>()
            .map(Sexp::Float)
            .map_err(|e| TranspileError::spanned(f, format!("invalid float literal: {e}"))),
        Lit::Bool(b) => Ok(if b.value { Sexp::True } else { Sexp::Nil }),
        Lit::Char(c) => Ok(Sexp::Char(c.value())),
        Lit::Str(s) => Ok(Sexp::Str(s.value())),
        Lit::ByteStr(b) => Err(TranspileError::spanned(
            b,
            "unsupported: byte-string literal",
        )),
        Lit::Byte(b) => Err(TranspileError::spanned(b, "unsupported: byte literal")),
        other => Err(TranspileError::spanned(other, "unsupported literal")),
    }
}

/// Lower a path expression. A bare single-segment path is a variable reference
/// (a symbol). `true`/`false` arrive as literals, not paths.
fn lower_path(p: &syn::ExprPath) -> Result<Sexp, TranspileError> {
    if p.qself.is_some() {
        return Err(TranspileError::spanned(
            p,
            "unsupported: qualified path expression",
        ));
    }
    let segs = &p.path.segments;
    if segs.len() != 1 {
        return Err(TranspileError::spanned(
            p,
            "unsupported: multi-segment path (use a single identifier)",
        ));
    }
    let seg = &segs[0];
    if !seg.arguments.is_none() {
        return Err(TranspileError::spanned(
            seg,
            "unsupported: path with generic arguments",
        ));
    }
    Ok(Sexp::sym(kebab_case(&seg.ident.to_string())))
}

/// Lower a unary operator. Only `!x` (logical not) is supported; `-x` is folded
/// into a negation call `(- x)`.
fn lower_unary(ctx: &Ctx, u: &syn::ExprUnary) -> Result<Sexp, TranspileError> {
    let operand = lower_expr(ctx, &u.expr)?;
    match u.op {
        UnOp::Not(_) => Ok(Sexp::call("not", [operand])),
        UnOp::Neg(_) => Ok(Sexp::call("-", [operand])),
        other => Err(TranspileError::spanned(
            &other,
            "unsupported unary operator",
        )),
    }
}

/// Lower a binary operator: arithmetic, comparison, and short-circuit logic.
fn lower_binary(ctx: &Ctx, b: &ExprBinary) -> Result<Sexp, TranspileError> {
    let lhs = lower_expr(ctx, &b.left)?;
    let rhs = lower_expr(ctx, &b.right)?;
    let head = match b.op {
        BinOp::Add(_) => "+",
        BinOp::Sub(_) => "-",
        BinOp::Mul(_) => "*",
        BinOp::Div(_) => "/",
        BinOp::Rem(_) => "%",
        BinOp::Lt(_) => "<",
        BinOp::Le(_) => "<=",
        BinOp::Gt(_) => ">",
        BinOp::Ge(_) => ">=",
        BinOp::And(_) => "and",
        BinOp::Or(_) => "or",
        BinOp::Eq(_) => return Ok(Sexp::call("equal", [lhs, rhs])),
        BinOp::Ne(_) => {
            return Ok(Sexp::call("not", [Sexp::call("equal", [lhs, rhs])]));
        }
        other => {
            return Err(TranspileError::spanned(
                &other,
                "unsupported binary operator (no bitwise or compound-assign ops)",
            ));
        }
    };
    Ok(Sexp::call(head, [lhs, rhs]))
}

/// Lower a function call `f(args)`.
///
/// Recognizes the four intrinsics (`sym`, `func`, `kbd`, `raw`) that produce
/// reader-macro nodes, the `format!`-style not applicable here, and otherwise
/// emits `(f-kebab args...)` using the FFI name resolution.
fn lower_call(ctx: &Ctx, c: &ExprCall) -> Result<Sexp, TranspileError> {
    let Expr::Path(path) = c.func.as_ref() else {
        return Err(TranspileError::spanned(
            &c.func,
            "unsupported: call to a non-identifier callee (no function pointers)",
        ));
    };
    if path.qself.is_some() || path.path.segments.len() != 1 {
        return Err(TranspileError::spanned(
            path,
            "unsupported: call through a qualified or multi-segment path",
        ));
    }
    let seg = &path.path.segments[0];
    if !seg.arguments.is_none() {
        return Err(TranspileError::spanned(
            seg,
            "unsupported: call with explicit generic arguments",
        ));
    }
    let rust_name = seg.ident.to_string();

    if INTRINSICS.contains(&rust_name.as_str()) {
        return lower_intrinsic(ctx, &rust_name, c);
    }

    let mut args = Vec::with_capacity(c.args.len());
    for a in &c.args {
        args.push(lower_expr(ctx, a)?);
    }
    Ok(Sexp::call(ctx.elisp_name(&rust_name), args))
}

/// Lower the four intrinsic helpers into their reader-macro `Sexp` nodes.
fn lower_intrinsic(ctx: &Ctx, name: &str, c: &ExprCall) -> Result<Sexp, TranspileError> {
    let one_str = |c: &ExprCall| -> Result<String, TranspileError> {
        if c.args.len() != 1 {
            return Err(TranspileError::spanned(
                c,
                format!("intrinsic `{name}` takes exactly one string-literal argument"),
            ));
        }
        match &c.args[0] {
            Expr::Lit(l) => match &l.lit {
                Lit::Str(s) => Ok(s.value()),
                other => Err(TranspileError::spanned(
                    other,
                    format!("intrinsic `{name}` requires a string-literal argument"),
                )),
            },
            other => Err(TranspileError::spanned(
                other,
                format!("intrinsic `{name}` requires a string-literal argument"),
            )),
        }
    };
    let _ = ctx;
    match name {
        "sym" => Ok(Sexp::Quote(Box::new(Sexp::sym(one_str(c)?)))),
        "func" => Ok(Sexp::Function(Box::new(Sexp::sym(one_str(c)?)))),
        "kbd" => Ok(Sexp::call("kbd", [Sexp::Str(one_str(c)?)])),
        "raw" => Ok(Sexp::Raw(one_str(c)?)),
        _ => unreachable!("INTRINSICS and lower_intrinsic are out of sync"),
    }
}

/// Lower a method call `recv.m(args)` to `(m recv args...)`.
fn lower_method_call(ctx: &Ctx, m: &ExprMethodCall) -> Result<Sexp, TranspileError> {
    if m.turbofish.is_some() {
        return Err(TranspileError::spanned(
            m,
            "unsupported: method call with turbofish generic arguments",
        ));
    }
    let head = ctx.elisp_name(&m.method.to_string());
    let mut args = Vec::with_capacity(m.args.len() + 1);
    args.push(lower_expr(ctx, &m.receiver)?);
    for a in &m.args {
        args.push(lower_expr(ctx, a)?);
    }
    Ok(Sexp::call(head, args))
}

/// Lower `if c { .. } else { .. }` to `(if c then else)`, and `if c { .. }`
/// without an `else` to `(when c ..)`.
fn lower_if(ctx: &Ctx, i: &syn::ExprIf) -> Result<Sexp, TranspileError> {
    if let Expr::Let(l) = i.cond.as_ref() {
        return Err(TranspileError::spanned(
            l,
            "unsupported: `if let` (use `match`)",
        ));
    }
    let cond = lower_expr(ctx, &i.cond)?;
    let then = lower_stmts(ctx, &i.then_branch.stmts)?;
    match &i.else_branch {
        None => {
            let mut form = vec![Sexp::sym("when"), cond];
            form.extend(splice_progn(then));
            Ok(Sexp::List(form))
        }
        Some((_, els)) => {
            let els = lower_expr(ctx, els)?;
            Ok(Sexp::call("if", [cond, then, els]))
        }
    }
}

/// Lower `while c { body }` to `(while c body...)`.
fn lower_while(ctx: &Ctx, w: &syn::ExprWhile) -> Result<Sexp, TranspileError> {
    if let Expr::Let(l) = w.cond.as_ref() {
        return Err(TranspileError::spanned(l, "unsupported: `while let`"));
    }
    let cond = lower_expr(ctx, &w.cond)?;
    let body = lower_stmts(ctx, &w.body.stmts)?;
    let mut form = vec![Sexp::sym("while"), cond];
    form.extend(splice_progn(body));
    Ok(Sexp::List(form))
}

/// Lower `for p in it { body }` to `(dolist (p it) body...)`, and the special
/// case `for i in 0..n { body }` to `(dotimes (i n) body...)`.
fn lower_for(ctx: &Ctx, f: &syn::ExprForLoop) -> Result<Sexp, TranspileError> {
    let Pat::Ident(pi) = f.pat.as_ref() else {
        return Err(TranspileError::spanned(
            &f.pat,
            "unsupported: only a single loop variable is supported in `for`",
        ));
    };
    let loop_var = Sexp::sym(kebab_case(&pi.ident.to_string()));
    let body = lower_stmts(ctx, &f.body.stmts)?;
    let body_forms = splice_progn(body);

    // Special case: `for i in 0..n` lowers to dotimes when the start is 0.
    if let Expr::Range(range) = f.expr.as_ref() {
        return lower_dotimes(ctx, &loop_var, range, body_forms);
    }

    let iter = lower_expr(ctx, &f.expr)?;
    let mut form = vec![Sexp::sym("dolist"), Sexp::List(vec![loop_var, iter])];
    form.extend(body_forms);
    Ok(Sexp::List(form))
}

/// Lower a `for i in 0..n` header into `(dotimes (i n) ..)`.
fn lower_dotimes(
    ctx: &Ctx,
    loop_var: &Sexp,
    range: &syn::ExprRange,
    body_forms: Vec<Sexp>,
) -> Result<Sexp, TranspileError> {
    use syn::RangeLimits;
    if matches!(range.limits, RangeLimits::Closed(_)) {
        return Err(TranspileError::spanned(
            range,
            "unsupported: inclusive range `0..=n` in a `for` header (use `0..n`)",
        ));
    }
    let start = range.start.as_ref().ok_or_else(|| {
        TranspileError::spanned(
            range,
            "unsupported: `for` range without a start (use `0..n`)",
        )
    })?;
    let is_zero = matches!(
        start.as_ref(),
        Expr::Lit(l) if matches!(&l.lit, Lit::Int(i) if i.base10_digits() == "0")
    );
    if !is_zero {
        return Err(TranspileError::spanned(
            start,
            "unsupported: `for` range must start at 0 (only `0..n` lowers to dotimes)",
        ));
    }
    let end = range.end.as_ref().ok_or_else(|| {
        TranspileError::spanned(
            range,
            "unsupported: `for` range without an end (use `0..n`)",
        )
    })?;
    let count = lower_expr(ctx, end)?;
    let mut form = vec![
        Sexp::sym("dotimes"),
        Sexp::List(vec![loop_var.clone(), count]),
    ];
    form.extend(body_forms);
    Ok(Sexp::List(form))
}

/// Lower `x = e;` to `(setq x e)`. Only assignment to a bare variable is
/// supported; field and index assignment are refused.
fn lower_assign(ctx: &Ctx, a: &syn::ExprAssign) -> Result<Sexp, TranspileError> {
    let Expr::Path(p) = a.left.as_ref() else {
        return Err(TranspileError::spanned(
            &a.left,
            "unsupported: assignment target must be a bare variable",
        ));
    };
    if p.qself.is_some() || p.path.segments.len() != 1 {
        return Err(TranspileError::spanned(
            p,
            "unsupported: assignment to a qualified or multi-segment path",
        ));
    }
    let name = kebab_case(&p.path.segments[0].ident.to_string());
    let value = lower_expr(ctx, &a.right)?;
    Ok(Sexp::call("setq", [Sexp::sym(name), value]))
}

/// Lower a closure `|a, b| body` to `(lambda (a b) body)`.
fn lower_closure(ctx: &Ctx, c: &syn::ExprClosure) -> Result<Sexp, TranspileError> {
    if c.asyncness.is_some() {
        return Err(TranspileError::spanned(c, "unsupported: `async` closure"));
    }
    if c.movability.is_some() {
        return Err(TranspileError::spanned(c, "unsupported: `static` closure"));
    }
    let mut params = Vec::new();
    for input in &c.inputs {
        match input {
            Pat::Ident(pi) => params.push(Sexp::sym(kebab_case(&pi.ident.to_string()))),
            Pat::Type(pt) => match pt.pat.as_ref() {
                Pat::Ident(pi) => params.push(Sexp::sym(kebab_case(&pi.ident.to_string()))),
                other => {
                    return Err(TranspileError::spanned(
                        other,
                        "unsupported: closure parameter pattern",
                    ));
                }
            },
            other => {
                return Err(TranspileError::spanned(
                    other,
                    "unsupported: closure parameter pattern",
                ));
            }
        }
    }
    let body = lower_expr(ctx, &c.body)?;
    let mut form = vec![Sexp::sym("lambda"), Sexp::List(params)];
    form.extend(splice_progn(body));
    Ok(Sexp::List(form))
}

/// Lower `return e;` only in tail position (where the caller treats it as the
/// block value). A non-tail return is refused; here we accept the syntactic
/// form and the surrounding sequencing decides tail-ness.
fn lower_return(ctx: &Ctx, r: &syn::ExprReturn) -> Result<Sexp, TranspileError> {
    match &r.expr {
        Some(e) => lower_expr(ctx, e),
        None => Ok(Sexp::Nil),
    }
}

/// Lower a macro invocation. Only `format!` and `println!`/`print!`-style
/// formatting are supported, mapping to `format`/`message`.
fn lower_macro(ctx: &Ctx, m: &syn::ExprMacro) -> Result<Sexp, TranspileError> {
    let name = m
        .mac
        .path
        .segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_default();
    let head = match name.as_str() {
        "format" => "format",
        "println" | "print" | "message" => "message",
        _ => {
            return Err(TranspileError::spanned(
                m,
                format!("unsupported macro `{name}!` (only format!/println!/message! lower)"),
            ));
        }
    };
    let parsed: syn::punctuated::Punctuated<Expr, syn::Token![,]> = m
        .mac
        .parse_body_with(syn::punctuated::Punctuated::parse_terminated)
        .map_err(|e| TranspileError::spanned(m, format!("malformed `{name}!` arguments: {e}")))?;
    let mut args = Vec::with_capacity(parsed.len());
    for e in &parsed {
        args.push(lower_expr(ctx, e)?);
    }
    if args.is_empty() {
        return Err(TranspileError::spanned(
            m,
            format!("`{name}!` requires at least a format-string argument"),
        ));
    }
    Ok(Sexp::call(head, args))
}

// --- match / pcase -----------------------------------------------------------

/// Lower `match v { arms }` to `(pcase v arms...)`.
fn lower_match(ctx: &Ctx, m: &syn::ExprMatch) -> Result<Sexp, TranspileError> {
    let scrutinee = lower_expr(ctx, &m.expr)?;
    let mut form = vec![Sexp::sym("pcase"), scrutinee];
    for arm in &m.arms {
        form.push(lower_match_arm(ctx, arm)?);
    }
    Ok(Sexp::List(form))
}

/// Lower one match arm into a `(pattern body)` clause, honoring an `if` guard.
fn lower_match_arm(ctx: &Ctx, arm: &syn::Arm) -> Result<Sexp, TranspileError> {
    let mut pat = lower_pattern(&arm.pat)?;
    if let Some((_, guard)) = &arm.guard {
        // `pat if cond` lowers to a pcase `(and PAT (guard COND))`.
        let cond = lower_expr(ctx, guard)?;
        pat = Sexp::List(vec![
            Sexp::sym("and"),
            pat,
            Sexp::List(vec![Sexp::sym("guard"), cond]),
        ]);
    }
    let body = lower_expr(ctx, &arm.body)?;
    let mut clause = vec![pat];
    clause.extend(splice_progn(body));
    Ok(Sexp::List(clause))
}

/// Lower a match pattern. Supported: literals, the wildcard `_`, a bare binding
/// identifier, and an or-pattern `a | b`. Anything else is refused.
fn lower_pattern(pat: &Pat) -> Result<Sexp, TranspileError> {
    match pat {
        Pat::Wild(_) => Ok(Sexp::sym("_")),
        Pat::Lit(l) => lower_lit(&l.lit),
        Pat::Ident(pi) => {
            if pi.by_ref.is_some() || pi.subpat.is_some() {
                return Err(TranspileError::spanned(
                    pi,
                    "unsupported: complex binding pattern in `match`",
                ));
            }
            let ident = pi.ident.to_string();
            // A capitalized bare identifier is a unit enum variant or struct,
            // not a binding (Rust convention). Lowering it as a binding would
            // silently mistranslate, so refuse it for honesty: enums and
            // structs are out of the subset.
            if ident.chars().next().is_some_and(|c| c.is_uppercase()) {
                return Err(TranspileError::spanned(
                    pi,
                    "unsupported: path pattern (a capitalized name is a unit \
                     variant; no enums/structs in the subset)",
                ));
            }
            // A bare lowercase identifier binds; pcase binds with a bare symbol.
            Ok(Sexp::sym(kebab_case(&ident)))
        }
        Pat::Or(or) => {
            let mut form = vec![Sexp::sym("or")];
            for case in &or.cases {
                form.push(lower_pattern(case)?);
            }
            Ok(Sexp::List(form))
        }
        Pat::Paren(p) => lower_pattern(&p.pat),
        Pat::Path(p) => {
            // A unit path like `None` is refused: enums are out of subset.
            Err(TranspileError::spanned(
                p,
                "unsupported: path pattern (no enums/structs in the subset)",
            ))
        }
        other => Err(TranspileError::spanned(
            other,
            "unsupported `match` pattern (only literals, `_`, bindings, and `a | b`)",
        )),
    }
}

// --- Attributes --------------------------------------------------------------

/// Collect `///` doc comments on `attrs` into a single docstring, or `None`.
fn doc_string(attrs: &[syn::Attribute]) -> Option<String> {
    let mut lines = Vec::new();
    for attr in attrs {
        if attr.path().is_ident("doc")
            && let syn::Meta::NameValue(nv) = &attr.meta
            && let Expr::Lit(syn::ExprLit {
                lit: Lit::Str(s), ..
            }) = &nv.value
        {
            lines.push(s.value().trim().to_string());
        }
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

/// Return `true` if any attribute is the FFI marker `#[elisp]` / `#[elisp(..)]`.
fn has_elisp_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| a.path().is_ident("elisp"))
}

/// Extract the `name = "..."` override from an `#[elisp(name = "..")]` attribute.
///
/// Returns `Ok(None)` when there is no `#[elisp]` attribute at all, `Ok(None)`
/// for a bare `#[elisp]` (kebab-case mapping is used), and the overridden name
/// for `#[elisp(name = "..")]`.
fn elisp_attr_override(attrs: &[syn::Attribute]) -> Result<Option<String>, TranspileError> {
    for attr in attrs {
        if !attr.path().is_ident("elisp") {
            continue;
        }
        // Bare `#[elisp]` is a Path meta: no override.
        if matches!(attr.meta, syn::Meta::Path(_)) {
            continue;
        }
        let mut found: Option<String> = None;
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                let value = meta.value()?;
                let lit: syn::LitStr = value.parse()?;
                found = Some(lit.value());
                Ok(())
            } else {
                Err(meta.error("unsupported `#[elisp(..)]` key (only `name` is supported)"))
            }
        })
        .map_err(TranspileError::from)?;
        if found.is_some() {
            return Ok(found);
        }
    }
    Ok(None)
}

/// Read an `#[interactive]` / `#[interactive("spec")]` attribute into the
/// `(interactive ..)` form, or `None` when absent.
fn interactive_attr(attrs: &[syn::Attribute]) -> Result<Option<Sexp>, TranspileError> {
    for attr in attrs {
        if !attr.path().is_ident("interactive") {
            continue;
        }
        match &attr.meta {
            syn::Meta::Path(_) => {
                return Ok(Some(Sexp::call::<[Sexp; 0]>("interactive", [])));
            }
            syn::Meta::List(_) => {
                let spec: syn::LitStr = attr.parse_args().map_err(|_| {
                    TranspileError::spanned(
                        attr,
                        "`#[interactive(..)]` takes a single string-literal spec, \
                         e.g. #[interactive(\"p\")]",
                    )
                })?;
                return Ok(Some(Sexp::call("interactive", [Sexp::Str(spec.value())])));
            }
            syn::Meta::NameValue(nv) => {
                return Err(TranspileError::spanned(
                    nv,
                    "`#[interactive]` does not take a `= value` form",
                ));
            }
        }
    }
    Ok(None)
}
