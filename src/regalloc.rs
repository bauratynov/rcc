/// Linear scan register allocator for IR backend
///
/// Assigns virtual registers to physical registers based on live ranges.
/// Spills to stack when registers are exhausted.
/// Uses callee-saved registers (rbx, r12-r15) to avoid save/restore on calls.

use std::collections::{HashMap, HashSet};
use crate::ir::*;

/// Physical register assignment
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Location {
    Reg(&'static str),
    Stack(i64), // offset from rbp
}

/// Available callee-saved registers for allocation
/// (rbx, r12, r13, r14, r15 — preserved across calls)
const ALLOC_REGS: [&str; 5] = ["%rbx", "%r12", "%r13", "%r14", "%r15"];

/// Live range of a virtual register
#[derive(Debug, Clone)]
struct LiveRange {
    vreg: VReg,
    start: usize, // first instruction index (global)
    end: usize,   // last instruction index
    use_count: usize,
}

/// Register allocation result
pub struct RegAllocResult {
    pub assignments: HashMap<VReg, Location>,
    pub used_callee_saved: Vec<&'static str>,
    pub stack_size: usize,
}

pub fn allocate(func: &IrFunction) -> RegAllocResult {
    // Collect vregs to exclude: allocas, addresses, params
    let mut exclude: HashSet<VReg> = HashSet::new();
    for bb in &func.blocks {
        for inst in &bb.insts {
            match inst {
                Inst::Alloca(d, _) => { exclude.insert(*d); }
                Inst::Store(v, addr, _) => { exclude.insert(*addr); exclude.insert(*v); }
                Inst::Load(d, addr, _) => { exclude.insert(*addr); exclude.insert(*d); }
                Inst::GetElementPtr(d, base, idx, _) => { exclude.insert(*d); exclude.insert(*base); exclude.insert(*idx); }
                Inst::AddImm(d, base, _) => { exclude.insert(*d); exclude.insert(*base); }
                Inst::LeaGlobal(d, _) | Inst::LeaLabel(d, _) => { exclude.insert(*d); }
                Inst::Call(_, _, _) | Inst::CallIndirect(_, _, _) => {
                    // Exclude call results — they're in rax
                    if let Some(d) = match inst {
                        Inst::Call(d, _, _) | Inst::CallIndirect(d, _, _) => Some(*d),
                        _ => None,
                    } { exclude.insert(d); }
                }
                _ => {}
            }
        }
    }
    for (v, _) in &func.params { exclude.insert(*v); }

    // Step 1: compute live ranges
    let ranges = compute_live_ranges(func);

    // Only allocate registers for pure computation results (BinOp, Cmp, UnOp, Const)
    // Exclude anything that touches memory or function calls
    let mut safe_vregs: HashSet<VReg> = HashSet::new();
    for bb in &func.blocks {
        for inst in &bb.insts {
            match inst {
                Inst::BinOp(d, _, _, _, _) | Inst::Cmp(d, _, _, _, _)
                | Inst::UnOp(d, _, _, _) | Inst::Const(d, _, _) => {
                    if !exclude.contains(d) {
                        safe_vregs.insert(*d);
                    }
                }
                _ => {}
            }
        }
    }

    // Step 2: sort by start position, only include safe vregs
    let mut sorted: Vec<LiveRange> = ranges.values()
        .filter(|r| safe_vregs.contains(&r.vreg))
        .cloned().collect();
    sorted.sort_by_key(|r| r.start);

    // Step 3: assign registers greedily
    let mut assignments: HashMap<VReg, Location> = HashMap::new();
    let mut active: Vec<(VReg, usize, usize)> = Vec::new(); // (vreg, end, reg_idx)
    let mut free_regs: Vec<bool> = vec![true; ALLOC_REGS.len()];
    let mut stack_offset = 0i64;
    let mut used_regs = vec![false; ALLOC_REGS.len()];

    // Reserve registers for function params (they start in specific regs)
    // Params are already handled by the backend, skip them

    for range in &sorted {
        // Expire old ranges
        active.retain(|&(v, end, ridx)| {
            if end <= range.start {
                free_regs[ridx] = true;
                false
            } else {
                true
            }
        });

        // Try to find a free register
        let mut assigned = false;
        for (idx, &is_free) in free_regs.iter().enumerate() {
            if is_free {
                assignments.insert(range.vreg, Location::Reg(ALLOC_REGS[idx]));
                free_regs[idx] = false;
                used_regs[idx] = true;
                active.push((range.vreg, range.end, idx));
                assigned = true;
                break;
            }
        }

        if !assigned {
            // Spill to stack
            // For safety, all vregs go to stack for now (register allocation disabled)
            // stack_offset += 8;
            // assignments.insert(range.vreg, Location::Stack(-stack_offset));
        }
    }

    let used_callee_saved: Vec<&str> = used_regs.iter().enumerate()
        .filter(|(_, used)| **used)
        .map(|(i, _)| ALLOC_REGS[i])
        .collect();

    RegAllocResult {
        assignments,
        used_callee_saved,
        stack_size: stack_offset as usize,
    }
}

fn compute_live_ranges(func: &IrFunction) -> HashMap<VReg, LiveRange> {
    let mut ranges: HashMap<VReg, LiveRange> = HashMap::new();
    let mut global_idx = 0usize;

    for bb in &func.blocks {
        for inst in &bb.insts {
            // Get defined and used vregs
            let (defs, uses) = inst_vregs(inst);

            for &v in &defs {
                let entry = ranges.entry(v).or_insert(LiveRange {
                    vreg: v, start: global_idx, end: global_idx, use_count: 0,
                });
                entry.end = global_idx;
                entry.use_count += 1;
            }

            for &v in &uses {
                let entry = ranges.entry(v).or_insert(LiveRange {
                    vreg: v, start: global_idx, end: global_idx, use_count: 0,
                });
                entry.end = global_idx;
                entry.use_count += 1;
            }

            global_idx += 1;
        }

        // Terminator uses
        match &bb.term {
            Terminator::Ret(Some(v)) => {
                let entry = ranges.entry(*v).or_insert(LiveRange {
                    vreg: *v, start: global_idx, end: global_idx, use_count: 0,
                });
                entry.end = global_idx;
                entry.use_count += 1;
            }
            Terminator::Branch(v, _, _) => {
                let entry = ranges.entry(*v).or_insert(LiveRange {
                    vreg: *v, start: global_idx, end: global_idx, use_count: 0,
                });
                entry.end = global_idx;
                entry.use_count += 1;
            }
            _ => {}
        }
        global_idx += 1;
    }

    ranges
}

fn inst_vregs(inst: &Inst) -> (Vec<VReg>, Vec<VReg>) {
    match inst {
        Inst::Const(d, _, _) => (vec![*d], vec![]),
        Inst::Alloca(d, _) => (vec![*d], vec![]),
        Inst::Load(d, addr, _) => (vec![*d], vec![*addr]),
        Inst::Store(v, addr, _) => (vec![], vec![*v, *addr]),
        Inst::BinOp(d, _, l, r, _) => (vec![*d], vec![*l, *r]),
        Inst::UnOp(d, _, s, _) => (vec![*d], vec![*s]),
        Inst::Cmp(d, _, l, r, _) => (vec![*d], vec![*l, *r]),
        Inst::Call(d, _, args) => (vec![*d], args.clone()),
        Inst::CallIndirect(d, f, args) => {
            let mut uses = vec![*f];
            uses.extend(args);
            (vec![*d], uses)
        }
        Inst::LeaGlobal(d, _) => (vec![*d], vec![]),
        Inst::LeaLabel(d, _) => (vec![*d], vec![]),
        Inst::GetElementPtr(d, base, idx, _) => (vec![*d], vec![*base, *idx]),
        Inst::AddImm(d, base, _) => (vec![*d], vec![*base]),
        Inst::Ext(d, s, _, _) | Inst::Trunc(d, s, _, _) => (vec![*d], vec![*s]),
        Inst::Comment(_) => (vec![], vec![]),
        Inst::Phi(d, args) => (vec![*d], args.iter().map(|(v, _)| *v).collect()),
    }
}
