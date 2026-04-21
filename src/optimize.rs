use crate::ast::*;

/// Optimize a translation unit
pub fn optimize(tu: &mut TranslationUnit) {
    for decl in &mut tu.decls {
        match decl {
            TopLevel::FuncDef { body, .. } => {
                *body = optimize_stmt(body.clone());
            }
            TopLevel::GlobalVar(decl, _) => {
                if let Some(init) = &mut decl.init {
                    *init = fold_expr(init.clone());
                }
            }
            _ => {}
        }
    }
}

fn optimize_stmt(stmt: Stmt) -> Stmt {
    match stmt {
        Stmt::Return(Some(expr), span) => {
            Stmt::Return(Some(fold_expr(expr)), span)
        }
        Stmt::Expr(expr, span) => {
            Stmt::Expr(fold_expr(expr), span)
        }
        Stmt::Block(stmts, span) => {
            let mut optimized: Vec<Stmt> = Vec::new();
            let mut after_return = false;
            for s in stmts {
                let s = optimize_stmt(s);
                // Dead code elimination: skip statements after return
                // BUT keep labels (they might be goto targets)
                if after_return {
                    if matches!(&s, Stmt::Label(..)) {
                        after_return = false; // label reachable via goto
                    } else {
                        continue; // dead code
                    }
                }
                if matches!(&s, Stmt::Return(..)) {
                    after_return = true;
                }
                optimized.push(s);
            }
            Stmt::Block(optimized, span)
        }
        Stmt::VarDecl(mut decl, span) => {
            if let Some(init) = decl.init {
                decl.init = Some(fold_expr(init));
            }
            Stmt::VarDecl(decl, span)
        }
        Stmt::If(cond, then, els, span) => {
            let cond = fold_expr(cond);
            // If condition is a constant, eliminate the branch
            if let Some(val) = const_value(&cond) {
                if val != 0 {
                    return optimize_stmt(*then);
                } else if let Some(els) = els {
                    return optimize_stmt(*els);
                } else {
                    return Stmt::Null;
                }
            }
            Stmt::If(
                cond,
                Box::new(optimize_stmt(*then)),
                els.map(|e| Box::new(optimize_stmt(*e))),
                span,
            )
        }
        Stmt::While(cond, body, span) => {
            let cond = fold_expr(cond);
            // while(0) → nothing
            if let Some(0) = const_value(&cond) {
                return Stmt::Null;
            }
            Stmt::While(cond, Box::new(optimize_stmt(*body)), span)
        }
        Stmt::DoWhile(body, cond, span) => {
            let cond = fold_expr(cond);
            Stmt::DoWhile(Box::new(optimize_stmt(*body)), cond, span)
        }
        Stmt::For(init, cond, inc, body, span) => {
            let init = init.map(|s| Box::new(optimize_stmt(*s)));
            let cond = cond.map(fold_expr);
            let inc = inc.map(fold_expr);
            // for(;0;) → nothing (but keep init for side effects)
            if let Some(c) = &cond {
                if let Some(0) = const_value(c) {
                    if let Some(init) = init {
                        return *init;
                    }
                    return Stmt::Null;
                }
            }
            Stmt::For(init, cond, inc, Box::new(optimize_stmt(*body)), span)
        }
        Stmt::Switch(cond, body, span) => {
            Stmt::Switch(fold_expr(cond), Box::new(optimize_stmt(*body)), span)
        }
        Stmt::Case(val, body, span) => {
            Stmt::Case(val, Box::new(optimize_stmt(*body)), span)
        }
        Stmt::Default(body, span) => {
            Stmt::Default(Box::new(optimize_stmt(*body)), span)
        }
        Stmt::Label(name, body, span) => {
            Stmt::Label(name, Box::new(optimize_stmt(*body)), span)
        }
        other => other,
    }
}

