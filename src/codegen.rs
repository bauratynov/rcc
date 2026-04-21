use std::collections::HashMap;
use std::fmt::Write;

use crate::ast::*;
use crate::error::CompileError;

struct Generator {
    output: String,
    label_count: usize,
    locals: HashMap<String, i64>,
    local_types: HashMap<String, Type>,
    global_types: HashMap<String, Type>,
    struct_types: HashMap<String, Type>,
    func_return_types: HashMap<String, Type>,
    stack_size: i64,
    current_fn: String,
    break_labels: Vec<String>,
    continue_labels: Vec<String>,
    call_temp_offset: i64, // tracks temp area to avoid overlap in nested calls
}

// x86-64 calling conventions
#[cfg(target_os = "windows")]
const ARG_REGS: &[&str] = &["%rcx", "%rdx", "%r8", "%r9"];
#[cfg(target_os = "windows")]
const FLOAT_REGS: &[&str] = &["%xmm0", "%xmm1", "%xmm2", "%xmm3"];
#[cfg(target_os = "windows")]
const MAX_REG_ARGS: usize = 4;
#[cfg(target_os = "windows")]
const SHADOW_SPACE: i64 = 32;
#[cfg(target_os = "windows")]
const STACK_ARG_START: usize = 48; // rbp+48 for 5th arg on Windows

#[cfg(not(target_os = "windows"))]
const ARG_REGS: &[&str] = &["%rdi", "%rsi", "%rdx", "%rcx", "%r8", "%r9"];
#[cfg(not(target_os = "windows"))]
const FLOAT_REGS: &[&str] = &["%xmm0", "%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5"];
#[cfg(not(target_os = "windows"))]
const MAX_REG_ARGS: usize = 6;
#[cfg(not(target_os = "windows"))]
const SHADOW_SPACE: i64 = 0;
#[cfg(not(target_os = "windows"))]
const STACK_ARG_START: usize = 16; // rbp+16 for 7th arg on Linux

impl Generator {
    fn new() -> Self {
        Self {
            output: String::new(),
            label_count: 0,
            locals: HashMap::new(),
            local_types: HashMap::new(),
            global_types: HashMap::new(),
            struct_types: HashMap::new(),
            func_return_types: HashMap::new(),
            stack_size: 0,
            current_fn: String::new(),
            break_labels: Vec::new(),
            continue_labels: Vec::new(),
            call_temp_offset: 0,
        }
    }

    fn emit(&mut self, s: &str) { writeln!(self.output, "{}", s).unwrap(); }
    fn emitf(&mut self, s: &str) { writeln!(self.output, "  {}", s).unwrap(); }

    fn new_label(&mut self) -> String {
        self.label_count += 1;
        format!(".L{}", self.label_count)
    }

    fn alloc_local(&mut self, name: &str, size: i64) -> i64 {
        self.stack_size += size;
        self.stack_size = (self.stack_size + 7) & !7;
        let offset = -self.stack_size;
        self.locals.insert(name.to_string(), offset);
        offset
    }

