/// x86-64 Windows backend: IR → assembly
///
/// With optional register allocation: frequently-used vregs
/// are placed in callee-saved registers (rbx, r12-r15).

use std::fmt::Write;
use crate::ir::*;
use crate::regalloc::{self, Location, RegAllocResult};

#[cfg(target_os = "windows")]
const IR_ARG_REGS: &[&str] = &["%rcx", "%rdx", "%r8", "%r9"];
#[cfg(not(target_os = "windows"))]
const IR_ARG_REGS: &[&str] = &["%rdi", "%rsi", "%rdx", "%rcx", "%r8", "%r9"];

pub fn emit_x64(module: &IrModule) -> String {
    let mut out = String::new();

    // Globals
    for g in &module.globals {
        if let Some(init) = &g.init {
            writeln!(out, "  .data").unwrap();
            writeln!(out, "  .globl {}", g.name).unwrap();
            writeln!(out, "{}:", g.name).unwrap();
            // Emit bytes
            for chunk in init.chunks(8) {
                if chunk.len() == 8 {
                    let val = i64::from_le_bytes(chunk.try_into().unwrap());
                    writeln!(out, "  .quad {}", val).unwrap();
                } else {
                    for b in chunk {
                        writeln!(out, "  .byte {}", b).unwrap();
                    }
                }
            }
        } else {
            writeln!(out, "  .bss").unwrap();
            writeln!(out, "  .globl {}", g.name).unwrap();
            writeln!(out, "{}:", g.name).unwrap();
            writeln!(out, "  .zero {}", g.size).unwrap();
        }
    }
    writeln!(out, "  .text").unwrap();

    // Data section (string literals)
    if !module.data.is_empty() {
        writeln!(out, "  .section .rodata").unwrap();
        for d in &module.data {
            writeln!(out, ".LD{}:", d.label).unwrap();
            // Emit as .byte directives for correctness
            write!(out, "  .byte ").unwrap();
            for (i, b) in d.bytes.iter().enumerate() {
                if i > 0 { write!(out, ",").unwrap(); }
                write!(out, "{}", b).unwrap();
            }
            writeln!(out).unwrap();
        }
        writeln!(out, "  .text").unwrap();
    }

    // Functions
    for func in &module.functions {
        emit_function(&mut out, func);
    }

    out
}

fn emit_function(out: &mut String, func: &IrFunction) {
    // Run register allocator
    let ra = regalloc::allocate(func);

    // Count total alloca space needed
    let mut alloca_space = 0usize;
    for bb in &func.blocks {
        for inst in &bb.insts {
            if let Inst::Alloca(_, size) = inst {
                alloca_space += *size;
            }
        }
    }

    let stack_slots = func.next_vreg as usize;
    let vreg_space = stack_slots * 8;
    let total = vreg_space + alloca_space + ra.stack_size + 32;
    let frame_size = ((total + 15) / 16) * 16;
    let frame_size = frame_size.max(64);
    let alloca_base = vreg_space;

    writeln!(out, "  .globl {}", func.name).unwrap();
    writeln!(out, "{}:", func.name).unwrap();
    writeln!(out, "  push %rbp").unwrap();
    writeln!(out, "  mov %rsp, %rbp").unwrap();

    writeln!(out, "  sub ${}, %rsp", frame_size).unwrap();

    // Save callee-saved registers used by regalloc (store into frame, not push)
    let save_base = -(frame_size as i64) + 8; // use end of frame
    for (i, reg) in ra.used_callee_saved.iter().enumerate() {
        writeln!(out, "  mov {}, {}(%rbp)", reg, save_base + (i as i64) * 8).unwrap();
    }

    // Store params
    for (i, (vreg, _ty)) in func.params.iter().enumerate() {
        if i < 4 {
            let offset = vreg_offset(*vreg);
            writeln!(out, "  mov {}, {}(%rbp)", IR_ARG_REGS[i], offset).unwrap();
        }
    }

    // Track alloca offset
    let mut alloca_offset = alloca_base;

    // Emit blocks
    for bb in &func.blocks {
        writeln!(out, ".LB{}_{}:", func.name, bb.label).unwrap();
        for inst in &bb.insts {
            emit_inst_ra(out, inst, &mut alloca_offset, &ra);
        }
        emit_term_ra(out, &bb.term, &func.name, &ra, frame_size);
    }
}

