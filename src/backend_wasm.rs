/// WASM backend: IR → WAT (WebAssembly Text format)
///
/// Generates WAT that can be compiled to .wasm with wat2wasm.
/// Uses WASM linear memory for stack simulation.
/// External functions (printf etc) are imported from "env" module.
///
/// Limitations:
/// - Control flow (branches, loops) uses fall-through only.
///   WASM requires structured control flow (block/loop/br_table).
///   A proper relooping algorithm is needed for full support.
/// - Works correctly for linear code and simple returns.

use std::collections::HashSet;
use std::fmt::Write;
use crate::ir::*;

/// Memory layout:
/// 0x0000 - 0x0FFF: data section (strings, globals)
/// 0x1000 - ...:    stack (grows upward, SP in global)
const DATA_BASE: u32 = 0;
const STACK_BASE: u32 = 0x10000; // 64KB for data

pub fn emit_wat(module: &IrModule) -> String {
    let mut out = String::new();
    writeln!(out, "(module").unwrap();

    // Memory: 2 pages (128KB)
    writeln!(out, "  (memory (export \"memory\") 2)").unwrap();

    // Stack pointer global
    writeln!(out, "  (global $sp (mut i32) (i32.const {}))", STACK_BASE).unwrap();

    // Collect external function names (functions called but not defined)
    let defined: HashSet<&str> = module.functions.iter().map(|f| f.name.as_str()).collect();
    let mut imports = HashSet::new();
    for func in &module.functions {
        for bb in &func.blocks {
            for inst in &bb.insts {
                if let Inst::Call(_, name, args) = inst {
                    if !defined.contains(name.as_str()) {
                        imports.insert((name.clone(), args.len()));
                    }
                }
            }
        }
    }

    // Import external functions
    for (name, nargs) in &imports {
        let params = (0..*nargs).map(|_| "i64").collect::<Vec<_>>().join(" ");
        let params_wat = if params.is_empty() {
            String::new()
        } else {
            format!(" (param {})", params)
        };
        writeln!(out, "  (import \"env\" \"{}\" (func ${}{} (result i64)))", name, name, params_wat).unwrap();
    }

    // Data section — string literals
    let mut data_offset = DATA_BASE;
    let mut data_offsets: std::collections::HashMap<Label, u32> = std::collections::HashMap::new();
    for d in &module.data {
        data_offsets.insert(d.label, data_offset);
        let escaped = d.bytes.iter().map(|b| format!("\\{:02x}", b)).collect::<String>();
        writeln!(out, "  (data (i32.const {}) \"{}\")", data_offset, escaped).unwrap();
        data_offset += d.bytes.len() as u32;
    }

    // Global variables
    let mut global_offsets: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for g in &module.globals {
        global_offsets.insert(g.name.clone(), data_offset);
        if let Some(init) = &g.init {
            let escaped = init.iter().map(|b| format!("\\{:02x}", b)).collect::<String>();
            writeln!(out, "  (data (i32.const {}) \"{}\")", data_offset, escaped).unwrap();
        }
        data_offset += g.size as u32;
    }

    // Functions
    for func in &module.functions {
        emit_wat_function(&mut out, func, &data_offsets, &global_offsets, &imports);
    }

    // Export main
    if defined.contains("main") {
        writeln!(out, "  (export \"main\" (func $main))").unwrap();
    }

    writeln!(out, ")").unwrap();
    out
}