/// Constant folding: evaluate constant expressions at compile time
fn fold_expr(expr: Expr) -> Expr {
    match expr {
        Expr::Binary(op, lhs, rhs, span) => {
            let lhs = fold_expr(*lhs);
            let rhs = fold_expr(*rhs);

            // If both sides are integer constants, compute at compile time
            if let (Some(l), Some(r)) = (const_value(&lhs), const_value(&rhs)) {
                let result = match op {
                    BinOp::Add => Some(l.wrapping_add(r)),
                    BinOp::Sub => Some(l.wrapping_sub(r)),
                    BinOp::Mul => Some(l.wrapping_mul(r)),
                    BinOp::Div if r != 0 => Some(l / r),
                    BinOp::Mod if r != 0 => Some(l % r),
                    BinOp::BitAnd => Some(l & r),
                    BinOp::BitOr => Some(l | r),
                    BinOp::BitXor => Some(l ^ r),
                    BinOp::Shl => Some(l << (r & 63)),
                    BinOp::Shr => Some(l >> (r & 63)),
                    BinOp::Eq => Some(if l == r { 1 } else { 0 }),
                    BinOp::Ne => Some(if l != r { 1 } else { 0 }),
                    BinOp::Lt => Some(if l < r { 1 } else { 0 }),
                    BinOp::Le => Some(if l <= r { 1 } else { 0 }),
                    BinOp::Gt => Some(if l > r { 1 } else { 0 }),
                    BinOp::Ge => Some(if l >= r { 1 } else { 0 }),
                    BinOp::LogAnd => Some(if l != 0 && r != 0 { 1 } else { 0 }),
                    BinOp::LogOr => Some(if l != 0 || r != 0 { 1 } else { 0 }),
                    _ => None,
                };
                if let Some(val) = result {
                    return Expr::IntLit(val, span);
                }
            }

            // Strength reduction: x * 2 → x << 1, x * 4 → x << 2, etc.
            if op == BinOp::Mul {
                if let Some(r) = const_value(&rhs) {
                    if r > 0 && (r as u64).is_power_of_two() {
                        let shift = r.trailing_zeros() as i64;
                        return Expr::Binary(
                            BinOp::Shl,
                            Box::new(lhs),
                            Box::new(Expr::IntLit(shift, span)),
                            span,
                        );
                    }
                }
                if let Some(l) = const_value(&lhs) {
                    if l > 0 && (l as u64).is_power_of_two() {
                        let shift = l.trailing_zeros() as i64;
                        return Expr::Binary(
                            BinOp::Shl,
                            Box::new(rhs),
                            Box::new(Expr::IntLit(shift, span)),
                            span,
                        );
                    }
                }
            }

            // Strength reduction: x / 2 → x >> 1 (for positive constants)
            if op == BinOp::Div {
                if let Some(r) = const_value(&rhs) {
                    if r > 0 && (r as u64).is_power_of_two() {
                        let shift = r.trailing_zeros() as i64;
                        return Expr::Binary(
                            BinOp::Shr,
                            Box::new(lhs),
                            Box::new(Expr::IntLit(shift, span)),
                            span,
                        );
                    }
                }
            }

            // Identity: x + 0, x - 0, x * 1, x / 1
            if let Some(r) = const_value(&rhs) {
                match (op, r) {
                    (BinOp::Add, 0) | (BinOp::Sub, 0) | (BinOp::BitOr, 0) | (BinOp::BitXor, 0) | (BinOp::Shl, 0) | (BinOp::Shr, 0) => return lhs,
                    (BinOp::Mul, 1) | (BinOp::Div, 1) => return lhs,
                    (BinOp::Mul, 0) | (BinOp::BitAnd, 0) => return Expr::IntLit(0, span),
                    _ => {}
                }
            }
            if let Some(l) = const_value(&lhs) {
                match (op, l) {
                    (BinOp::Add, 0) | (BinOp::BitOr, 0) | (BinOp::BitXor, 0) => return rhs,
                    (BinOp::Mul, 1) => return rhs,
                    (BinOp::Mul, 0) => return Expr::IntLit(0, span),
                    _ => {}
                }
            }

            Expr::Binary(op, Box::new(lhs), Box::new(rhs), span)
        }
        Expr::Unary(op, operand, span) => {
            let operand = fold_expr(*operand);
            if let Some(val) = const_value(&operand) {
                let result = match op {
                    UnaryOp::Neg => Some(-val),
                    UnaryOp::Not => Some(if val == 0 { 1 } else { 0 }),
                    UnaryOp::BitNot => Some(!val),
                    _ => None,
                };
                if let Some(v) = result {
                    return Expr::IntLit(v, span);
                }
            }
            Expr::Unary(op, Box::new(operand), span)
        }
        Expr::Cond(cond, then, els, span) => {
            let cond = fold_expr(*cond);
            if let Some(val) = const_value(&cond) {
                return if val != 0 { fold_expr(*then) } else { fold_expr(*els) };
            }
            Expr::Cond(
                Box::new(cond),
                Box::new(fold_expr(*then)),
                Box::new(fold_expr(*els)),
                span,
            )
        }
        Expr::Call(func, args, span) => {
            let args = args.into_iter().map(fold_expr).collect();
            Expr::Call(func, args, span)
        }
        other => other,
    }
}

fn const_value(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::IntLit(val, _) => Some(*val),
        Expr::CharLit(val, _) => Some(*val),
        _ => None,
    }
}
