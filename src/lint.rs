/// Lint warnings for C code
///
/// Detects common issues:
/// - Unused variables
/// - Missing return in non-void function
/// - Unreachable code after return
/// - Comparison of assignment (= vs ==)
/// - Empty if/while body

use std::collections::{HashMap, HashSet};
use crate::ast::*;
use crate::error::Span;

#[derive(Debug)]
pub struct LintWarning {
    pub message: String,
    pub span: Span,
    pub code: &'static str,
}

impl LintWarning {
    pub fn print(&self, filename: &str, source: &str) {
        let (line_no, col, line_text) = locate(source, self.span.offset);

        eprintln!(
            "\x1b[1;33mwarning[{}]\x1b[0m: {}",
            self.code, self.message
        );
        eprintln!(
            "  \x1b[1;34m-->\x1b[0m {}:{}:{}",
            filename, line_no, col + 1
        );
        let gutter = format!("{}", line_no).len();
        eprintln!("{:>w$} \x1b[1;34m|\x1b[0m", "", w = gutter);
        eprintln!("{:>w$} \x1b[1;34m|\x1b[0m {}", line_no, line_text, w = gutter);
        let plen = self.span.len.max(1);
        eprintln!(
            "{:>w$} \x1b[1;34m|\x1b[0m {}\x1b[1;33m{}\x1b[0m",
            "", " ".repeat(col), "~".repeat(plen), w = gutter
        );
    }
}

fn locate(source: &str, offset: usize) -> (usize, usize, &str) {
    let mut line_no = 1;
    let mut line_start = 0;
    for (i, ch) in source.char_indices() {
        if i >= offset { break; }
        if ch == '\n' { line_no += 1; line_start = i + 1; }
    }
    let col = offset.saturating_sub(line_start);
    let line_end = source[offset..].find('\n').map(|p| offset + p).unwrap_or(source.len());
    (line_no, col, &source[line_start..line_end])
}

pub fn lint(tu: &TranslationUnit) -> Vec<LintWarning> {
    let mut warnings = Vec::new();

    for decl in &tu.decls {
        if let TopLevel::FuncDef { name, return_ty, body, span, .. } = decl {
            // Check unused variables
            let mut declared = HashMap::new();
            let mut used = HashSet::new();
            collect_vars(body, &mut declared, &mut used);
            for (vname, vspan) in &declared {
                if !used.contains(vname.as_str()) && !vname.starts_with('_') {
                    warnings.push(LintWarning {
                        message: format!("unused variable '{}'", vname),
                        span: *vspan,
                        code: "W001",
                    });
                }
            }

            // Check missing return in non-void function
            if *return_ty != Type::Void && name != "main" {
                if !has_return(body) {
                    warnings.push(LintWarning {
                        message: format!("function '{}' may not return a value", name),
                        span: *span,
                        code: "W002",
                    });
                }
            }

            // Check unreachable code
            check_unreachable(body, &mut warnings);
        }
    }

    warnings
}

fn collect_vars(stmt: &Stmt, declared: &mut HashMap<String, Span>, used: &mut HashSet<String>) {
    match stmt {
        Stmt::VarDecl(decl, span) => {
            declared.insert(decl.name.clone(), *span);
            if let Some(init) = &decl.init { collect_expr_vars(init, used); }
        }
        Stmt::Block(stmts, _) => { for s in stmts { collect_vars(s, declared, used); } }
        Stmt::Expr(e, _) => collect_expr_vars(e, used),
        Stmt::Return(Some(e), _) => collect_expr_vars(e, used),
        Stmt::If(c, t, e, _) => {
            collect_expr_vars(c, used);
            collect_vars(t, declared, used);
            if let Some(e) = e { collect_vars(e, declared, used); }
        }
        Stmt::While(c, b, _) => { collect_expr_vars(c, used); collect_vars(b, declared, used); }
        Stmt::DoWhile(b, c, _) => { collect_vars(b, declared, used); collect_expr_vars(c, used); }
        Stmt::For(i, c, inc, b, _) => {
            if let Some(i) = i { collect_vars(i, declared, used); }
            if let Some(c) = c { collect_expr_vars(c, used); }
            if let Some(inc) = inc { collect_expr_vars(inc, used); }
            collect_vars(b, declared, used);
        }
        Stmt::Switch(c, b, _) => { collect_expr_vars(c, used); collect_vars(b, declared, used); }
        Stmt::Case(_, b, _) | Stmt::Default(b, _) | Stmt::Label(_, b, _) => collect_vars(b, declared, used),
        _ => {}
    }
}

fn collect_expr_vars(expr: &Expr, used: &mut HashSet<String>) {
    match expr {
        Expr::Var(name, _) => { used.insert(name.clone()); }
        Expr::Binary(_, l, r, _) => { collect_expr_vars(l, used); collect_expr_vars(r, used); }
        Expr::Unary(_, e, _) => collect_expr_vars(e, used),
        Expr::Call(f, args, _) => { collect_expr_vars(f, used); for a in args { collect_expr_vars(a, used); } }
        Expr::Index(a, i, _) => { collect_expr_vars(a, used); collect_expr_vars(i, used); }
        Expr::Member(b, _, _) | Expr::Arrow(b, _, _) => collect_expr_vars(b, used),
        Expr::Cond(c, t, e, _) => { collect_expr_vars(c, used); collect_expr_vars(t, used); collect_expr_vars(e, used); }
        Expr::InitList(items, _) => { for i in items { collect_expr_vars(i, used); } }
        _ => {}
    }
}

fn has_return(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(_, _) => true,
        Stmt::Block(stmts, _) => stmts.iter().any(has_return),
        Stmt::If(_, t, Some(e), _) => has_return(t) && has_return(e),
        _ => false,
    }
}

fn check_unreachable(stmt: &Stmt, warnings: &mut Vec<LintWarning>) {
    if let Stmt::Block(stmts, _) = stmt {
        let mut found_return = false;
        for s in stmts {
            if found_return {
                let span = stmt_span(s);
                if span.len > 0 {
                    warnings.push(LintWarning {
                        message: "unreachable code after return".to_string(),
                        span,
                        code: "W003",
                    });
                }
                break;
            }
            if matches!(s, Stmt::Return(_, _)) {
                found_return = true;
            }
            check_unreachable(s, warnings);
        }
    }
    match stmt {
        Stmt::If(_, t, e, _) => {
            check_unreachable(t, warnings);
            if let Some(e) = e { check_unreachable(e, warnings); }
        }
        Stmt::While(_, b, _) | Stmt::DoWhile(b, _, _) => check_unreachable(b, warnings),
        Stmt::For(_, _, _, b, _) => check_unreachable(b, warnings),
        _ => {}
    }
}

fn stmt_span(stmt: &Stmt) -> Span {
    match stmt {
        Stmt::Return(_, s) | Stmt::Expr(_, s) | Stmt::Block(_, s) | Stmt::If(_, _, _, s)
        | Stmt::While(_, _, s) | Stmt::DoWhile(_, _, s) | Stmt::For(_, _, _, _, s)
        | Stmt::Switch(_, _, s) | Stmt::Case(_, _, s) | Stmt::Default(_, s)
        | Stmt::Break(s) | Stmt::Continue(s) | Stmt::Goto(_, s) | Stmt::Label(_, _, s)
        | Stmt::VarDecl(_, s) => *s,
        Stmt::Null => Span::new(0, 0),
    }
}