fn emit_wat_function(
    out: &mut String,
    func: &IrFunction,
    data_offsets: &std::collections::HashMap<Label, u32>,
    global_offsets: &std::collections::HashMap<String, u32>,
    imports: &HashSet<(String, usize)>,
) {
    let nparams = func.params.len();
    let nlocals = func.next_vreg as usize;

    let params_str = (0..nparams).map(|i| format!("(param $p{} i64)", i)).collect::<Vec<_>>().join(" ");
    let result_str = if func.return_ty == IrType::Void { "" } else { "(result i64)" };

    write!(out, "  (func ${} {} {}", func.name, params_str, result_str).unwrap();
    writeln!(out).unwrap();

    // Locals: one i64 per vreg + frame pointer
    writeln!(out, "    (local $fp i32)").unwrap();
    for i in 0..nlocals {
        writeln!(out, "    (local $v{} i64)", i).unwrap();
    }

    // Allocate stack frame
    let frame_size = nlocals * 8 + 64; // extra space for allocas
    writeln!(out, "    ;; allocate frame").unwrap();
    writeln!(out, "    global.get $sp").unwrap();
    writeln!(out, "    local.set $fp").unwrap();
    writeln!(out, "    global.get $sp").unwrap();
    writeln!(out, "    i32.const {}", frame_size).unwrap();
    writeln!(out, "    i32.add").unwrap();
    writeln!(out, "    global.set $sp").unwrap();

    // Store params into locals
    for (i, (vreg, _)) in func.params.iter().enumerate() {
        writeln!(out, "    local.get $p{}", i).unwrap();
        writeln!(out, "    local.set $v{}", vreg).unwrap();
    }

    // Emit blocks using block/loop/br structure
    // Simplified: emit as a flat sequence with br_table for jumps
    // For now, use a simple approach: each basic block is a block, use br for jumps

    // We'll use a more pragmatic approach: emit all instructions linearly
    // with block labels mapped to WASM block nesting.
    // This is a simplified approach that works for simple control flow.

    // State variable for control flow dispatch
    writeln!(out, "    (local $state i32)").unwrap();
    writeln!(out, "    i32.const 0").unwrap();
    writeln!(out, "    local.set $state").unwrap();

    // Main dispatch loop
    writeln!(out, "    (block $exit").unwrap();
    writeln!(out, "    (loop $dispatch").unwrap();

    // Generate a block for each BB
    let nblocks = func.blocks.len();
    for (idx, _) in func.blocks.iter().enumerate() {
        writeln!(out, "    (block $bb{}", idx).unwrap();
    }

    // br_table dispatch: jump to correct block based on $state
    writeln!(out, "      local.get $state").unwrap();
    let targets: Vec<String> = (0..nblocks).map(|i| format!("$bb{}", i)).collect();
    writeln!(out, "      br_table {} $exit", targets.join(" ")).unwrap();

    // Close blocks and emit code (in reverse order due to br_table semantics)
    for (idx, bb) in func.blocks.iter().enumerate().rev() {
        writeln!(out, "    ) ;; end $bb{}", idx).unwrap();
        writeln!(out, "    ;; BB {} (label {})", idx, bb.label).unwrap();
        for inst in &bb.insts {
            emit_wat_inst(out, inst, data_offsets, global_offsets, imports);
        }
        // Emit terminator
        match &bb.term {
            Terminator::Ret(val) => {
                if let Some(v) = val {
                    writeln!(out, "      local.get $v{}", v).unwrap();
                    writeln!(out, "      local.get $fp").unwrap();
                    writeln!(out, "      global.set $sp").unwrap();
                    writeln!(out, "      return").unwrap();
                } else {
                    writeln!(out, "      local.get $fp").unwrap();
                    writeln!(out, "      global.set $sp").unwrap();
                    writeln!(out, "      return").unwrap();
                }
            }
            Terminator::Jump(target) => {
                // Find target BB index
                let target_idx = func.blocks.iter().position(|b| b.label == *target).unwrap_or(0);
                writeln!(out, "      i32.const {}", target_idx).unwrap();
                writeln!(out, "      local.set $state").unwrap();
                writeln!(out, "      br $dispatch").unwrap();
            }
            Terminator::Branch(cond, true_l, false_l) => {
                let true_idx = func.blocks.iter().position(|b| b.label == *true_l).unwrap_or(0);
                let false_idx = func.blocks.iter().position(|b| b.label == *false_l).unwrap_or(0);
                writeln!(out, "      local.get $v{}", cond).unwrap();
                writeln!(out, "      i64.const 0").unwrap();
                writeln!(out, "      i64.ne").unwrap();
                writeln!(out, "      if (result i32)").unwrap();
                writeln!(out, "        i32.const {}", true_idx).unwrap();
                writeln!(out, "      else").unwrap();
                writeln!(out, "        i32.const {}", false_idx).unwrap();
                writeln!(out, "      end").unwrap();
                writeln!(out, "      local.set $state").unwrap();
                writeln!(out, "      br $dispatch").unwrap();
            }
        }
    }

    writeln!(out, "    ) ;; end $dispatch").unwrap();
    writeln!(out, "    ) ;; end $exit").unwrap();

    // Default return
    if func.return_ty != IrType::Void {
        writeln!(out, "    i64.const 0").unwrap();
    }

    // Deallocate frame
    writeln!(out, "    local.get $fp").unwrap();
    writeln!(out, "    global.set $sp").unwrap();

    writeln!(out, "  )").unwrap();
}

