/// AST → IR lowering
///
/// Converts the high-level AST into linear IR with virtual registers.

use std::collections::HashMap;
use crate::ast::*;
use crate::ir::*;

pub struct Lowering {
    module: IrModule,
    /// Current function being lowered
    func: Option<IrFunction>,
    /// Current basic block instructions (before terminator)
    current_insts: Vec<Inst>,
    current_label: Label,
    /// Variable name → (alloca vreg, ir type)
    locals: HashMap<String, (VReg, IrType)>,
    local_ast_types: HashMap<String, Type>,
    /// break/continue targets
    break_labels: Vec<Label>,
    continue_labels: Vec<Label>,
    /// Data section items (string literals)
    next_data_label: Label,
}

impl Lowering {
    pub fn new() -> Self {
        Self {
            module: IrModule::new(),
            func: None,
            current_insts: Vec::new(),
            current_label: 0,
            locals: HashMap::new(),
            local_ast_types: HashMap::new(),
            break_labels: Vec::new(),
            continue_labels: Vec::new(),
            next_data_label: 10000, // high numbers to avoid collision with block labels
        }
    }

    fn f(&mut self) -> &mut IrFunction {
        self.func.as_mut().unwrap()
    }

    fn new_vreg(&mut self) -> VReg {
        self.f().new_vreg()
    }

    fn new_label(&mut self) -> Label {
        self.f().new_label()
    }

    fn emit(&mut self, inst: Inst) {
        self.current_insts.push(inst);
    }

    fn finish_block(&mut self, term: Terminator) {
        let label = self.current_label;
        let insts = std::mem::take(&mut self.current_insts);
        self.f().blocks.push(BasicBlock { label, insts, term });
    }

    fn start_block(&mut self, label: Label) {
        self.current_label = label;
    }

    fn finish_block_and_start(&mut self, term: Terminator, next: Label) {
        self.finish_block(term);
        self.start_block(next);
    }

    fn data_label(&mut self) -> Label {
        self.next_data_label += 1;
        self.next_data_label
    }

    fn type_of_ast(&self, ty: &Type) -> IrType {
        IrType::from_ast_type(ty)
    }

    pub fn lower(mut self, tu: &TranslationUnit) -> IrModule {
        for decl in &tu.decls {
            match decl {
                TopLevel::FuncDef { name, return_ty, params, body, .. } => {
                    self.lower_func(name, return_ty, params, body);
                }
                TopLevel::GlobalVar(decl, _) => {
                    if decl.name.is_empty() { continue; }
                    let size = decl.ty.size().max(8);
                    let init = match &decl.init {
                        Some(Expr::IntLit(val, _)) => {
                            let mut bytes = vec![0u8; size];
                            let b = val.to_le_bytes();
                            for i in 0..size.min(8) { bytes[i] = b[i]; }
                            Some(bytes)
                        }
                        _ => None,
                    };
                    self.module.globals.push(IrGlobal { name: decl.name.clone(), size, init });
                }
                TopLevel::FuncDecl { .. } => {}
            }
        }
        self.module
    }

    fn lower_func(&mut self, name: &str, return_ty: &Type, params: &[(Type, String)], body: &Stmt) {
        let ret_ty = self.type_of_ast(return_ty);
        let mut func = IrFunction::new(name, ret_ty);

        // Create param vregs
        let mut param_info = Vec::new();
        for (ty, pname) in params {
            let ir_ty = IrType::from_ast_type(ty);
            let vreg = func.new_vreg();
            func.params.push((vreg, ir_ty));
            param_info.push((pname.clone(), vreg, ir_ty, ty.clone()));
        }

        let entry_label = func.new_label();
        self.func = Some(func);
        self.locals.clear();
        self.local_ast_types.clear();
        self.current_insts.clear();
        self.current_label = entry_label;

        // Allocate params on stack and store
        for (pname, param_vreg, ir_ty, ast_ty) in &param_info {
            let size = ast_ty.size().max(8);
            let alloca = self.new_vreg();
            self.emit(Inst::Alloca(alloca, size));
            self.emit(Inst::Store(*param_vreg, alloca, *ir_ty));
            self.locals.insert(pname.clone(), (alloca, *ir_ty));
            self.local_ast_types.insert(pname.clone(), ast_ty.clone());
        }

        // Pre-allocate all locals
        self.prealloc_locals(body);

        // Lower body
        self.lower_stmt(body);

        // Ensure function ends with a return
        let ret_label = self.new_label();
        self.finish_block(Terminator::Jump(ret_label));
        self.start_block(ret_label);
        if ret_ty == IrType::Void {
            self.finish_block(Terminator::Ret(None));
        } else {
            let zero = self.new_vreg();
            self.emit(Inst::Const(zero, 0, ret_ty));
            self.finish_block(Terminator::Ret(Some(zero)));
        }

        let func = self.func.take().unwrap();
        self.module.functions.push(func);
    }

