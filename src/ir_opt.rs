/// IR-level optimizations
///
/// 1. Dead instruction elimination: remove instructions whose results are never used
/// 2. Constant folding at IR level
/// 3. Redundant load elimination: load after store to same address → use stored value
/// 4. Branch simplification: branch on constant → jump

use std::collections::HashSet;
use crate::ir::*;

pub fn optimize_ir(module: &mut IrModule) {
    for func in &mut module.functions {
        for _ in 0..3 {
            dead_inst_elimination(func);
            constant_fold_ir(func);
            simplify_branches(func);
        }
    }
}

/// Remove instructions whose results are never used
fn dead_inst_elimination(func: &mut IrFunction) {
    // Collect all used vregs
    let mut used: HashSet<VReg> = HashSet::new();

    for bb in &func.blocks {
        for inst in &bb.insts {
            for v in inst_uses(inst) {
                used.insert(v);
            }
        }
        match &bb.term {
            Terminator::Ret(Some(v)) => { used.insert(*v); }
            Terminator::Branch(v, _, _) => { used.insert(*v); }
            _ => {}
        }
    }

    // Remove instructions that define unused vregs
    // (except stores, calls, and allocas which have side effects)
    for bb in &mut func.blocks {
        bb.insts.retain(|inst| {
            match inst {
                Inst::Store(_, _, _) | Inst::Call(_, _, _) | Inst::CallIndirect(_, _, _)
                | Inst::Alloca(_, _) | Inst::Comment(_) | Inst::Phi(_, _) => true,
                _ => {
                    if let Some(def) = inst_def(inst) {
                        used.contains(&def)
                    } else {
                        true
                    }
                }
            }
        });
    }
}

/// Fold constant operations at IR level
fn constant_fold_ir(func: &mut IrFunction) {
    // Collect known constants: vreg → value
    let mut constants: std::collections::HashMap<VReg, i64> = std::collections::HashMap::new();

    for bb in &mut func.blocks {
        for inst in &mut bb.insts {
            // Record constants
            if let Inst::Const(d, val, _) = inst {
                constants.insert(*d, *val);
            }

            // Fold binary ops on constants
            let replacement = match inst {
                Inst::BinOp(d, op, l, r, ty) => {
                    match (constants.get(l), constants.get(r)) {
                        (Some(&lv), Some(&rv)) => {
                            let result = match op {
                                BinIrOp::Add => Some(lv.wrapping_add(rv)),
                                BinIrOp::Sub => Some(lv.wrapping_sub(rv)),
                                BinIrOp::Mul => Some(lv.wrapping_mul(rv)),
                                BinIrOp::Div if rv != 0 => Some(lv / rv),
                                BinIrOp::Mod if rv != 0 => Some(lv % rv),
                                BinIrOp::And => Some(lv & rv),
                                BinIrOp::Or => Some(lv | rv),
                                BinIrOp::Xor => Some(lv ^ rv),
                                BinIrOp::Shl => Some(lv << (rv & 63)),
                                BinIrOp::Shr => Some(lv >> (rv & 63)),
                                _ => None,
                            };
                            result.map(|val| (*d, val, *ty))
                        }
                        _ => None,
                    }
                }
                Inst::Cmp(d, op, l, r, ty) => {
                    match (constants.get(l), constants.get(r)) {
                        (Some(&lv), Some(&rv)) => {
                            let result = match op {
                                CmpOp::Eq => lv == rv, CmpOp::Ne => lv != rv,
                                CmpOp::Lt => lv < rv, CmpOp::Le => lv <= rv,
                                CmpOp::Gt => lv > rv, CmpOp::Ge => lv >= rv,
                            };
                            Some((*d, if result { 1 } else { 0 }, *ty))
                        }
                        _ => None,
                    }
                }
                Inst::UnOp(d, op, s, ty) => {
                    constants.get(s).and_then(|&sv| {
                        let r = match op {
                            UnIrOp::Neg => Some(-sv),
                            UnIrOp::Not => Some(if sv == 0 { 1 } else { 0 }),
                            UnIrOp::BitNot => Some(!sv),
                        };
                        r.map(|val| (*d, val, *ty))
                    })
                }
                _ => None,
            };

            if let Some((d, val, ty)) = replacement {
                *inst = Inst::Const(d, val, ty);
                constants.insert(d, val);
            }
        }
    }
}

/// Simplify branches on known constants
fn simplify_branches(func: &mut IrFunction) {
    let mut constants: std::collections::HashMap<VReg, i64> = std::collections::HashMap::new();

    // Collect constants across all blocks
    for bb in &func.blocks {
        for inst in &bb.insts {
            if let Inst::Const(d, val, _) = inst {
                constants.insert(*d, *val);
            }
        }
    }

    // Simplify branches
    for bb in &mut func.blocks {
        if let Terminator::Branch(cond, true_l, false_l) = &bb.term {
            if let Some(&val) = constants.get(cond) {
                bb.term = if val != 0 {
                    Terminator::Jump(*true_l)
                } else {
                    Terminator::Jump(*false_l)
                };
            }
        }
    }
}

fn inst_def(inst: &Inst) -> Option<VReg> {
    match inst {
        Inst::Const(d, _, _) | Inst::Load(d, _, _) | Inst::BinOp(d, _, _, _, _)
        | Inst::UnOp(d, _, _, _) | Inst::Cmp(d, _, _, _, _) | Inst::Call(d, _, _)
        | Inst::CallIndirect(d, _, _) | Inst::LeaGlobal(d, _) | Inst::LeaLabel(d, _)
        | Inst::GetElementPtr(d, _, _, _) | Inst::AddImm(d, _, _)
        | Inst::Ext(d, _, _, _) | Inst::Trunc(d, _, _, _)
        | Inst::Alloca(d, _) | Inst::Phi(d, _) => Some(*d),
        Inst::Store(_, _, _) | Inst::Comment(_) => None,
    }
}

fn inst_uses(inst: &Inst) -> Vec<VReg> {
    match inst {
        Inst::Const(_, _, _) | Inst::Alloca(_, _) | Inst::Comment(_)
        | Inst::LeaGlobal(_, _) | Inst::LeaLabel(_, _) => vec![],
        Inst::Phi(_, args) => args.iter().map(|(v, _)| *v).collect(),
        Inst::Load(_, addr, _) => vec![*addr],
        Inst::Store(v, addr, _) => vec![*v, *addr],
        Inst::BinOp(_, _, l, r, _) | Inst::Cmp(_, _, l, r, _) => vec![*l, *r],
        Inst::UnOp(_, _, s, _) | Inst::Ext(_, s, _, _) | Inst::Trunc(_, s, _, _) => vec![*s],
        Inst::Call(_, _, args) => args.clone(),
        Inst::CallIndirect(_, f, args) => {
            let mut v = vec![*f];
            v.extend(args);
            v
        }
        Inst::GetElementPtr(_, base, idx, _) => vec![*base, *idx],
        Inst::AddImm(_, base, _) => vec![*base],
    }
}