fn emit_wat_inst(
    out: &mut String,
    inst: &Inst,
    data_offsets: &std::collections::HashMap<Label, u32>,
    global_offsets: &std::collections::HashMap<String, u32>,
    _imports: &HashSet<(String, usize)>,
) {
    match inst {
        Inst::Const(dst, val, _) => {
            writeln!(out, "      i64.const {}", val).unwrap();
            writeln!(out, "      local.set $v{}", dst).unwrap();
        }
        Inst::Alloca(dst, _size) => {
            // Alloca: allocate from frame, return address
            writeln!(out, "      local.get $fp").unwrap();
            writeln!(out, "      i64.extend_i32_u").unwrap();
            writeln!(out, "      local.set $v{}", dst).unwrap();
            // Bump fp for next alloca
            writeln!(out, "      local.get $fp").unwrap();
            writeln!(out, "      i32.const 8").unwrap();
            writeln!(out, "      i32.add").unwrap();
            writeln!(out, "      local.set $fp").unwrap();
        }
        Inst::Load(dst, addr, ty) => {
            writeln!(out, "      local.get $v{}", addr).unwrap();
            writeln!(out, "      i32.wrap_i64").unwrap();
            match ty.size() {
                1 => writeln!(out, "      i64.load8_s").unwrap(),
                2 => writeln!(out, "      i64.load16_s").unwrap(),
                4 => writeln!(out, "      i64.load32_s").unwrap(),
                _ => writeln!(out, "      i64.load").unwrap(),
            }
            writeln!(out, "      local.set $v{}", dst).unwrap();
        }
        Inst::Store(val, addr, ty) => {
            writeln!(out, "      local.get $v{}", addr).unwrap();
            writeln!(out, "      i32.wrap_i64").unwrap();
            writeln!(out, "      local.get $v{}", val).unwrap();
            match ty.size() {
                1 => writeln!(out, "      i64.store8").unwrap(),
                2 => writeln!(out, "      i64.store16").unwrap(),
                4 => writeln!(out, "      i64.store32").unwrap(),
                _ => writeln!(out, "      i64.store").unwrap(),
            }
        }
        Inst::BinOp(dst, op, lhs, rhs, _) => {
            writeln!(out, "      local.get $v{}", lhs).unwrap();
            writeln!(out, "      local.get $v{}", rhs).unwrap();
            match op {
                BinIrOp::Add => writeln!(out, "      i64.add").unwrap(),
                BinIrOp::Sub => writeln!(out, "      i64.sub").unwrap(),
                BinIrOp::Mul => writeln!(out, "      i64.mul").unwrap(),
                BinIrOp::Div => writeln!(out, "      i64.div_s").unwrap(),
                BinIrOp::Mod => writeln!(out, "      i64.rem_s").unwrap(),
                BinIrOp::And => writeln!(out, "      i64.and").unwrap(),
                BinIrOp::Or => writeln!(out, "      i64.or").unwrap(),
                BinIrOp::Xor => writeln!(out, "      i64.xor").unwrap(),
                BinIrOp::Shl => writeln!(out, "      i64.shl").unwrap(),
                BinIrOp::Shr => writeln!(out, "      i64.shr_s").unwrap(),
            }
            writeln!(out, "      local.set $v{}", dst).unwrap();
        }
        Inst::UnOp(dst, op, src, _) => {
            match op {
                UnIrOp::Neg => {
                    writeln!(out, "      i64.const 0").unwrap();
                    writeln!(out, "      local.get $v{}", src).unwrap();
                    writeln!(out, "      i64.sub").unwrap();
                }
                UnIrOp::Not => {
                    writeln!(out, "      local.get $v{}", src).unwrap();
                    writeln!(out, "      i64.eqz").unwrap();
                    writeln!(out, "      i64.extend_i32_u").unwrap();
                }
                UnIrOp::BitNot => {
                    writeln!(out, "      i64.const -1").unwrap();
                    writeln!(out, "      local.get $v{}", src).unwrap();
                    writeln!(out, "      i64.xor").unwrap();
                }
            }
            writeln!(out, "      local.set $v{}", dst).unwrap();
        }
        Inst::Cmp(dst, op, lhs, rhs, _) => {
            writeln!(out, "      local.get $v{}", lhs).unwrap();
            writeln!(out, "      local.get $v{}", rhs).unwrap();
            match op {
                CmpOp::Eq => writeln!(out, "      i64.eq").unwrap(),
                CmpOp::Ne => writeln!(out, "      i64.ne").unwrap(),
                CmpOp::Lt => writeln!(out, "      i64.lt_s").unwrap(),
                CmpOp::Le => writeln!(out, "      i64.le_s").unwrap(),
                CmpOp::Gt => writeln!(out, "      i64.gt_s").unwrap(),
                CmpOp::Ge => writeln!(out, "      i64.ge_s").unwrap(),
            }
            writeln!(out, "      i64.extend_i32_u").unwrap();
            writeln!(out, "      local.set $v{}", dst).unwrap();
        }
        Inst::Call(dst, name, args) => {
            for arg in args {
                writeln!(out, "      local.get $v{}", arg).unwrap();
            }
            writeln!(out, "      call ${}", name).unwrap();
            writeln!(out, "      local.set $v{}", dst).unwrap();
        }
        Inst::CallIndirect(dst, _, args) => {
            // Simplified: not supported in WASM easily
            for _ in args { writeln!(out, "      i64.const 0").unwrap(); }
            writeln!(out, "      i64.const 0").unwrap();
            writeln!(out, "      local.set $v{}", dst).unwrap();
        }
        Inst::LeaGlobal(dst, name) => {
            let offset = global_offsets.get(name).copied().unwrap_or(0);
            writeln!(out, "      i64.const {}", offset).unwrap();
            writeln!(out, "      local.set $v{}", dst).unwrap();
        }
        Inst::LeaLabel(dst, label) => {
            let offset = data_offsets.get(label).copied().unwrap_or(0);
            writeln!(out, "      i64.const {}", offset).unwrap();
            writeln!(out, "      local.set $v{}", dst).unwrap();
        }
        Inst::GetElementPtr(dst, base, idx, stride) => {
            writeln!(out, "      local.get $v{}", base).unwrap();
            writeln!(out, "      local.get $v{}", idx).unwrap();
            writeln!(out, "      i64.const {}", stride).unwrap();
            writeln!(out, "      i64.mul").unwrap();
            writeln!(out, "      i64.add").unwrap();
            writeln!(out, "      local.set $v{}", dst).unwrap();
        }
        Inst::AddImm(dst, base, offset) => {
            writeln!(out, "      local.get $v{}", base).unwrap();
            writeln!(out, "      i64.const {}", offset).unwrap();
            writeln!(out, "      i64.add").unwrap();
            writeln!(out, "      local.set $v{}", dst).unwrap();
        }
        Inst::Ext(dst, src, _, _) | Inst::Trunc(dst, src, _, _) => {
            writeln!(out, "      local.get $v{}", src).unwrap();
            writeln!(out, "      local.set $v{}", dst).unwrap();
        }
        Inst::Phi(_dst, _args) => {
            writeln!(out, "      ;; phi").unwrap();
        }
        Inst::Comment(s) => {
            writeln!(out, "      ;; {}", s).unwrap();
        }
    }
}

fn emit_wat_term(out: &mut String, term: &Terminator) {
    match term {
        Terminator::Ret(val) => {
            if let Some(v) = val {
                writeln!(out, "      local.get $v{}", v).unwrap();
                writeln!(out, "      ;; deallocate frame on return").unwrap();
                writeln!(out, "      local.get $fp").unwrap();
                writeln!(out, "      global.set $sp").unwrap();
                writeln!(out, "      return").unwrap();
            } else {
                writeln!(out, "      local.get $fp").unwrap();
                writeln!(out, "      global.set $sp").unwrap();
                writeln!(out, "      return").unwrap();
            }
        }
        Terminator::Jump(_label) => {
            // In simplified mode, jumps are just fall-through
            writeln!(out, "      ;; jump (fall-through)").unwrap();
        }
        Terminator::Branch(cond, _true_l, _false_l) => {
            // Simplified: branch as if/then
            writeln!(out, "      ;; branch on $v{}", cond).unwrap();
        }
    }
}