fn vreg_offset(v: VReg) -> i64 {
    -((v as i64 + 1) * 8)
}

/// Load vreg into target register. If vreg is allocated to a physical register,
/// emit mov reg→target (or nothing if same). Otherwise load from stack.
fn load_vreg_ra(out: &mut String, target: &str, v: VReg, ra: &RegAllocResult) {
    if let Some(loc) = ra.assignments.get(&v) {
        match loc {
            Location::Reg(r) => {
                if *r != target {
                    writeln!(out, "  mov {}, {}", r, target).unwrap();
                }
            }
            Location::Stack(off) => {
                writeln!(out, "  mov {}(%rbp), {}", off, target).unwrap();
            }
        }
    } else {
        writeln!(out, "  mov {}(%rbp), {}", vreg_offset(v), target).unwrap();
    }
}

/// Store register value into vreg. If vreg is allocated to a physical register,
/// emit mov source→reg. Otherwise store to stack.
fn store_vreg_ra(out: &mut String, source: &str, v: VReg, ra: &RegAllocResult) {
    if let Some(loc) = ra.assignments.get(&v) {
        match loc {
            Location::Reg(r) => {
                if *r != source {
                    writeln!(out, "  mov {}, {}", source, r).unwrap();
                }
            }
            Location::Stack(off) => {
                writeln!(out, "  mov {}, {}(%rbp)", source, off).unwrap();
            }
        }
    } else {
        writeln!(out, "  mov {}, {}(%rbp)", source, vreg_offset(v)).unwrap();
    }
}

// Fallback without regalloc (for backwards compat)
fn load_vreg(out: &mut String, reg: &str, v: VReg) {
    writeln!(out, "  mov {}(%rbp), {}", vreg_offset(v), reg).unwrap();
}

fn store_vreg(out: &mut String, reg: &str, v: VReg) {
    writeln!(out, "  mov {}, {}(%rbp)", reg, vreg_offset(v)).unwrap();
}