    fn count_locals(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl(decl, _) => {
                let mut size = match &decl.ty {
                    Type::Array(_, _) | Type::Struct { .. } => decl.ty.size().max(8) as i64,
                    _ => 8,
                };
                // If init list, ensure we allocate enough
                if let Some(Expr::InitList(items, _)) = &decl.init {
                    let elem_size = match &decl.ty {
                        Type::Array(base, _) => base.size(),
                        _ => 8,
                    };
                    let flat_count = count_flat_init(items);
                    let needed = (flat_count * elem_size).max(8) as i64;
                    if needed > size { size = needed; }
                }
                self.alloc_local(&decl.name, size);
                // For inferred arrays (int arr[] = {...}), use correct size
                let ty = if let Some(Expr::InitList(items, _)) = &decl.init {
                    if let Type::Array(base, 0) = &decl.ty {
                        Type::Array(base.clone(), items.len())
                    } else {
                        decl.ty.clone()
                    }
                } else {
                    decl.ty.clone()
                };
                self.local_types.insert(decl.name.clone(), ty.clone());
                self.register_struct_type(&ty);
                if let Some(init) = &decl.init { self.count_locals_expr(init); }
            }
            Stmt::Block(stmts, _) => { for s in stmts { self.count_locals(s); } }
            Stmt::If(_, then, els, _) => {
                self.count_locals(then);
                if let Some(els) = els { self.count_locals(els); }
            }
            Stmt::While(_, body, _) | Stmt::DoWhile(body, _, _) => { self.count_locals(body); }
            Stmt::For(init, _, _, body, _) => {
                if let Some(init) = init { self.count_locals(init); }
                self.count_locals(body);
            }
            Stmt::Switch(_, body, _) | Stmt::Case(_, body, _) | Stmt::Default(body, _)
            | Stmt::Label(_, body, _) => { self.count_locals(body); }
            Stmt::Expr(e, _) | Stmt::Return(Some(e), _) => { self.count_locals_expr(e); }
            _ => {}
        }
    }

    fn count_locals_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::StmtExpr(stmts, last, _) => {
                for s in stmts { self.count_locals(s); }
                self.count_locals_expr(last);
            }
            Expr::Binary(_, l, r, _) => { self.count_locals_expr(l); self.count_locals_expr(r); }
            Expr::Unary(_, e, _) | Expr::Cast(_, e, _) => { self.count_locals_expr(e); }
            Expr::Cond(c, t, e, _) => { self.count_locals_expr(c); self.count_locals_expr(t); self.count_locals_expr(e); }
            Expr::Call(f, args, _) => { self.count_locals_expr(f); for a in args { self.count_locals_expr(a); } }
            _ => {}
        }
    }

    fn gen_program(&mut self, program: &TranslationUnit) -> Result<(), CompileError> {
        // Import struct types from parser
        for (name, ty) in &program.struct_types {
            self.struct_types.insert(name.clone(), ty.clone());
        }
        // Import typedefs that are structs
        for (_name, ty) in &program.typedefs {
            self.register_struct_type(ty);
        }

        // Collect global variable types
        for decl in &program.decls {
            if let TopLevel::GlobalVar(vd, _) = decl {
                if !vd.name.is_empty() {
                    self.global_types.insert(vd.name.clone(), vd.ty.clone());
                }
            }
        }

        // Collect function return types
        for decl in &program.decls {
            match decl {
                TopLevel::FuncDef { name, return_ty, .. } | TopLevel::FuncDecl { name, return_ty, .. } => {
                    self.func_return_types.insert(name.clone(), return_ty.clone());
                }
                _ => {}
            }
        }

        // Pre-scan: collect all struct types from declarations
        for decl in &program.decls {
            match decl {
                TopLevel::GlobalVar(vd, _) => {
                    self.register_struct_type(&vd.ty);
                }
                TopLevel::FuncDef { params, .. } | TopLevel::FuncDecl { params, .. } => {
                    for (ty, _) in params {
                        self.register_struct_type(ty);
                    }
                }
                _ => {}
            }
        }

        for decl in &program.decls {
            match decl {
                TopLevel::FuncDef { name, params, body, .. } => self.gen_func(name, params, body)?,
                TopLevel::GlobalVar(decl, _) => self.gen_global_var(decl)?,
                TopLevel::FuncDecl { .. } => {}
            }
        }
        Ok(())
    }

    fn gen_global_var(&mut self, decl: &VarDecl) -> Result<(), CompileError> {
        if decl.name.is_empty() { return Ok(()); } // skip anonymous struct defs
        if let Some(Expr::IntLit(val, _)) = &decl.init {
            self.emit("  .data");
            self.emit(&format!("  .globl {}", decl.name));
            self.emit(&format!("{}:", decl.name));
            match decl.ty.size() {
                1 => self.emit(&format!("  .byte {}", val)),
                2 => self.emit(&format!("  .short {}", val)),
                4 => self.emit(&format!("  .long {}", val)),
                _ => self.emit(&format!("  .quad {}", val)),
            }
        } else if let Some(Expr::StrLit(s, _)) = &decl.init {
            self.emit("  .data");
            let str_label = self.new_label();
            self.emit(&format!("{}:", str_label));
            self.emit(&format!("  .string {:?}", s));
            self.emit(&format!("  .globl {}", decl.name));
            self.emit(&format!("{}:", decl.name));
            self.emit(&format!("  .quad {}", str_label));
        } else if let Some(Expr::InitList(items, _)) = &decl.init {
            self.emit("  .data");
            self.emit(&format!("  .globl {}", decl.name));
            self.emit(&format!("{}:", decl.name));
            for item in items {
                match item {
                    Expr::IntLit(val, _) => self.emit(&format!("  .quad {}", val)),
                    Expr::Var(name, _) => self.emit(&format!("  .quad {}", name)), // function pointer
                    Expr::StrLit(s, _) => {
                        let label = self.new_label();
                        self.emit(&format!("  .quad {}", label));
                        // Defer string data
                        self.emit(&format!("  .section .rodata"));
                        self.emit(&format!("{}:", label));
                        self.emit(&format!("  .string {:?}", s));
                        self.emit("  .data");
                    }
                    Expr::Cast(_, inner, _) => {
                        // (void*)0 etc
                        if let Expr::IntLit(val, _) = inner.as_ref() {
                            self.emit(&format!("  .quad {}", val));
                        } else if let Expr::Var(name, _) = inner.as_ref() {
                            self.emit(&format!("  .quad {}", name));
                        } else {
                            self.emit("  .quad 0");
                        }
                    }
                    _ => self.emit("  .quad 0"),
                }
            }
        } else {
            self.emit("  .bss");
            self.emit(&format!("  .globl {}", decl.name));
            self.emit(&format!("{}:", decl.name));
            self.emit(&format!("  .zero {}", decl.ty.size().max(8)));
        }
        self.emit("  .text");
        Ok(())
    }

    fn gen_func(&mut self, name: &str, params: &[(Type, String)], body: &Stmt) -> Result<(), CompileError> {
        self.locals.clear();
        self.local_types.clear();
        self.stack_size = 0;
        self.call_temp_offset = 0;
        self.current_fn = name.to_string();

        self.count_locals(body);

        // Allocate params as locals
        for (ty, pname) in params {
            if !self.locals.contains_key(pname) {
                self.alloc_local(pname, ty.size().max(8) as i64);
                self.local_types.insert(pname.clone(), ty.clone());
            }
        }

        // Count total call args for temp space
        let call_temp_space = count_call_args(body) as i64 * 8 + 64;

        // Align stack to 16 bytes. Windows needs 32-byte shadow space + call temps.
        let aligned_stack = ((self.stack_size + call_temp_space + SHADOW_SPACE + 15) & !15).max(48);

        self.emit(&format!("  .globl {}", name));
        self.emit("  .text");
        self.emit(&format!("{}:", name));
        self.emitf("push %rbp");
        self.emitf("mov %rsp, %rbp");
        self.emitf(&format!("sub ${}, %rsp", aligned_stack));

        // Store params from Windows registers / stack to local slots
        for (i, (ty, pname)) in params.iter().enumerate() {
            let offset = self.locals[pname];
            if i < MAX_REG_ARGS {
                if self.is_float_type(ty) {
                    // Double params come in xmm0-xmm3
                    self.emitf(&format!("movsd {}, {}(%rbp)", FLOAT_REGS[i], offset));
                } else {
                    self.emitf(&format!("mov {}, {}(%rbp)", ARG_REGS[i], offset));
                }
            } else {
                // Windows x64 stack args: rbp+48 for 5th arg, rbp+56 for 6th, etc.
                // rbp+0=old_rbp, +8=ret_addr, +16..+40=shadow(32 bytes), +48=arg5
                let stack_off = STACK_ARG_START + (i - MAX_REG_ARGS) * 8;
                self.emitf(&format!("mov {}(%rbp), %rax", stack_off));
                self.emitf(&format!("mov %rax, {}(%rbp)", offset));
            }
        }

        self.gen_stmt(body)?;

        self.emit(&format!(".Lreturn.{}:", name));
        self.emitf("mov %rbp, %rsp");
        self.emitf("pop %rbp");
        self.emitf("ret");
        Ok(())
    }

    fn gen_stmt(&mut self, stmt: &Stmt) -> Result<(), CompileError> {
        match stmt {
            Stmt::Return(expr, _) => {
                if let Some(expr) = expr { self.gen_expr(expr)?; }
                self.emitf(&format!("jmp .Lreturn.{}", self.current_fn));
            }
            Stmt::Expr(expr, _) => { self.gen_expr(expr)?; }
            Stmt::Block(stmts, _) => { for s in stmts { self.gen_stmt(s)?; } }
            Stmt::VarDecl(decl, _) => {
                if let Some(init) = &decl.init {
                    // Re-allocate if array size was inferred from init list
                    if let Expr::InitList(items, _) = init {
                        let flat = flatten_init_list(items);
                        let elem_size = match &decl.ty {
                            Type::Array(base, _) => base.size(),
                            _ => 8,
                        };
                        let needed = (flat.len() * elem_size).max(8) as i64;
                        let current = self.locals.get(&decl.name).map(|o| -o).unwrap_or(0);
                        if needed > current {
                            // Re-allocate with correct size
                            self.stack_size += needed - current;
                            self.stack_size = (self.stack_size + 7) & !7;
                            let new_offset = -self.stack_size;
                            self.locals.insert(decl.name.clone(), new_offset);
                        }
                    }
                    let offset = self.locals[&decl.name];
                    match init {
                        Expr::InitList(items, _) => {
                            // Flatten nested init lists for struct init
                            let flat = flatten_init_list(items);
                            let elem_size = match &decl.ty {
                                Type::Array(base, _) => base.size(),
                                _ => 8,
                            };
                            for (i, item) in flat.iter().enumerate() {
                                self.gen_expr(item)?;
                                let elem_offset = offset + (i * elem_size) as i64;
                                self.emitf(&format!("lea {}(%rbp), %rdi", elem_offset));
                                self.emit_store(elem_size);
                            }
                        }
                        _ => {
                            self.gen_expr(init)?;
                            let is_float = self.is_float_type(&decl.ty);
                            self.emitf(&format!("lea {}(%rbp), %rdi", offset));
                            if is_float {
                                // If init wasn't float, convert
                                if !self.is_float_expr(init) {
                                    self.emitf("cvtsi2sd %rax, %xmm0");
                                }
                                self.emit_store_float();
                            } else {
                                let size = decl.ty.size();
                                self.emit_store(size);
                            }
                        }
                    }
                }
            }
            Stmt::If(cond, then, els, _) => {
                let else_label = self.new_label();
                let end_label = self.new_label();
                self.gen_expr(cond)?;
                self.emitf("cmp $0, %rax");
                self.emitf(&format!("je {}", else_label));
                self.gen_stmt(then)?;
                self.emitf(&format!("jmp {}", end_label));
                self.emit(&format!("{}:", else_label));
                if let Some(els) = els { self.gen_stmt(els)?; }
                self.emit(&format!("{}:", end_label));
            }
            Stmt::While(cond, body, _) => {
                let begin = self.new_label();
                let end = self.new_label();
                self.break_labels.push(end.clone());
                self.continue_labels.push(begin.clone());
                self.emit(&format!("{}:", begin));
                self.gen_expr(cond)?;
                self.emitf("cmp $0, %rax");
                self.emitf(&format!("je {}", end));
                self.gen_stmt(body)?;
                self.emitf(&format!("jmp {}", begin));
                self.emit(&format!("{}:", end));
                self.break_labels.pop();
                self.continue_labels.pop();
            }
            Stmt::DoWhile(body, cond, _) => {
                let begin = self.new_label();
                let end = self.new_label();
                let cont = self.new_label();
                self.break_labels.push(end.clone());
                self.continue_labels.push(cont.clone());
                self.emit(&format!("{}:", begin));
                self.gen_stmt(body)?;
                self.emit(&format!("{}:", cont));
                self.gen_expr(cond)?;
                self.emitf("cmp $0, %rax");
                self.emitf(&format!("jne {}", begin));
                self.emit(&format!("{}:", end));
                self.break_labels.pop();
                self.continue_labels.pop();
            }
            Stmt::For(init, cond, inc, body, _) => {
                let begin = self.new_label();
                let end = self.new_label();
                let cont = self.new_label();
                self.break_labels.push(end.clone());
                self.continue_labels.push(cont.clone());
                if let Some(init) = init { self.gen_stmt(init)?; }
                self.emit(&format!("{}:", begin));
                if let Some(cond) = cond {
                    self.gen_expr(cond)?;
                    self.emitf("cmp $0, %rax");
                    self.emitf(&format!("je {}", end));
                }
                self.gen_stmt(body)?;
                self.emit(&format!("{}:", cont));
                if let Some(inc) = inc { self.gen_expr(inc)?; }
                self.emitf(&format!("jmp {}", begin));
                self.emit(&format!("{}:", end));
                self.break_labels.pop();
                self.continue_labels.pop();
            }
            Stmt::Break(_) => {
                if let Some(l) = self.break_labels.last() { self.emitf(&format!("jmp {}", l)); }
            }
            Stmt::Continue(_) => {
                if let Some(l) = self.continue_labels.last() { self.emitf(&format!("jmp {}", l)); }
            }
            Stmt::Switch(cond, body, _) => {
                let end = self.new_label();
                self.break_labels.push(end.clone());
                self.gen_expr(cond)?;
                self.emitf("mov %rax, %r10");

                // Collect case values and labels, then emit jump table
                let mut cases = Vec::new();
                let default_label = self.new_label();
                self.collect_cases(body, &mut cases);

                // Emit comparisons
                for (val, label) in &cases {
                    self.emitf(&format!("cmp ${}, %r10", val));
                    self.emitf(&format!("je {}", label));
                }
                self.emitf(&format!("jmp {}", default_label));

                // Emit body with case labels
                self.gen_switch_body(body, &cases, &default_label)?;

                // default_label might already be emitted by gen_switch_body for Default
                // Emit it again only if no Default branch exists (acts as fallthrough target)
                if !has_default(body) {
                    self.emit(&format!("{}:", default_label));
                }
                self.emit(&format!("{}:", end));
                self.break_labels.pop();
            }
            Stmt::Case(_, _, _) | Stmt::Default(_, _) => {
                // Handled by gen_switch_body inside Switch
            }
            Stmt::Label(name, body, _) => {
                self.emit(&format!(".Luser.{}.{}:", self.current_fn, name));
                self.gen_stmt(body)?;
            }
            Stmt::Goto(name, _) => { self.emitf(&format!("jmp .Luser.{}.{}", self.current_fn, name)); }
            Stmt::Null => {}
        }
        Ok(())
    }

    fn gen_expr(&mut self, expr: &Expr) -> Result<(), CompileError> {
        match expr {
            Expr::IntLit(val, _) => { self.emitf(&format!("mov ${}, %rax", val)); }
            Expr::FloatLit(f, _) => {
                let bits = (*f).to_bits();
                let label = self.new_label();
                self.emit("  .section .rodata");
                self.emit("  .align 8");
                self.emit(&format!("{}:", label));
                self.emit(&format!("  .quad {}", bits));
                self.emit("  .text");
                self.emitf(&format!("movsd {}(%rip), %xmm0", label));
                // Keep result in xmm0, also copy to rax for integer contexts
                self.emitf("cvttsd2si %xmm0, %rax");
            }
            Expr::CharLit(val, _) => { self.emitf(&format!("mov ${}, %rax", val)); }
            Expr::StrLit(s, _) => {
                let label = self.new_label();
                self.emit("  .section .rodata");
                self.emit(&format!("{}:", label));
                self.emit(&format!("  .string {:?}", s));
                self.emit("  .text");
                self.emitf(&format!("lea {}(%rip), %rax", label));
            }
            Expr::Var(name, _) => {
                let ty = self.local_types.get(name)
                    .or_else(|| self.global_types.get(name))
                    .cloned().unwrap_or(Type::Long);
                let is_array = matches!(ty, Type::Array(_, _));
                let is_float = self.is_float_type(&ty);
                if let Some(&offset) = self.locals.get(name) {
                    self.emitf(&format!("lea {}(%rbp), %rax", offset));
                    if !is_array {
                        if is_float {
                            self.emit_load_float();
                            self.emitf("cvttsd2si %xmm0, %rax"); // also in rax
                        } else {
                            self.emit_load(ty.size());
                        }
                    }
                } else {
                    self.emitf(&format!("lea {}(%rip), %rax", name));
                    if !is_array {
                        if is_float {
                            self.emit_load_float();
                            self.emitf("cvttsd2si %xmm0, %rax");
                        } else {
                            self.emit_load(ty.size());
                        }
                    }
                }
            }
            Expr::Binary(op, lhs, rhs, _) => {
                match op {
                    BinOp::Assign => {
                        let lhs_ty = self.expr_type(lhs);
                        let lhs_float = self.is_float_type(&lhs_ty);
                        let size = lhs_ty.size();
                        let is_struct = matches!(lhs_ty, Type::Struct { .. });

                        if is_struct && size > 8 {
                            // Struct assignment: memcpy-style
                            // Get src address
                            self.gen_addr(rhs)?;
                            self.emitf("push %rdi"); // save src addr
                            self.gen_addr(lhs)?;     // rdi = dst addr
                            self.emitf("pop %rsi");  // rsi = src addr
                            // Copy 8 bytes at a time
                            let mut off = 0;
                            while off + 8 <= size {
                                self.emitf(&format!("mov {}(%rsi), %rax", off));
                                self.emitf(&format!("mov %rax, {}(%rdi)", off));
                                off += 8;
                            }
                            while off < size {
                                self.emitf(&format!("movb {}(%rsi), %al", off));
                                self.emitf(&format!("movb %al, {}(%rdi)", off));
                                off += 1;
                            }
                        } else if lhs_float {
                            self.gen_expr(rhs)?;
                            if !self.is_float_expr(rhs) {
                                self.emitf("cvtsi2sd %rax, %xmm0");
                            }
                            self.emitf("sub $8, %rsp");
                            self.emitf("movsd %xmm0, (%rsp)");
                            self.gen_addr(lhs)?;
                            self.emitf("movsd (%rsp), %xmm0");
                            self.emitf("add $8, %rsp");
                            self.emit_store_float();
                        } else {
                            self.gen_expr(rhs)?;
                            self.emitf("push %rax");
                            self.gen_addr(lhs)?;
                            self.emitf("pop %rax");
                            self.emit_store(size);
                        }
                    }
                    BinOp::AddAssign | BinOp::SubAssign | BinOp::MulAssign
                    | BinOp::DivAssign | BinOp::ModAssign
                    | BinOp::BitAndAssign | BinOp::BitOrAssign | BinOp::BitXorAssign
                    | BinOp::ShlAssign | BinOp::ShrAssign => {
                        let lhs_size = self.expr_type(lhs).size();
                        self.gen_addr(lhs)?;
                        self.emitf("push %rdi");
                        self.emitf("mov %rdi, %rax");
                        // Load current value with correct size
                        match lhs_size {
                            1 => self.emitf("movsbl (%rax), %eax"),
                            2 => self.emitf("movswl (%rax), %eax"),
                            4 => self.emitf("movslq (%rax), %rax"),
                            _ => self.emitf("mov (%rax), %rax"),
                        }
                        self.emitf("push %rax");
                        self.gen_expr(rhs)?;
                        self.emitf("mov %rax, %rcx");
                        self.emitf("pop %rax");
                        match op {
                            BinOp::AddAssign => self.emitf("add %rcx, %rax"),
                            BinOp::SubAssign => self.emitf("sub %rcx, %rax"),
                            BinOp::MulAssign => self.emitf("imul %rcx, %rax"),
                            BinOp::DivAssign => { self.emitf("cqo"); self.emitf("idiv %rcx"); }
                            BinOp::ModAssign => { self.emitf("cqo"); self.emitf("idiv %rcx"); self.emitf("mov %rdx, %rax"); }
                            BinOp::BitAndAssign => self.emitf("and %rcx, %rax"),
                            BinOp::BitOrAssign => self.emitf("or %rcx, %rax"),
                            BinOp::BitXorAssign => self.emitf("xor %rcx, %rax"),
                            BinOp::ShlAssign => self.emitf("shl %cl, %rax"),
                            BinOp::ShrAssign => self.emitf("sar %cl, %rax"),
                            _ => unreachable!(),
                        }
                        self.emitf("pop %rdi");
                        self.emit_store(lhs_size);
                    }
                    BinOp::LogAnd => {
                        let fl = self.new_label(); let el = self.new_label();
                        self.gen_expr(lhs)?; self.emitf("cmp $0, %rax"); self.emitf(&format!("je {}", fl));
                        self.gen_expr(rhs)?; self.emitf("cmp $0, %rax"); self.emitf(&format!("je {}", fl));
                        self.emitf("mov $1, %rax"); self.emitf(&format!("jmp {}", el));
                        self.emit(&format!("{}:", fl)); self.emitf("mov $0, %rax");
                        self.emit(&format!("{}:", el));
                    }
                    BinOp::LogOr => {
                        let tl = self.new_label(); let el = self.new_label();
                        self.gen_expr(lhs)?; self.emitf("cmp $0, %rax"); self.emitf(&format!("jne {}", tl));
                        self.gen_expr(rhs)?; self.emitf("cmp $0, %rax"); self.emitf(&format!("jne {}", tl));
                        self.emitf("mov $0, %rax"); self.emitf(&format!("jmp {}", el));
                        self.emit(&format!("{}:", tl)); self.emitf("mov $1, %rax");
                        self.emit(&format!("{}:", el));
                    }
                    _ => {
                        let lhs_float = self.is_float_expr(lhs);
                        let rhs_float = self.is_float_expr(rhs);
                        let use_float = lhs_float || rhs_float;

                        if use_float && matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div
                            | BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge) {
                            // Float binary operation
                            self.gen_expr(rhs)?;
                            if rhs_float {
                                // xmm0 has rhs float, save to stack
                                self.emitf("sub $8, %rsp");
                                self.emitf("movsd %xmm0, (%rsp)");
                            } else {
                                self.emitf("cvtsi2sd %rax, %xmm0");
                                self.emitf("sub $8, %rsp");
                                self.emitf("movsd %xmm0, (%rsp)");
                            }
                            self.gen_expr(lhs)?;
                            if lhs_float {
                                // xmm0 has lhs
                            } else {
                                self.emitf("cvtsi2sd %rax, %xmm0");
                            }
                            // xmm0 = lhs, stack top = rhs
                            self.emitf("movsd (%rsp), %xmm1");
                            self.emitf("add $8, %rsp");
                            match op {
                                BinOp::Add => self.emitf("addsd %xmm1, %xmm0"),
                                BinOp::Sub => self.emitf("subsd %xmm1, %xmm0"),
                                BinOp::Mul => self.emitf("mulsd %xmm1, %xmm0"),
                                BinOp::Div => self.emitf("divsd %xmm1, %xmm0"),
                                BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le
                                | BinOp::Gt | BinOp::Ge => {
                                    self.emitf("ucomisd %xmm1, %xmm0");
                                    let set = match op {
                                        BinOp::Eq => "sete", BinOp::Ne => "setne",
                                        BinOp::Lt => "setb", BinOp::Le => "setbe",
                                        BinOp::Gt => "seta", BinOp::Ge => "setae",
                                        _ => unreachable!(),
                                    };
                                    self.emitf(&format!("{} %al", set));
                                    self.emitf("movzbl %al, %eax");
                                    // Result is int in rax, not float
                                    return Ok(());
                                }
                                _ => unreachable!(),
                            }
                            // Result in xmm0, also put in rax
                            self.emitf("cvttsd2si %xmm0, %rax");
                        } else {
                            // Integer binary operation
                            self.gen_expr(rhs)?; self.emitf("push %rax");
                            self.gen_expr(lhs)?; self.emitf("pop %rcx");
                            // Pointer arithmetic: scale integer operand by pointee size
                            if matches!(op, BinOp::Add | BinOp::Sub) {
                                let lty = self.expr_type(lhs);
                                let rty = self.expr_type(rhs);
                                if lty.is_ptr() && !rty.is_ptr() {
                                    let scale = lty.base_type().map(|t| t.size()).unwrap_or(1);
                                    if scale > 1 { self.emitf(&format!("imul ${}, %rcx", scale)); }
                                } else if rty.is_ptr() && !lty.is_ptr() {
                                    let scale = rty.base_type().map(|t| t.size()).unwrap_or(1);
                                    if scale > 1 { self.emitf(&format!("imul ${}, %rax", scale)); }
                                }
                            }
                            match op {
                                BinOp::Add => self.emitf("add %rcx, %rax"),
                                BinOp::Sub => self.emitf("sub %rcx, %rax"),
                                BinOp::Mul => self.emitf("imul %rcx, %rax"),
                                BinOp::Div => { self.emitf("cqo"); self.emitf("idiv %rcx"); }
                                BinOp::Mod => { self.emitf("cqo"); self.emitf("idiv %rcx"); self.emitf("mov %rdx, %rax"); }
                                BinOp::BitAnd => self.emitf("and %rcx, %rax"),
                                BinOp::BitOr => self.emitf("or %rcx, %rax"),
                                BinOp::BitXor => self.emitf("xor %rcx, %rax"),
                                BinOp::Shl => self.emitf("shl %cl, %rax"),
                                BinOp::Shr => self.emitf("sar %cl, %rax"),
                                BinOp::Eq => { self.emitf("cmp %rcx, %rax"); self.emitf("sete %al"); self.emitf("movzbl %al, %eax"); }
                                BinOp::Ne => { self.emitf("cmp %rcx, %rax"); self.emitf("setne %al"); self.emitf("movzbl %al, %eax"); }
                                BinOp::Lt => { self.emitf("cmp %rcx, %rax"); self.emitf("setl %al"); self.emitf("movzbl %al, %eax"); }
                                BinOp::Le => { self.emitf("cmp %rcx, %rax"); self.emitf("setle %al"); self.emitf("movzbl %al, %eax"); }
                                BinOp::Gt => { self.emitf("cmp %rcx, %rax"); self.emitf("setg %al"); self.emitf("movzbl %al, %eax"); }
                                BinOp::Ge => { self.emitf("cmp %rcx, %rax"); self.emitf("setge %al"); self.emitf("movzbl %al, %eax"); }
                                BinOp::Comma => { self.emitf("mov %rcx, %rax"); }
                                _ => unreachable!(),
                            }
                        }
                    }
                }
            }
            Expr::Unary(op, operand, _) => {
                match op {
                    UnaryOp::Neg => { self.gen_expr(operand)?; self.emitf("neg %rax"); }
                    UnaryOp::Not => { self.gen_expr(operand)?; self.emitf("cmp $0, %rax"); self.emitf("sete %al"); self.emitf("movzbl %al, %eax"); }
                    UnaryOp::BitNot => { self.gen_expr(operand)?; self.emitf("not %rax"); }
                    UnaryOp::Addr => { self.gen_addr(operand)?; self.emitf("mov %rdi, %rax"); }
                    UnaryOp::Deref => {
                        let pointee = self.expr_type(expr);
                        let size = pointee.size();
                        self.gen_expr(operand)?;
                        self.emit_load(size);
                    }
                    UnaryOp::PreInc => {
                        let ty = self.expr_type(operand);
                        let add = if ty.size() <= 4 { "addl" } else { "addq" };
                        self.gen_addr(operand)?;
                        self.emitf(&format!("{} $1, (%rdi)", add));
                        self.emitf("mov %rdi, %rax");
                        self.emit_load(ty.size());
                    }
                    UnaryOp::PreDec => {
                        let ty = self.expr_type(operand);
                        let sub = if ty.size() <= 4 { "subl" } else { "subq" };
                        self.gen_addr(operand)?;
                        self.emitf(&format!("{} $1, (%rdi)", sub));
                        self.emitf("mov %rdi, %rax");
                        self.emit_load(ty.size());
                    }
                    UnaryOp::PostInc => {
                        let ty = self.expr_type(operand);
                        let add = if ty.size() <= 4 { "addl" } else { "addq" };
                        self.gen_addr(operand)?;
                        self.emitf("mov %rdi, %rax");
                        self.emit_load(ty.size());
                        self.emitf(&format!("{} $1, (%rdi)", add));
                    }
                    UnaryOp::PostDec => {
                        let ty = self.expr_type(operand);
                        let sub = if ty.size() <= 4 { "subl" } else { "subq" };
                        self.gen_addr(operand)?;
                        self.emitf("mov %rdi, %rax");
                        self.emit_load(ty.size());
                        self.emitf(&format!("{} $1, (%rdi)", sub));
                    }
                }
            }
            Expr::Call(func, args, _) => {
                let nargs = args.len();
                // Windows x64 ABI:
                // - First 4 args in rcx, rdx, r8, r9
                // - Additional args on stack at rsp+32, rsp+40, etc.
                // - 32-byte shadow space always required
                // - Stack must be 16-byte aligned before call

                // Use unique temp area per call to avoid overlap in nested calls
                let arg_base = self.stack_size + 8 + self.call_temp_offset;
                self.call_temp_offset += (nargs as i64) * 8 + 8;
                let total_stack_args = if nargs > MAX_REG_ARGS { nargs - MAX_REG_ARGS } else { 0 };

                // Evaluate all args and store to temp area (no push/pop)
                for (i, arg) in args.iter().enumerate() {
                    self.gen_expr(arg)?;
                    if self.is_float_expr(arg) {
                        self.emitf("movq %xmm0, %rax");
                    }
                    let off = arg_base + (i as i64) * 8;
                    self.emitf(&format!("mov %rax, -{}(%rbp)", off));
                }

                // Allocate call frame: shadow(32) + stack args
                let call_frame = ((SHADOW_SPACE as usize + total_stack_args * 8 + 15) / 16) * 16;
                if total_stack_args > 0 {
                    self.emitf(&format!("sub ${}, %rsp", call_frame));
                    for i in 4..nargs {
                        let off = arg_base + (i as i64) * 8;
                        self.emitf(&format!("mov -{}(%rbp), %rax", off));
                        self.emitf(&format!("mov %rax, {}(%rsp)", SHADOW_SPACE as usize + (i - MAX_REG_ARGS) * 8));
                    }
                }

                // Load register args + mirror to XMM for variadic
                for i in 0..nargs.min(MAX_REG_ARGS) {
                    let off = arg_base + (i as i64) * 8;
                    self.emitf(&format!("mov -{}(%rbp), {}", off, ARG_REGS[i]));
                    self.emitf(&format!("movq {}, {}", ARG_REGS[i], FLOAT_REGS[i]));
                }

                let ret_ty = if let Expr::Var(name, _) = func.as_ref() {
                    self.emitf(&format!("call {}", name));
                    self.func_return_types.get(name).cloned().unwrap_or(Type::Int)
                } else {
                    self.gen_expr(func)?;
                    self.emitf("call *%rax");
                    Type::Int
                };

                if total_stack_args > 0 {
                    self.emitf(&format!("add ${}, %rsp", call_frame));
                }

                // If function returns float/double, result is in xmm0
                if self.is_float_type(&ret_ty) {
                    // Keep xmm0 as-is, also put raw bits in rax for integer contexts
                    self.emitf("movq %xmm0, %rax");
                }
            }
            Expr::Cond(cond, then, els, _) => {
                let el = self.new_label(); let end = self.new_label();
                self.gen_expr(cond)?; self.emitf("cmp $0, %rax"); self.emitf(&format!("je {}", el));
                self.gen_expr(then)?; self.emitf(&format!("jmp {}", end));
                self.emit(&format!("{}:", el)); self.gen_expr(els)?;
                self.emit(&format!("{}:", end));
            }
            Expr::Sizeof(arg, _) => {
                let size = match arg.as_ref() {
                    SizeofArg::Type(ty) => ty.size(),
                    SizeofArg::Expr(e) => self.expr_type(e).size(),
                };
                self.emitf(&format!("mov ${}, %rax", size));
            }
            Expr::Index(array, index, _) => {
                let stride = self.index_stride(array);
                let elem_ty = self.expr_type(expr);
                let elem_size = elem_ty.size();
                self.gen_expr(index)?;
                self.emitf(&format!("imul ${}, %rax", stride));
                self.emitf("push %rax");
                self.gen_expr(array)?;
                self.emitf("pop %rcx");
                self.emitf("add %rcx, %rax");
                self.emit_load(elem_size);
            }
            Expr::Member(base, member, _) => {
                let mem_ty = self.expr_type(expr);
                let is_float = self.is_float_type(&mem_ty);
                self.gen_addr(base)?;
                self.emitf("mov %rdi, %rax");
                let offset = self.find_member_offset(base, member);
                if offset != 0 {
                    self.emitf(&format!("add ${}, %rax", offset));
                }
                if is_float {
                    self.emit_load_float();
                    self.emitf("cvttsd2si %xmm0, %rax");
                } else {
                    self.emit_load(mem_ty.size());
                }
            }
            Expr::Arrow(base, member, _) => {
                let mem_ty = self.expr_type(expr);
                let is_float = self.is_float_type(&mem_ty);
                self.gen_expr(base)?;
                let offset = self.find_member_offset(base, member);
                if offset != 0 {
                    self.emitf(&format!("add ${}, %rax", offset));
                }
                if is_float {
                    self.emit_load_float();
                    self.emitf("cvttsd2si %xmm0, %rax");
                } else {
                    self.emit_load(mem_ty.size());
                }
            }
            Expr::Cast(ty, inner, _) => {
                self.gen_expr(inner)?;
                let from_float = self.is_float_expr(inner);
                let to_float = self.is_float_type(ty);
                if from_float && !to_float {
                    self.emitf("cvttsd2si %xmm0, %rax");
                } else if !from_float && to_float {
                    self.emitf("cvtsi2sd %rax, %xmm0");
                    self.emitf("cvttsd2si %xmm0, %rax"); // also in rax
                }
            }
            Expr::InitList(_, _) => {
                // InitList as expression — handled by VarDecl
            }
            Expr::StmtExpr(stmts, last_expr, _) => {
                // Statement expression: ({ stmt; stmt; expr })
                for s in stmts {
                    self.gen_stmt(s)?;
                }
                self.gen_expr(last_expr)?;
            }
        }
        Ok(())
    }

    fn collect_cases(&mut self, stmt: &Stmt, cases: &mut Vec<(i64, String)>) {
        match stmt {
            Stmt::Case(val, body, _) => {
                let label = self.new_label();
                cases.push((*val, label));
                self.collect_cases(body, cases);
            }
            Stmt::Block(stmts, _) => {
                for s in stmts { self.collect_cases(s, cases); }
            }
            Stmt::Default(body, _) => { self.collect_cases(body, cases); }
            _ => {}
        }
    }

    fn gen_switch_body(&mut self, stmt: &Stmt, cases: &[(i64, String)], _default: &str) -> Result<(), CompileError> {
        match stmt {
            Stmt::Case(val, body, _) => {
                // Find label for this case
                for (v, label) in cases {
                    if *v == *val {
                        self.emit(&format!("{}:", label));
                        break;
                    }
                }
                self.gen_switch_body(body, cases, _default)?;
            }
            Stmt::Default(body, _) => {
                // Emit default label here so it's reachable via jmp
                self.emit(&format!("{}:", _default));
                self.gen_switch_body(body, cases, _default)?;
            }
            Stmt::Block(stmts, _) => {
                for s in stmts {
                    self.gen_switch_body(s, cases, _default)?;
                }
            }
            other => { self.gen_stmt(other)?; }
        }
        Ok(())
    }

    fn is_float_type(&self, ty: &Type) -> bool {
        matches!(ty, Type::Float | Type::Double)
    }

    fn is_float_expr(&self, expr: &Expr) -> bool {
        self.is_float_type(&self.expr_type(expr))
    }

    /// Ensure result is in rax (convert from xmm0 if needed)
    fn ensure_int(&mut self, expr: &Expr) {
        if self.is_float_expr(expr) {
            self.emitf("cvttsd2si %xmm0, %rax");
        }
    }

    /// Ensure result is in xmm0 (convert from rax if needed)
    fn ensure_float(&mut self, expr: &Expr) {
        if !self.is_float_expr(expr) {
            self.emitf("cvtsi2sd %rax, %xmm0");
        }
    }

    /// Emit float load from address in rax to xmm0
    fn emit_load_float(&mut self) {
        self.emitf("movsd (%rax), %xmm0");
    }

    /// Emit float store from xmm0 to address in rdi
    fn emit_store_float(&mut self) {
        self.emitf("movsd %xmm0, (%rdi)");
    }

    fn register_struct_type(&mut self, ty: &Type) {
        match ty {
            Type::Struct { name: Some(n), members } if !members.is_empty() => {
                self.struct_types.insert(n.clone(), ty.clone());
            }
            Type::Ptr(inner) => self.register_struct_type(inner),
            _ => {}
        }
    }

    /// Resolve struct type — if empty members, look up from known structs
    fn resolve_struct<'a>(&'a self, ty: &'a Type) -> &'a Type {
        if let Type::Struct { name: Some(n), members } = ty {
            if members.is_empty() {
                if let Some(full) = self.struct_types.get(n) {
                    return full;
                }
            }
        }
        ty
    }

    /// Find member offset and type from base expression type
    fn find_member_info(&self, base: &Expr, member: &str) -> (usize, usize) {
        let base_ty = self.expr_type(base);
        let struct_ty = match &base_ty {
            Type::Struct { .. } => self.resolve_struct(&base_ty),
            Type::Ptr(inner) => self.resolve_struct(inner.as_ref()),
            _ => return (0, 8),
        };
        if let Type::Struct { members, .. } = struct_ty {
            for m in members {
                if m.name == member {
                    return (m.offset, m.ty.size().max(1));
                }
            }
        }
        (0, 8)
    }

    fn find_member_offset(&self, base: &Expr, member: &str) -> usize {
        self.find_member_info(base, member).0
    }

    /// Emit load from address in %rax based on type size, result in %rax
    fn emit_load(&mut self, size: usize) {
        match size {
            1 => self.emitf("movsbl (%rax), %eax"),
            2 => self.emitf("movswl (%rax), %eax"),
            4 => self.emitf("movslq (%rax), %rax"),
            _ => self.emitf("mov (%rax), %rax"),
        }
    }

    /// Emit typed load — uses sign-extend or zero-extend based on type
    fn emit_load_typed(&mut self, ty: &Type) {
        if ty.is_unsigned() {
            self.emit_load_unsigned(ty.size());
        } else {
            self.emit_load(ty.size());
        }
    }

    /// Emit unsigned load (zero-extend instead of sign-extend)
    fn emit_load_unsigned(&mut self, size: usize) {
        match size {
            1 => self.emitf("movzbl (%rax), %eax"),
            2 => self.emitf("movzwl (%rax), %eax"),
            4 => self.emitf("mov (%rax), %eax"), // zero-extends to 64-bit automatically
            _ => self.emitf("mov (%rax), %rax"),
        }
    }

    /// Emit store from %rax to address in %rdi based on type size
    fn emit_store(&mut self, size: usize) {
        match size {
            1 => self.emitf("mov %al, (%rdi)"),
            2 => self.emitf("mov %ax, (%rdi)"),
            4 => self.emitf("mov %eax, (%rdi)"),
            _ => self.emitf("mov %rax, (%rdi)"),
        }
    }

    /// Infer the type of an expression
    fn expr_type(&self, expr: &Expr) -> Type {
        match expr {
            Expr::IntLit(_, _) => Type::Int,
            Expr::FloatLit(_, _) => Type::Double,
            Expr::CharLit(_, _) => Type::Char,
            Expr::StrLit(_, _) => Type::Ptr(Box::new(Type::Char)),
            Expr::Var(name, _) => {
                self.local_types.get(name).cloned().unwrap_or(Type::Long)
            }
            Expr::Binary(op, lhs, _rhs, _) => {
                match op {
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le
                    | BinOp::Gt | BinOp::Ge | BinOp::LogAnd | BinOp::LogOr => Type::Int,
                    BinOp::Assign | BinOp::AddAssign | BinOp::SubAssign
                    | BinOp::MulAssign | BinOp::DivAssign | BinOp::ModAssign
                    | BinOp::BitAndAssign | BinOp::BitOrAssign | BinOp::BitXorAssign
                    | BinOp::ShlAssign | BinOp::ShrAssign => self.expr_type(lhs),
                    _ => self.expr_type(lhs),
                }
            }
            Expr::Unary(op, operand, _) => {
                match op {
                    UnaryOp::Addr => Type::Ptr(Box::new(self.expr_type(operand))),
                    UnaryOp::Deref => {
                        match self.expr_type(operand) {
                            Type::Ptr(inner) => *inner,
                            _ => Type::Long,
                        }
                    }
                    UnaryOp::Not => Type::Int,
                    _ => self.expr_type(operand),
                }
            }
            Expr::Member(base, member, _) => {
                let base_ty = self.expr_type(base);
                let resolved = self.resolve_struct(&base_ty);
                if let Type::Struct { members, .. } = resolved {
                    for m in members {
                        if m.name == *member { return m.ty.clone(); }
                    }
                }
                Type::Long
            }
            Expr::Arrow(base, member, _) => {
                let base_ty = self.expr_type(base);
                let inner = match &base_ty {
                    Type::Ptr(inner) => inner.as_ref(),
                    _ => &base_ty,
                };
                let resolved = self.resolve_struct(inner);
                if let Type::Struct { members, .. } = resolved {
                    for m in members {
                        if m.name == *member { return m.ty.clone(); }
                    }
                }
                Type::Long
            }
            Expr::Index(base, _, _) => {
                match self.expr_type(base) {
                    Type::Ptr(inner) | Type::Array(inner, _) => *inner,
                    _ => Type::Long,
                }
            }
            Expr::Call(func, _, _) => {
                if let Expr::Var(name, _) = func.as_ref() {
                    self.func_return_types.get(name).cloned().unwrap_or(Type::Int)
                } else {
                    Type::Int
                }
            }
            Expr::Cond(_, then, _, _) => self.expr_type(then),
            Expr::Sizeof(_, _) => Type::Long,
            Expr::Cast(ty, _, _) => ty.clone(),
            Expr::InitList(_, _) => Type::Int,
            Expr::StmtExpr(_, last, _) => self.expr_type(last),
        }
    }

    /// Get element size for array/pointer indexing
    fn index_stride(&self, base: &Expr) -> usize {
        match self.expr_type(base) {
            Type::Ptr(inner) | Type::Array(inner, _) => inner.size().max(1),
            _ => 8,
        }
    }

    fn gen_addr(&mut self, expr: &Expr) -> Result<(), CompileError> {
        match expr {
            Expr::Var(name, _) => {
                if let Some(&offset) = self.locals.get(name) {
                    self.emitf(&format!("lea {}(%rbp), %rdi", offset));
                } else {
                    self.emitf(&format!("lea {}(%rip), %rdi", name));
                }
                Ok(())
            }
            Expr::Unary(UnaryOp::Deref, inner, _) => {
                self.gen_expr(inner)?;
                self.emitf("mov %rax, %rdi");
                Ok(())
            }
            Expr::Index(array, index, _) => {
                let stride = self.index_stride(array);
                self.gen_expr(index)?;
                self.emitf(&format!("imul ${}, %rax", stride));
                self.emitf("push %rax");
                self.gen_expr(array)?;
                self.emitf("pop %rcx");
                self.emitf("add %rcx, %rax");
                self.emitf("mov %rax, %rdi");
                Ok(())
            }
            Expr::Member(base, member, _) => {
                self.gen_addr(base)?;
                let offset = self.find_member_offset(base, member);
                if offset != 0 {
                    self.emitf(&format!("add ${}, %rdi", offset));
                }
                Ok(())
            }
            Expr::Arrow(base, member, _) => {
                self.gen_expr(base)?;
                self.emitf("mov %rax, %rdi");
                let offset = self.find_member_offset(base, member);
                if offset != 0 {
                    self.emitf(&format!("add ${}, %rdi", offset));
                }
                Ok(())
            }
            _ => {
                // For complex expressions (assign, ternary, call),
                // evaluate and store to a temp, return temp address
                self.gen_expr(expr)?;
                // Store rax to temp on stack
                self.stack_size += 8;
                let tmp_off = -self.stack_size;
                self.emitf(&format!("mov %rax, {}(%rbp)", tmp_off));
                self.emitf(&format!("lea {}(%rbp), %rdi", tmp_off));
                Ok(())
            }
        }
    }
}