    fn prealloc_locals(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl(decl, _) => {
                if !self.locals.contains_key(&decl.name) {
                    let ir_ty = self.type_of_ast(&decl.ty);
                    let size = match &decl.ty {
                        Type::Array(_, _) | Type::Struct { .. } => decl.ty.size().max(8),
                        _ => 8,
                    };
                    // Infer size from init list
                    let size = if let Some(Expr::InitList(items, _)) = &decl.init {
                        let elem_size = match &decl.ty {
                            Type::Array(base, _) => base.size(),
                            _ => 8,
                        };
                        (items.len() * elem_size).max(size)
                    } else {
                        size
                    };
                    let alloca = self.new_vreg();
                    self.emit(Inst::Alloca(alloca, size));
                    self.locals.insert(decl.name.clone(), (alloca, ir_ty));
                    // Store correct type for arrays
                    let ty = if let Some(Expr::InitList(items, _)) = &decl.init {
                        if let Type::Array(base, 0) = &decl.ty {
                            Type::Array(base.clone(), items.len())
                        } else {
                            decl.ty.clone()
                        }
                    } else {
                        decl.ty.clone()
                    };
                    self.local_ast_types.insert(decl.name.clone(), ty);
                }
            }
            Stmt::Block(stmts, _) => { for s in stmts { self.prealloc_locals(s); } }
            Stmt::If(_, then, els, _) => {
                self.prealloc_locals(then);
                if let Some(e) = els { self.prealloc_locals(e); }
            }
            Stmt::While(_, b, _) | Stmt::DoWhile(b, _, _) => self.prealloc_locals(b),
            Stmt::For(init, _, _, body, _) => {
                if let Some(i) = init { self.prealloc_locals(i); }
                self.prealloc_locals(body);
            }
            Stmt::Switch(_, b, _) | Stmt::Case(_, b, _) | Stmt::Default(b, _)
            | Stmt::Label(_, b, _) => self.prealloc_locals(b),
            _ => {}
        }
    }

    fn lower_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Null => {}
            Stmt::Block(stmts, _) => {
                for s in stmts { self.lower_stmt(s); }
            }
            Stmt::Expr(expr, _) => {
                self.lower_expr(expr);
            }
            Stmt::Return(expr, _) => {
                let val = expr.as_ref().map(|e| self.lower_expr(e));
                let next = self.new_label();
                self.finish_block(Terminator::Ret(val));
                self.start_block(next); // dead block, but needed structurally
            }
            Stmt::VarDecl(decl, _) => {
                if let Some(init) = &decl.init {
                    let (alloca, _) = self.locals[&decl.name];
                    match init {
                        Expr::InitList(items, _) => {
                            let elem_size = match &decl.ty {
                                Type::Array(base, _) => base.size(),
                                _ => 8,
                            };
                            let elem_ir_ty = match &decl.ty {
                                Type::Array(base, _) => IrType::from_ast_type(base),
                                _ => IrType::I64,
                            };
                            for (i, item) in items.iter().enumerate() {
                                let val = self.lower_expr(item);
                                let offset = (i * elem_size) as i64;
                                let addr = self.new_vreg();
                                self.emit(Inst::AddImm(addr, alloca, offset));
                                self.emit(Inst::Store(val, addr, elem_ir_ty));
                            }
                        }
                        _ => {
                            let val = self.lower_expr(init);
                            let ir_ty = self.type_of_ast(&decl.ty);
                            self.emit(Inst::Store(val, alloca, ir_ty));
                        }
                    }
                }
            }
            Stmt::If(cond, then, els, _) => {
                let cv = self.lower_expr(cond);
                let then_label = self.new_label();
                let else_label = self.new_label();
                let end_label = self.new_label();
                self.finish_block_and_start(Terminator::Branch(cv, then_label, else_label), then_label);
                self.lower_stmt(then);
                self.finish_block_and_start(Terminator::Jump(end_label), else_label);
                if let Some(els) = els { self.lower_stmt(els); }
                self.finish_block_and_start(Terminator::Jump(end_label), end_label);
            }
            Stmt::While(cond, body, _) => {
                let cond_label = self.new_label();
                let body_label = self.new_label();
                let end_label = self.new_label();
                self.break_labels.push(end_label);
                self.continue_labels.push(cond_label);
                self.finish_block_and_start(Terminator::Jump(cond_label), cond_label);
                let cv = self.lower_expr(cond);
                self.finish_block_and_start(Terminator::Branch(cv, body_label, end_label), body_label);
                self.lower_stmt(body);
                self.finish_block_and_start(Terminator::Jump(cond_label), end_label);
                self.break_labels.pop();
                self.continue_labels.pop();
            }
            Stmt::DoWhile(body, cond, _) => {
                let body_label = self.new_label();
                let cond_label = self.new_label();
                let end_label = self.new_label();
                self.break_labels.push(end_label);
                self.continue_labels.push(cond_label);
                self.finish_block_and_start(Terminator::Jump(body_label), body_label);
                self.lower_stmt(body);
                self.finish_block_and_start(Terminator::Jump(cond_label), cond_label);
                let cv = self.lower_expr(cond);
                self.finish_block(Terminator::Branch(cv, body_label, end_label));
                self.start_block(end_label);
                self.break_labels.pop();
                self.continue_labels.pop();
            }
            Stmt::For(init, cond, inc, body, _) => {
                if let Some(init) = init { self.lower_stmt(init); }
                let cond_label = self.new_label();
                let body_label = self.new_label();
                let cont_label = self.new_label();
                let end_label = self.new_label();
                self.break_labels.push(end_label);
                self.continue_labels.push(cont_label);
                self.finish_block_and_start(Terminator::Jump(cond_label), cond_label);
                if let Some(cond) = cond {
                    let cv = self.lower_expr(cond);
                    self.finish_block_and_start(Terminator::Branch(cv, body_label, end_label), body_label);
                } else {
                    self.finish_block_and_start(Terminator::Jump(body_label), body_label);
                }
                self.lower_stmt(body);
                self.finish_block_and_start(Terminator::Jump(cont_label), cont_label);
                if let Some(inc) = inc { self.lower_expr(inc); }
                self.finish_block_and_start(Terminator::Jump(cond_label), end_label);
                self.break_labels.pop();
                self.continue_labels.pop();
            }
            Stmt::Break(_) => {
                if let Some(&l) = self.break_labels.last() {
                    let next = self.new_label();
                    self.finish_block_and_start(Terminator::Jump(l), next);
                }
            }
            Stmt::Continue(_) => {
                if let Some(&l) = self.continue_labels.last() {
                    let next = self.new_label();
                    self.finish_block_and_start(Terminator::Jump(l), next);
                }
            }
            Stmt::Switch(cond, body, _) => {
                let cv = self.lower_expr(cond);
                let end_label = self.new_label();
                self.break_labels.push(end_label);
                // Collect cases
                let mut cases = Vec::new();
                self.collect_switch_cases(body, &mut cases);
                let default_label = self.new_label();
                // Emit cascading branches — pre-assign labels
                for i in 0..cases.len() {
                    cases[i].1 = self.new_label();
                }
                let case_labels: Vec<(i64, Label)> = cases.clone();
                for (val, match_label) in &case_labels {
                    let next_label = self.new_label();
                    let cmp = self.new_vreg();
                    let val_reg = self.new_vreg();
                    self.emit(Inst::Const(val_reg, *val, IrType::I64));
                    self.emit(Inst::Cmp(cmp, CmpOp::Eq, cv, val_reg, IrType::I64));
                    self.finish_block_and_start(Terminator::Branch(cmp, *match_label, next_label), next_label);
                }
                self.finish_block_and_start(Terminator::Jump(default_label), default_label);
                self.lower_switch_body(body, &cases);
                self.finish_block_and_start(Terminator::Jump(end_label), end_label);
                self.break_labels.pop();
            }
            Stmt::Case(_, _, _) | Stmt::Default(_, _) => {
                // Handled inside switch
            }
            Stmt::Goto(name, _) => {
                // Simplified: not fully supported in IR yet
                self.emit(Inst::Comment(format!("goto {}", name)));
            }
            Stmt::Label(name, body, _) => {
                self.emit(Inst::Comment(format!("label {}", name)));
                self.lower_stmt(body);
            }
        }
    }

    fn collect_switch_cases(&self, stmt: &Stmt, cases: &mut Vec<(i64, Label)>) {
        match stmt {
            Stmt::Case(val, body, _) => {
                cases.push((*val, 0)); // label filled later
                self.collect_switch_cases(body, cases);
            }
            Stmt::Block(stmts, _) => {
                for s in stmts { self.collect_switch_cases(s, cases); }
            }
            Stmt::Default(body, _) => self.collect_switch_cases(body, cases),
            _ => {}
        }
    }

    fn lower_switch_body(&mut self, stmt: &Stmt, cases: &[(i64, Label)]) {
        match stmt {
            Stmt::Case(val, body, _) => {
                if let Some((_, label)) = cases.iter().find(|(v, _)| *v == *val) {
                    // Start the case block
                    let next = self.new_label();
                    self.finish_block_and_start(Terminator::Jump(*label), *label);
                }
                self.lower_switch_body(body, cases);
            }
            Stmt::Default(body, _) => {
                self.lower_switch_body(body, cases);
            }
            Stmt::Block(stmts, _) => {
                for s in stmts { self.lower_switch_body(s, cases); }
            }
            other => self.lower_stmt(other),
        }
    }

    /// Lower an expression, return the vreg holding the result
    fn lower_expr(&mut self, expr: &Expr) -> VReg {
        match expr {
            Expr::IntLit(val, _) => {
                let dst = self.new_vreg();
                self.emit(Inst::Const(dst, *val, IrType::I64));
                dst
            }
            Expr::CharLit(val, _) => {
                let dst = self.new_vreg();
                self.emit(Inst::Const(dst, *val, IrType::I8));
                dst
            }
            Expr::FloatLit(val, _) => {
                let dst = self.new_vreg();
                self.emit(Inst::Const(dst, val.to_bits() as i64, IrType::F64));
                dst
            }
            Expr::StrLit(s, _) => {
                let label = self.data_label();
                let mut bytes = s.as_bytes().to_vec();
                bytes.push(0); // null terminator
                self.module.data.push(IrData { label, bytes });
                let dst = self.new_vreg();
                self.emit(Inst::LeaLabel(dst, label));
                dst
            }
            Expr::Var(name, _) => {
                if let Some(&(alloca, ir_ty)) = self.locals.get(name) {
                    let ast_ty = self.local_ast_types.get(name);
                    let is_array = ast_ty.map_or(false, |t| matches!(t, Type::Array(_, _)));
                    if is_array {
                        // Array decays to pointer — return address
                        alloca
                    } else {
                        let dst = self.new_vreg();
                        self.emit(Inst::Load(dst, alloca, ir_ty));
                        dst
                    }
                } else {
                    // Global
                    let addr = self.new_vreg();
                    self.emit(Inst::LeaGlobal(addr, name.clone()));
                    let dst = self.new_vreg();
                    self.emit(Inst::Load(dst, addr, IrType::I64));
                    dst
                }
            }
            Expr::Binary(op, lhs, rhs, _) => {
                self.lower_binary(*op, lhs, rhs)
            }
            Expr::Unary(op, operand, _) => {
                self.lower_unary(*op, operand)
            }
            Expr::Call(func, args, _) => {
                let arg_vregs: Vec<VReg> = args.iter().map(|a| self.lower_expr(a)).collect();
                let dst = self.new_vreg();
                if let Expr::Var(name, _) = func.as_ref() {
                    self.emit(Inst::Call(dst, name.clone(), arg_vregs));
                } else {
                    let fv = self.lower_expr(func);
                    self.emit(Inst::CallIndirect(dst, fv, arg_vregs));
                }
                dst
            }
            Expr::Index(base, index, _) => {
                let base_v = self.lower_expr(base);
                let idx_v = self.lower_expr(index);
                let stride = self.index_stride_ast(base);
                let addr = self.new_vreg();
                self.emit(Inst::GetElementPtr(addr, base_v, idx_v, stride));
                let elem_ty = self.index_elem_type(base);
                let dst = self.new_vreg();
                self.emit(Inst::Load(dst, addr, elem_ty));
                dst
            }
            Expr::Member(base, member, _) => {
                let base_addr = self.lower_addr(base);
                let offset = self.member_offset(base, member);
                let addr = self.new_vreg();
                self.emit(Inst::AddImm(addr, base_addr, offset as i64));
                let mem_ty = self.member_type(base, member);
                let dst = self.new_vreg();
                self.emit(Inst::Load(dst, addr, mem_ty));
                dst
            }
            Expr::Arrow(base, member, _) => {
                let ptr = self.lower_expr(base);
                let offset = self.member_offset_ptr(base, member);
                let addr = self.new_vreg();
                self.emit(Inst::AddImm(addr, ptr, offset as i64));
                let mem_ty = self.member_type_ptr(base, member);
                let dst = self.new_vreg();
                self.emit(Inst::Load(dst, addr, mem_ty));
                dst
            }
            Expr::Cond(cond, then, els, _) => {
                let cv = self.lower_expr(cond);
                let then_label = self.new_label();
                let else_label = self.new_label();
                let end_label = self.new_label();
                let result_alloca = self.new_vreg();
                self.emit(Inst::Alloca(result_alloca, 8));
                self.finish_block_and_start(Terminator::Branch(cv, then_label, else_label), then_label);
                let tv = self.lower_expr(then);
                self.emit(Inst::Store(tv, result_alloca, IrType::I64));
                self.finish_block_and_start(Terminator::Jump(end_label), else_label);
                let ev = self.lower_expr(els);
                self.emit(Inst::Store(ev, result_alloca, IrType::I64));
                self.finish_block_and_start(Terminator::Jump(end_label), end_label);
                let dst = self.new_vreg();
                self.emit(Inst::Load(dst, result_alloca, IrType::I64));
                dst
            }
            Expr::Sizeof(arg, _) => {
                let size = match arg.as_ref() {
                    SizeofArg::Type(ty) => ty.size(),
                    SizeofArg::Expr(_) => 8,
                };
                let dst = self.new_vreg();
                self.emit(Inst::Const(dst, size as i64, IrType::I64));
                dst
            }
            Expr::Cast(_, inner, _) => {
                self.lower_expr(inner)
            }
            Expr::InitList(_, _) => {
                let dst = self.new_vreg();
                self.emit(Inst::Const(dst, 0, IrType::I64));
                dst
            }
            Expr::StmtExpr(stmts, last_expr, _) => {
                for s in stmts { self.lower_stmt(s); }
                self.lower_expr(last_expr)
            }
        }
    }

    fn lower_binary(&mut self, op: BinOp, lhs: &Expr, rhs: &Expr) -> VReg {
        match op {
            BinOp::Assign => {
                let val = self.lower_expr(rhs);
                let addr = self.lower_addr(lhs);
                let ty = self.expr_ir_type(lhs);
                self.emit(Inst::Store(val, addr, ty));
                val
            }
            BinOp::AddAssign | BinOp::SubAssign | BinOp::MulAssign
            | BinOp::DivAssign | BinOp::ModAssign
            | BinOp::BitAndAssign | BinOp::BitOrAssign | BinOp::BitXorAssign
            | BinOp::ShlAssign | BinOp::ShrAssign => {
                let addr = self.lower_addr(lhs);
                let ty = self.expr_ir_type(lhs);
                let cur = self.new_vreg();
                self.emit(Inst::Load(cur, addr, ty));
                let rv = self.lower_expr(rhs);
                let ir_op = match op {
                    BinOp::AddAssign => BinIrOp::Add,
                    BinOp::SubAssign => BinIrOp::Sub,
                    BinOp::MulAssign => BinIrOp::Mul,
                    BinOp::DivAssign => BinIrOp::Div,
                    BinOp::ModAssign => BinIrOp::Mod,
                    BinOp::BitAndAssign => BinIrOp::And,
                    BinOp::BitOrAssign => BinIrOp::Or,
                    BinOp::BitXorAssign => BinIrOp::Xor,
                    BinOp::ShlAssign => BinIrOp::Shl,
                    BinOp::ShrAssign => BinIrOp::Shr,
                    _ => unreachable!(),
                };
                let result = self.new_vreg();
                self.emit(Inst::BinOp(result, ir_op, cur, rv, IrType::I64));
                self.emit(Inst::Store(result, addr, ty));
                result
            }
            BinOp::LogAnd => {
                let lv = self.lower_expr(lhs);
                let rhs_label = self.new_label();
                let false_label = self.new_label();
                let end_label = self.new_label();
                let result = self.new_vreg();
                self.emit(Inst::Alloca(result, 8));
                self.finish_block_and_start(Terminator::Branch(lv, rhs_label, false_label), rhs_label);
                let rv = self.lower_expr(rhs);
                let one = self.new_vreg();
                self.emit(Inst::Const(one, 1, IrType::I64));
                let zero = self.new_vreg();
                self.emit(Inst::Const(zero, 0, IrType::I64));
                // If rhs is true, result=1, else result=0
                let true_label = self.new_label();
                self.finish_block_and_start(Terminator::Branch(rv, true_label, false_label), true_label);
                self.emit(Inst::Store(one, result, IrType::I64));
                self.finish_block_and_start(Terminator::Jump(end_label), false_label);
                self.emit(Inst::Store(zero, result, IrType::I64));
                self.finish_block_and_start(Terminator::Jump(end_label), end_label);
                let dst = self.new_vreg();
                self.emit(Inst::Load(dst, result, IrType::I64));
                dst
            }
            BinOp::LogOr => {
                let lv = self.lower_expr(lhs);
                let rhs_label = self.new_label();
                let true_label = self.new_label();
                let end_label = self.new_label();
                let result = self.new_vreg();
                self.emit(Inst::Alloca(result, 8));
                self.finish_block_and_start(Terminator::Branch(lv, true_label, rhs_label), rhs_label);
                let rv = self.lower_expr(rhs);
                let false_label = self.new_label();
                self.finish_block_and_start(Terminator::Branch(rv, true_label, false_label), true_label);
                let one = self.new_vreg();
                self.emit(Inst::Const(one, 1, IrType::I64));
                self.emit(Inst::Store(one, result, IrType::I64));
                self.finish_block_and_start(Terminator::Jump(end_label), false_label);
                let zero = self.new_vreg();
                self.emit(Inst::Const(zero, 0, IrType::I64));
                self.emit(Inst::Store(zero, result, IrType::I64));
                self.finish_block_and_start(Terminator::Jump(end_label), end_label);
                let dst = self.new_vreg();
                self.emit(Inst::Load(dst, result, IrType::I64));
                dst
            }
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let lv = self.lower_expr(lhs);
                let rv = self.lower_expr(rhs);
                let cmp_op = match op {
                    BinOp::Eq => CmpOp::Eq,
                    BinOp::Ne => CmpOp::Ne,
                    BinOp::Lt => CmpOp::Lt,
                    BinOp::Le => CmpOp::Le,
                    BinOp::Gt => CmpOp::Gt,
                    BinOp::Ge => CmpOp::Ge,
                    _ => unreachable!(),
                };
                let dst = self.new_vreg();
                self.emit(Inst::Cmp(dst, cmp_op, lv, rv, IrType::I64));
                dst
            }
            _ => {
                let lv = self.lower_expr(lhs);
                let rv = self.lower_expr(rhs);
                let ir_op = match op {
                    BinOp::Add => BinIrOp::Add,
                    BinOp::Sub => BinIrOp::Sub,
                    BinOp::Mul => BinIrOp::Mul,
                    BinOp::Div => BinIrOp::Div,
                    BinOp::Mod => BinIrOp::Mod,
                    BinOp::BitAnd => BinIrOp::And,
                    BinOp::BitOr => BinIrOp::Or,
                    BinOp::BitXor => BinIrOp::Xor,
                    BinOp::Shl => BinIrOp::Shl,
                    BinOp::Shr => BinIrOp::Shr,
                    BinOp::Comma => return rv,
                    _ => unreachable!("unhandled binary op: {:?}", op),
                };
                let dst = self.new_vreg();
                self.emit(Inst::BinOp(dst, ir_op, lv, rv, IrType::I64));
                dst
            }
        }
    }

    fn lower_unary(&mut self, op: UnaryOp, operand: &Expr) -> VReg {
        match op {
            UnaryOp::Neg => {
                let v = self.lower_expr(operand);
                let dst = self.new_vreg();
                self.emit(Inst::UnOp(dst, UnIrOp::Neg, v, IrType::I64));
                dst
            }
            UnaryOp::Not => {
                let v = self.lower_expr(operand);
                let dst = self.new_vreg();
                self.emit(Inst::UnOp(dst, UnIrOp::Not, v, IrType::I64));
                dst
            }
            UnaryOp::BitNot => {
                let v = self.lower_expr(operand);
                let dst = self.new_vreg();
                self.emit(Inst::UnOp(dst, UnIrOp::BitNot, v, IrType::I64));
                dst
            }
            UnaryOp::Addr => {
                self.lower_addr(operand)
            }
            UnaryOp::Deref => {
                let v = self.lower_expr(operand);
                let dst = self.new_vreg();
                self.emit(Inst::Load(dst, v, IrType::I64));
                dst
            }
            UnaryOp::PreInc | UnaryOp::PreDec => {
                let addr = self.lower_addr(operand);
                let ty = self.expr_ir_type(operand);
                let cur = self.new_vreg();
                self.emit(Inst::Load(cur, addr, ty));
                let one = self.new_vreg();
                self.emit(Inst::Const(one, 1, IrType::I64));
                let op = if op == UnaryOp::PreInc { BinIrOp::Add } else { BinIrOp::Sub };
                let result = self.new_vreg();
                self.emit(Inst::BinOp(result, op, cur, one, IrType::I64));
                self.emit(Inst::Store(result, addr, ty));
                result
            }
            UnaryOp::PostInc | UnaryOp::PostDec => {
                let addr = self.lower_addr(operand);
                let ty = self.expr_ir_type(operand);
                let cur = self.new_vreg();
                self.emit(Inst::Load(cur, addr, ty));
                let one = self.new_vreg();
                self.emit(Inst::Const(one, 1, IrType::I64));
                let op = if op == UnaryOp::PostInc { BinIrOp::Add } else { BinIrOp::Sub };
                let result = self.new_vreg();
                self.emit(Inst::BinOp(result, op, cur, one, IrType::I64));
                self.emit(Inst::Store(result, addr, ty));
                cur // return old value
            }
        }
    }

    /// Lower an lvalue expression, return vreg holding the ADDRESS
    fn lower_addr(&mut self, expr: &Expr) -> VReg {
        match expr {
            Expr::Var(name, _) => {
                if let Some(&(alloca, _)) = self.locals.get(name) {
                    alloca
                } else {
                    let addr = self.new_vreg();
                    self.emit(Inst::LeaGlobal(addr, name.clone()));
                    addr
                }
            }
            Expr::Unary(UnaryOp::Deref, inner, _) => {
                self.lower_expr(inner)
            }
            Expr::Index(base, index, _) => {
                let base_v = self.lower_expr(base);
                let idx_v = self.lower_expr(index);
                let stride = self.index_stride_ast(base);
                let addr = self.new_vreg();
                self.emit(Inst::GetElementPtr(addr, base_v, idx_v, stride));
                addr
            }
            Expr::Member(base, member, _) => {
                let base_addr = self.lower_addr(base);
                let offset = self.member_offset(base, member);
                let addr = self.new_vreg();
                self.emit(Inst::AddImm(addr, base_addr, offset as i64));
                addr
            }
            _ => {
                // Fallback: evaluate and use as address
                self.lower_expr(expr)
            }
        }
    }

    // ── Type helpers ──

    fn expr_ir_type(&self, expr: &Expr) -> IrType {
        match expr {
            Expr::Var(name, _) => {
                self.locals.get(name).map(|(_, t)| *t).unwrap_or(IrType::I64)
            }
            Expr::Member(base, member, _) => self.member_type(base, member),
            _ => IrType::I64,
        }
    }

    fn index_stride_ast(&self, base: &Expr) -> usize {
        if let Expr::Var(name, _) = base {
            if let Some(ty) = self.local_ast_types.get(name) {
                match ty {
                    Type::Ptr(inner) | Type::Array(inner, _) => return inner.size().max(1),
                    _ => {}
                }
            }
        }
        8
    }

    fn index_elem_type(&self, base: &Expr) -> IrType {
        if let Expr::Var(name, _) = base {
            if let Some(ty) = self.local_ast_types.get(name) {
                match ty {
                    Type::Ptr(inner) | Type::Array(inner, _) => return IrType::from_ast_type(inner),
                    _ => {}
                }
            }
        }
        IrType::I64
    }

    fn member_offset(&self, base: &Expr, member: &str) -> usize {
        if let Expr::Var(name, _) = base {
            if let Some(ty) = self.local_ast_types.get(name) {
                if let Type::Struct { members, .. } = ty {
                    for m in members {
                        if m.name == member { return m.offset; }
                    }
                }
            }
        }
        0
    }

    fn member_type(&self, base: &Expr, member: &str) -> IrType {
        if let Expr::Var(name, _) = base {
            if let Some(ty) = self.local_ast_types.get(name) {
                if let Type::Struct { members, .. } = ty {
                    for m in members {
                        if m.name == member { return IrType::from_ast_type(&m.ty); }
                    }
                }
            }
        }
        IrType::I64
    }

    fn member_offset_ptr(&self, base: &Expr, member: &str) -> usize {
        if let Expr::Var(name, _) = base {
            if let Some(ty) = self.local_ast_types.get(name) {
                if let Type::Ptr(inner) = ty {
                    if let Type::Struct { members, .. } = inner.as_ref() {
                        for m in members {
                            if m.name == member { return m.offset; }
                        }
                    }
                }
            }
        }
        0
    }

    fn member_type_ptr(&self, base: &Expr, member: &str) -> IrType {
        if let Expr::Var(name, _) = base {
            if let Some(ty) = self.local_ast_types.get(name) {
                if let Type::Ptr(inner) = ty {
                    if let Type::Struct { members, .. } = inner.as_ref() {
                        for m in members {
                            if m.name == member { return IrType::from_ast_type(&m.ty); }
                        }
                    }
                }
            }
        }
        IrType::I64
    }
}