fn emit_inst_ra(out: &mut String, inst: &Inst, alloca_offset: &mut usize, ra: &RegAllocResult) {
    // Wrapper that uses register-allocated load/store
    match inst {
        Inst::Const(dst, val, _ty) => {
            writeln!(out, "  mov ${}, %rax", val).unwrap();
            store_vreg_ra(out, "%rax", *dst, ra);
        }
        Inst::Alloca(dst, size) => {
            let offset = -(*alloca_offset as i64 + *size as i64);
            *alloca_offset += *size;
            writeln!(out, "  lea {}(%rbp), %rax", offset).unwrap();
            store_vreg_ra(out, "%rax", *dst, ra);
        }
        Inst::Load(dst, addr, ty) => {
            load_vreg_ra(out, "%rax", *addr, ra);
            match ty.size() {
                1 => writeln!(out, "  movsbl (%rax), %eax").unwrap(),
                2 => writeln!(out, "  movswl (%rax), %eax").unwrap(),
                4 => writeln!(out, "  movslq (%rax), %rax").unwrap(),
                _ => writeln!(out, "  mov (%rax), %rax").unwrap(),
            }
            store_vreg_ra(out, "%rax", *dst, ra);
        }
        Inst::Store(val, addr, ty) => {
            load_vreg_ra(out, "%rax", *val, ra);
            load_vreg_ra(out, "%rdi", *addr, ra);
            match ty.size() {
                1 => writeln!(out, "  mov %al, (%rdi)").unwrap(),
                2 => writeln!(out, "  mov %ax, (%rdi)").unwrap(),
                4 => writeln!(out, "  mov %eax, (%rdi)").unwrap(),
                _ => writeln!(out, "  mov %rax, (%rdi)").unwrap(),
            }
        }
        Inst::BinOp(dst, op, lhs, rhs, _ty) => {
            load_vreg_ra(out, "%rax", *lhs, ra);
            load_vreg_ra(out, "%rcx", *rhs, ra);
            match op {
                BinIrOp::Add => writeln!(out, "  add %rcx, %rax").unwrap(),
                BinIrOp::Sub => writeln!(out, "  sub %rcx, %rax").unwrap(),
                BinIrOp::Mul => writeln!(out, "  imul %rcx, %rax").unwrap(),
                BinIrOp::Div => { writeln!(out, "  cqo").unwrap(); writeln!(out, "  idiv %rcx").unwrap(); }
                BinIrOp::Mod => { writeln!(out, "  cqo").unwrap(); writeln!(out, "  idiv %rcx").unwrap(); writeln!(out, "  mov %rdx, %rax").unwrap(); }
                BinIrOp::And => writeln!(out, "  and %rcx, %rax").unwrap(),
                BinIrOp::Or => writeln!(out, "  or %rcx, %rax").unwrap(),
                BinIrOp::Xor => writeln!(out, "  xor %rcx, %rax").unwrap(),
                BinIrOp::Shl => writeln!(out, "  shl %cl, %rax").unwrap(),
                BinIrOp::Shr => writeln!(out, "  sar %cl, %rax").unwrap(),
            }
            store_vreg_ra(out, "%rax", *dst, ra);
        }
        Inst::UnOp(dst, op, src, _ty) => {
            load_vreg_ra(out, "%rax", *src, ra);
            match op {
                UnIrOp::Neg => writeln!(out, "  neg %rax").unwrap(),
                UnIrOp::Not => {
                    writeln!(out, "  cmp $0, %rax").unwrap();
                    writeln!(out, "  sete %al").unwrap();
                    writeln!(out, "  movzbl %al, %eax").unwrap();
                }
                UnIrOp::BitNot => writeln!(out, "  not %rax").unwrap(),
            }
            store_vreg_ra(out, "%rax", *dst, ra);
        }
        Inst::Cmp(dst, op, lhs, rhs, _ty) => {
            load_vreg_ra(out, "%rax", *lhs, ra);
            load_vreg_ra(out, "%rcx", *rhs, ra);
            writeln!(out, "  cmp %rcx, %rax").unwrap();
            let set = match op {
                CmpOp::Eq => "sete", CmpOp::Ne => "setne",
                CmpOp::Lt => "setl", CmpOp::Le => "setle",
                CmpOp::Gt => "setg", CmpOp::Ge => "setge",
            };
            writeln!(out, "  {} %al", set).unwrap();
            writeln!(out, "  movzbl %al, %eax").unwrap();
            store_vreg_ra(out, "%rax", *dst, ra);
        }
        Inst::Call(dst, name, args) => {
            for (i, arg) in args.iter().enumerate() {
                load_vreg_ra(out, "%rax", *arg, ra);
                if i < 4 {
                    writeln!(out, "  mov %rax, {}", IR_ARG_REGS[i]).unwrap();
                } else {
                    writeln!(out, "  mov %rax, {}(%rsp)", 32 + (i - 4) * 8).unwrap();
                }
            }
            writeln!(out, "  call {}", name).unwrap();
            store_vreg_ra(out, "%rax", *dst, ra);
        }
        // Delegate remaining to original emit_inst
        other => emit_inst(out, other, alloca_offset),
    }
}

fn emit_term_ra(out: &mut String, term: &Terminator, fn_name: &str, ra: &RegAllocResult, frame_size: usize) {
    match term {
        Terminator::Ret(val) => {
            if let Some(v) = val {
                load_vreg_ra(out, "%rax", *v, ra);
            }
            // Restore callee-saved registers before leaving frame
            let save_base = -(frame_size as i64) + 8;
            for (i, reg) in ra.used_callee_saved.iter().enumerate() {
                writeln!(out, "  mov {}(%rbp), {}", save_base + (i as i64) * 8, reg).unwrap();
            }
            writeln!(out, "  mov %rbp, %rsp").unwrap();
            writeln!(out, "  pop %rbp").unwrap();
            writeln!(out, "  ret").unwrap();
        }
        Terminator::Jump(label) => {
            writeln!(out, "  jmp .LB{}_{}", fn_name, label).unwrap();
        }
        Terminator::Branch(cond, true_l, false_l) => {
            load_vreg_ra(out, "%rax", *cond, ra);
            writeln!(out, "  cmp $0, %rax").unwrap();
            writeln!(out, "  jne .LB{}_{}", fn_name, true_l).unwrap();
            writeln!(out, "  jmp .LB{}_{}", fn_name, false_l).unwrap();
        }
    }
}