fn count_call_args(stmt: &Stmt) -> usize {
    match stmt {
        Stmt::Expr(e, _) | Stmt::Return(Some(e), _) => count_expr_args(e),
        Stmt::Block(stmts, _) => stmts.iter().map(count_call_args).sum(),
        Stmt::VarDecl(d, _) => d.init.as_ref().map_or(0, count_expr_args),
        Stmt::If(c, t, e, _) => count_expr_args(c) + count_call_args(t) + e.as_ref().map_or(0, |e| count_call_args(e)),
        Stmt::While(c, b, _) | Stmt::DoWhile(b, c, _) => count_expr_args(c) + count_call_args(b),
        Stmt::For(i, c, inc, b, _) => {
            i.as_ref().map_or(0, |s| count_call_args(s))
            + c.as_ref().map_or(0, count_expr_args)
            + inc.as_ref().map_or(0, count_expr_args)
            + count_call_args(b)
        }
        Stmt::Switch(c, b, _) => count_expr_args(c) + count_call_args(b),
        Stmt::Case(_, b, _) | Stmt::Default(b, _) | Stmt::Label(_, b, _) => count_call_args(b),
        _ => 0,
    }
}

fn count_expr_args(expr: &Expr) -> usize {
    match expr {
        Expr::Call(_, args, _) => {
            args.len() + 1 + args.iter().map(count_expr_args).sum::<usize>()
        }
        Expr::Binary(_, l, r, _) => count_expr_args(l) + count_expr_args(r),
        Expr::Unary(_, e, _) => count_expr_args(e),
        Expr::Cond(c, t, e, _) => count_expr_args(c) + count_expr_args(t) + count_expr_args(e),
        Expr::Index(a, i, _) => count_expr_args(a) + count_expr_args(i),
        Expr::Member(b, _, _) | Expr::Arrow(b, _, _) => count_expr_args(b),
        _ => 0,
    }
}

fn has_default(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Default(_, _) => true,
        Stmt::Block(stmts, _) => stmts.iter().any(has_default),
        Stmt::Case(_, body, _) => has_default(body),
        _ => false,
    }
}

fn count_flat_init(items: &[Expr]) -> usize {
    items.iter().map(|item| {
        if let Expr::InitList(inner, _) = item { count_flat_init(inner) } else { 1 }
    }).sum()
}

fn flatten_init_list(items: &[Expr]) -> Vec<&Expr> {
    let mut flat = Vec::new();
    for item in items {
        if let Expr::InitList(inner, _) = item {
            flat.extend(flatten_init_list(inner));
        } else {
            flat.push(item);
        }
    }
    flat
}

pub fn generate(program: &TranslationUnit) -> Result<String, CompileError> {
    let mut g = Generator::new();
    g.gen_program(program)?;
    Ok(g.output)
}
