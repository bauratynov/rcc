/// SSA Construction — full mem2reg with phi nodes
///
/// Implements Braun et al. 2013 "Simple and Efficient Construction of SSA Form"
///
/// Promotes alloca'd scalar variables to SSA virtual registers.
/// Inserts phi nodes at join points where multiple definitions reach.
/// Then eliminates phi nodes by inserting mov instructions at predecessor block ends.

use std::collections::{HashMap, HashSet};
use crate::ir::*;

/// Per-block variable definitions: block_label → (alloca_vreg → value_vreg)
type BlockDefs = HashMap<Label, HashMap<VReg, VReg>>;

pub fn promote_to_ssa(func: &mut IrFunction) {
    let promotable = find_promotable_allocas(func);
    if promotable.is_empty() { return; }

    let preds = build_predecessors(func);
    let block_labels: Vec<Label> = func.blocks.iter().map(|b| b.label).collect();

    // ── Phase 1: Braun's algorithm — compute reaching definitions ──

    // First pass: record stores within each block
    let mut block_defs: BlockDefs = HashMap::new();
    let mut block_local_flow: HashMap<Label, Vec<(VReg, VReg, bool)>> = HashMap::new();
    // (alloca, value_or_dst, is_store)

    for bb in &func.blocks {
        let mut flow = Vec::new();
        for inst in &bb.insts {
            match inst {
                Inst::Store(val, addr, _) if promotable.contains(addr) => {
                    flow.push((*addr, *val, true));
                }
                Inst::Load(dst, addr, _) if promotable.contains(addr) => {
                    flow.push((*addr, *dst, false));
                }
                _ => {}
            }
        }
        // Record the LAST store for each alloca as the block's exit definition
        for &(alloca, val, is_store) in flow.iter().rev() {
            if is_store {
                block_defs.entry(bb.label).or_default().entry(alloca).or_insert(val);
            }
        }
        block_local_flow.insert(bb.label, flow);
    }

    // readVariable: find reaching definition with phi insertion
    // Use iterative approach instead of recursion to avoid borrow issues

    // For each promotable alloca, compute its value at the START of each block
    let mut block_entry_defs: BlockDefs = HashMap::new();

    // Iterate until stable
    for _ in 0..(block_labels.len() + 2) {
        let mut changed = false;

        for &label in &block_labels {
            let pred_labels = preds.get(&label).cloned().unwrap_or_default();

            for &alloca in &promotable {
                // Skip if already defined at entry of this block
                if block_entry_defs.get(&label).and_then(|d| d.get(&alloca)).is_some() {
                    continue;
                }

                if pred_labels.is_empty() {
                    // Entry block — no predecessors, initial value is 0
                    // Don't set entry def — will be handled by store in this block
                    continue;
                }

                // Collect definitions from all predecessors' exits
                let mut pred_vals: Vec<(VReg, Label)> = Vec::new();
                for &pl in &pred_labels {
                    // Exit def = entry def modified by stores within the block
                    let exit_val = block_defs.get(&pl).and_then(|d| d.get(&alloca))
                        .or_else(|| block_entry_defs.get(&pl).and_then(|d| d.get(&alloca)));
                    if let Some(&val) = exit_val {
                        pred_vals.push((val, pl));
                    }
                }

                if pred_vals.is_empty() {
                    continue; // No predecessor defines this — will use 0
                }

                // Check if all predecessors agree
                let all_same = pred_vals.iter().all(|(v, _)| *v == pred_vals[0].0);

                if all_same {
                    // No phi needed — all preds give same value
                    block_entry_defs.entry(label).or_default().insert(alloca, pred_vals[0].0);
                    changed = true;
                } else if pred_vals.len() == pred_labels.len() {
                    // Need phi — different values from different predecessors
                    // Create a new vreg for the phi result
                    let phi_vreg = func.new_vreg();
                    block_entry_defs.entry(label).or_default().insert(alloca, phi_vreg);
                    changed = true;
                }
            }
        }

        if !changed { break; }
    }

    // ── Phase 2: rebuild blocks with loads/stores replaced ──

    let mut new_blocks = Vec::new();

    for bb in &func.blocks {
        let mut new_insts = Vec::new();
        let mut current: HashMap<VReg, VReg> = HashMap::new();

        // Initialize with entry definitions
        if let Some(entry) = block_entry_defs.get(&bb.label) {
            current = entry.clone();
        }

        // Insert phi nodes at the beginning (for allocas with entry defs from phi)
        // These will be eliminated later by inserting movs at pred block ends

        for inst in &bb.insts {
            match inst {
                Inst::Store(val, addr, _) if promotable.contains(addr) => {
                    current.insert(*addr, *val);
                    // Don't emit store
                }
                Inst::Load(dst, addr, _) if promotable.contains(addr) => {
                    if let Some(&val) = current.get(addr) {
                        // Replace with copy
                        new_insts.push(Inst::AddImm(*dst, val, 0));
                    } else {
                        // No def yet — init to 0
                        new_insts.push(Inst::Const(*dst, 0, IrType::I64));
                    }
                }
                Inst::Alloca(dst, _) if promotable.contains(dst) => {
                    // Remove alloca
                }
                other => {
                    new_insts.push(other.clone());
                }
            }
        }

        // Update block exit defs for next iteration
        for (alloca, val) in &current {
            block_defs.entry(bb.label).or_default().insert(*alloca, *val);
        }

        new_blocks.push(BasicBlock {
            label: bb.label,
            insts: new_insts,
            term: bb.term.clone(),
        });
    }

    func.blocks = new_blocks;

    // ── Phase 3: handle phi-like cross-block values ──
    // For blocks where entry def came from phi (multiple preds with different vals),
    // insert mov at the end of each predecessor block

    for &label in &block_labels {
        if let Some(entry) = block_entry_defs.get(&label) {
            let pred_labels = preds.get(&label).cloned().unwrap_or_default();
            if pred_labels.len() <= 1 { continue; }

            for (&alloca, &phi_vreg) in entry {
                // Check if this was a phi (created by us)
                // Insert mov at end of each predecessor
                for &pl in &pred_labels {
                    if let Some(&pred_exit_val) = block_defs.get(&pl).and_then(|d| d.get(&alloca)) {
                        // Find predecessor block and insert mov before terminator
                        for bb in &mut func.blocks {
                            if bb.label == pl {
                                // Insert: phi_vreg = pred_exit_val
                                bb.insts.push(Inst::AddImm(phi_vreg, pred_exit_val, 0));
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
}

fn build_predecessors(func: &IrFunction) -> HashMap<Label, Vec<Label>> {
    let mut preds: HashMap<Label, Vec<Label>> = HashMap::new();
    for bb in &func.blocks {
        match &bb.term {
            Terminator::Jump(target) => {
                preds.entry(*target).or_default().push(bb.label);
            }
            Terminator::Branch(_, true_l, false_l) => {
                preds.entry(*true_l).or_default().push(bb.label);
                if true_l != false_l {
                    preds.entry(*false_l).or_default().push(bb.label);
                }
            }
            Terminator::Ret(_) => {}
        }
    }
    preds
}

fn find_promotable_allocas(func: &IrFunction) -> HashSet<VReg> {
    let mut allocas: HashSet<VReg> = HashSet::new();
    let mut non_promotable: HashSet<VReg> = HashSet::new();

    for bb in &func.blocks {
        for inst in &bb.insts {
            if let Inst::Alloca(dst, size) = inst {
                if *size <= 8 { allocas.insert(*dst); }
            }
        }
    }

    for bb in &func.blocks {
        for inst in &bb.insts {
            match inst {
                Inst::Load(_, _, _) | Inst::Store(_, _, _) => {}
                Inst::GetElementPtr(_, base, idx, _) => {
                    non_promotable.insert(*base);
                    non_promotable.insert(*idx);
                }
                Inst::AddImm(_, base, off) if *off != 0 => {
                    non_promotable.insert(*base);
                }
                Inst::Call(_, _, args) | Inst::CallIndirect(_, _, args) => {
                    for a in args { non_promotable.insert(*a); }
                }
                _ => {}
            }
        }
    }

    allocas.difference(&non_promotable).cloned().collect()
}