fn emit_inst(out: &mut String, inst: &Inst, alloca_offset: &mut usize) {
    match inst {
        Inst::Const(dst, val, _ty) => {
            writeln!(out, "  mov ${}, %rax", val).unwrap();
            store_vreg(out, "%rax", *dst);
        }
        Inst::Alloca(dst, size) => {
            // Alloca: point to separate memory region on stack
            let offset = -(*alloca_offset as i64 + *size as i64);
            *alloca_offset += *size;
            writeln!(out, "  lea {}(%rbp), %rax", offset).unwrap();
            store_vreg(out, "%rax", *dst);
        }
        Inst::Load(dst, addr, ty) => {
            load_vreg(out, "%rax", *addr); // rax = address
            match ty.size() {
                1 => writeln!(out, "  movsbl (%rax), %eax").unwrap(),
                2 => writeln!(out, "  movswl (%rax), %eax").unwrap(),
                4 => writeln!(out, "  movslq (%rax), %rax").unwrap(),
                _ => writeln!(out, "  mov (%rax), %rax").unwrap(),
            }
            store_vreg(out, "%rax", *dst);
        }
        Inst::Store(val, addr, ty) => {
            load_vreg(out, "%rax", *val);  // rax = value
            load_vreg(out, "%rdi", *addr); // rdi = address
            match ty.size() {
                1 => writeln!(out, "  mov %al, (%rdi)").unwrap(),
                2 => writeln!(out, "  mov %ax, (%rdi)").unwrap(),
                4 => writeln!(out, "  mov %eax, (%rdi)").unwrap(),
                _ => writeln!(out, "  mov %rax, (%rdi)").unwrap(),
            }
        }
        Inst::BinOp(dst, op, lhs, rhs, _ty) => {
            load_vreg(out, "%rax", *lhs);
            load_vreg(out, "%rcx", *rhs);
            match op {
                BinIrOp::Add => writeln!(out, "  add %rcx, %rax").unwrap(),
                BinIrOp::Sub => writeln!(out, "  sub %rcx, %rax").unwrap(),
                BinIrOp::Mul => writeln!(out, "  imul %rcx, %rax").unwrap(),
                BinIrOp::Div => { writeln!(out, "  cqo").unwrap(); writeln!(out, "  idiv %rcx").unwrap(); }
                BinIrOp::Mod => { writeln!(out, "  cqo").unwrap(); writeln!(out, "  idiv %rcx").unwrap(); writeln!(out, "  mov %rdx, %rax").unwrap(); }
                BinIrOp::And => writeln!(out, "  and %rcx, %rax").unwrap(),
                BinIrOp::Or => writeln!(out, "  or %rcx, %rax").unwrap(),
                BinIrOp::Xor => writeln!(out, "  xor %rcx, %rax").unwrap(),
                BinIrOp::Shl => writeln!(out, "  shl %cl, %rax").unwrap(),
                BinIrOp::Shr => writeln!(out, "  sar %cl, %rax").unwrap(),
            }
            store_vreg(out, "%rax", *dst);
        }
        Inst::UnOp(dst, op, src, _ty) => {
            load_vreg(out, "%rax", *src);
            match op {
                UnIrOp::Neg => writeln!(out, "  neg %rax").unwrap(),
                UnIrOp::Not => {
                    writeln!(out, "  cmp $0, %rax").unwrap();
                    writeln!(out, "  sete %al").unwrap();
                    writeln!(out, "  movzbl %al, %eax").unwrap();
                }
                UnIrOp::BitNot => writeln!(out, "  not %rax").unwrap(),
            }
            store_vreg(out, "%rax", *dst);
        }
        Inst::Cmp(dst, op, lhs, rhs, _ty) => {
            load_vreg(out, "%rax", *lhs);
            load_vreg(out, "%rcx", *rhs);
            writeln!(out, "  cmp %rcx, %rax").unwrap();
            let set = match op {
                CmpOp::Eq => "sete", CmpOp::Ne => "setne",
                CmpOp::Lt => "setl", CmpOp::Le => "setle",
                CmpOp::Gt => "setg", CmpOp::Ge => "setge",
            };
            writeln!(out, "  {} %al", set).unwrap();
            writeln!(out, "  movzbl %al, %eax").unwrap();
            store_vreg(out, "%rax", *dst);
        }
        Inst::Call(dst, name, args) => {
            // Push args
            for (i, arg) in args.iter().enumerate() {
                load_vreg(out, "%rax", *arg);
                if i < 4 {
                    writeln!(out, "  mov %rax, {}", IR_ARG_REGS[i]).unwrap();
                } else {
                    // Stack arg — push after shadow space
                    writeln!(out, "  mov %rax, {}(%rsp)", 32 + (i - 4) * 8).unwrap();
                }
            }
            writeln!(out, "  call {}", name).unwrap();
            store_vreg(out, "%rax", *dst);
        }
        Inst::CallIndirect(dst, fptr, args) => {
            for (i, arg) in args.iter().enumerate() {
                load_vreg(out, "%rax", *arg);
                if i < 4 {
                    writeln!(out, "  mov %rax, {}", IR_ARG_REGS[i]).unwrap();
                }
            }
            load_vreg(out, "%rax", *fptr);
            writeln!(out, "  call *%rax").unwrap();
            store_vreg(out, "%rax", *dst);
        }
        Inst::LeaGlobal(dst, name) => {
            writeln!(out, "  lea {}(%rip), %rax", name).unwrap();
            store_vreg(out, "%rax", *dst);
        }
        Inst::LeaLabel(dst, label) => {
            writeln!(out, "  lea .LD{}(%rip), %rax", label).unwrap();
            store_vreg(out, "%rax", *dst);
        }
        Inst::GetElementPtr(dst, base, idx, stride) => {
            load_vreg(out, "%rax", *idx);
            writeln!(out, "  imul ${}, %rax", stride).unwrap();
            writeln!(out, "  push %rax").unwrap();
            load_vreg(out, "%rax", *base);
            writeln!(out, "  pop %rcx").unwrap();
            writeln!(out, "  add %rcx, %rax").unwrap();
            store_vreg(out, "%rax", *dst);
        }
        Inst::AddImm(dst, base, offset) => {
            load_vreg(out, "%rax", *base);
            if *offset != 0 {
                writeln!(out, "  add ${}, %rax", offset).unwrap();
            }
            store_vreg(out, "%rax", *dst);
        }
        Inst::Ext(dst, src, _from, _to) | Inst::Trunc(dst, src, _from, _to) => {
            load_vreg(out, "%rax", *src);
            store_vreg(out, "%rax", *dst);
        }
        Inst::Phi(dst, _args) => {
            // Phi nodes are resolved during SSA destruction — for now, treat as nop
            // The backend should insert mov instructions at predecessor block ends
            writeln!(out, "  # phi %{}", dst).unwrap();
        }
        Inst::Comment(s) => {
            writeln!(out, "  # {}", s).unwrap();
        }
    }
}

fn emit_term(out: &mut String, term: &Terminator, fn_name: &str, _callee_saved: &[&str]) {
    match term {
        Terminator::Ret(val) => {
            if let Some(v) = val {
                load_vreg(out, "%rax", *v);
            }
            writeln!(out, "  mov %rbp, %rsp").unwrap();
            writeln!(out, "  pop %rbp").unwrap();
            writeln!(out, "  ret").unwrap();
        }
        Terminator::Jump(label) => {
            writeln!(out, "  jmp .LB{}_{}", fn_name, label).unwrap();
        }
        Terminator::Branch(cond, true_l, false_l) => {
            load_vreg(out, "%rax", *cond);
            writeln!(out, "  cmp $0, %rax").unwrap();
            writeln!(out, "  jne .LB{}_{}", fn_name, true_l).unwrap();
            writeln!(out, "  jmp .LB{}_{}", fn_name, false_l).unwrap();
        }
    }
}
