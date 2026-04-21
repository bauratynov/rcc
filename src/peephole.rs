/// Peephole optimizer for x86-64 assembly
///
/// Runs on the text assembly output and applies local optimizations:
/// 1. Remove redundant mov (mov X, Y; mov Y, X)
/// 2. Remove dead stores (mov to location that's immediately overwritten)
/// 3. Eliminate mov X, X (self-moves)
/// 4. Strength reduction: imul $1 → nop, add $0 → nop
/// 5. Combine push/pop pairs into mov
/// 6. Remove jumps to next instruction

pub fn optimize_asm(input: &str) -> String {
    let mut lines: Vec<String> = input.lines().map(|l| l.to_string()).collect();

    // Multiple passes for cascading optimizations
    for _ in 0..3 {
        lines = eliminate_self_moves(lines);
        lines = eliminate_redundant_mov_pairs(lines);
        lines = eliminate_push_pop_pairs(lines);
        lines = eliminate_dead_stores(lines);
        lines = eliminate_trivial_ops(lines);
        lines = eliminate_jumps_to_next(lines);
    }

    let mut result = String::new();
    for line in &lines {
        result.push_str(line);
        result.push('\n');
    }
    result
}

/// Remove `mov %X, %X` (self-move)
fn eliminate_self_moves(lines: Vec<String>) -> Vec<String> {
    lines.into_iter().filter(|line| {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("mov ") {
            let parts: Vec<&str> = rest.splitn(2, ", ").collect();
            if parts.len() == 2 && parts[0].trim() == parts[1].trim() {
                return false; // Remove self-move
            }
        }
        true
    }).collect()
}

/// Remove `mov A, B; mov B, A` patterns
fn eliminate_redundant_mov_pairs(lines: Vec<String>) -> Vec<String> {
    let mut result = Vec::with_capacity(lines.len());
    let mut i = 0;

    while i < lines.len() {
        if i + 1 < lines.len() {
            let a = lines[i].trim();
            let b = lines[i + 1].trim();

            if let (Some(mov1), Some(mov2)) = (parse_mov(a), parse_mov(b)) {
                // mov A, B; mov B, A → mov A, B (remove second)
                if mov1.0 == mov2.1 && mov1.1 == mov2.0 {
                    result.push(lines[i].clone());
                    i += 2;
                    continue;
                }
            }
        }
        result.push(lines[i].clone());
        i += 1;
    }
    result
}

/// Replace `push %X; pop %Y` with `mov %X, %Y`
fn eliminate_push_pop_pairs(lines: Vec<String>) -> Vec<String> {
    let mut result = Vec::with_capacity(lines.len());
    let mut i = 0;

    while i < lines.len() {
        if i + 1 < lines.len() {
            let a = lines[i].trim();
            let b = lines[i + 1].trim();

            if let (Some(push_reg), Some(pop_reg)) = (parse_push(a), parse_pop(b)) {
                if push_reg == pop_reg {
                    // push %X; pop %X → nothing
                    i += 2;
                    continue;
                } else {
                    // push %X; pop %Y → mov %X, %Y
                    result.push(format!("  mov {}, {}", push_reg, pop_reg));
                    i += 2;
                    continue;
                }
            }
        }
        result.push(lines[i].clone());
        i += 1;
    }
    result
}

/// Remove store immediately followed by overwriting store to same location
fn eliminate_dead_stores(lines: Vec<String>) -> Vec<String> {
    let mut result = Vec::with_capacity(lines.len());
    let mut i = 0;

    while i < lines.len() {
        if i + 1 < lines.len() {
            let a = lines[i].trim();
            let b = lines[i + 1].trim();

            if let (Some(mov1), Some(mov2)) = (parse_mov(a), parse_mov(b)) {
                // mov X, DEST; mov Y, DEST → skip first (dead store)
                if mov1.1 == mov2.1 && !mov1.1.starts_with('%') {
                    // Only if dest is memory, not register (registers might be read between)
                    i += 1; // Skip the dead store
                    continue;
                }
            }
        }
        result.push(lines[i].clone());
        i += 1;
    }
    result
}

/// Remove trivial operations: add $0, imul $1, sub $0
fn eliminate_trivial_ops(lines: Vec<String>) -> Vec<String> {
    lines.into_iter().filter(|line| {
        let trimmed = line.trim();
        if trimmed == "add $0, %rax" || trimmed == "add $0, %rdi" { return false; }
        if trimmed == "sub $0, %rax" || trimmed == "sub $0, %rdi" { return false; }
        if trimmed == "imul $1, %rax" { return false; }
        if trimmed == "shl $0, %rax" || trimmed == "shr $0, %rax" { return false; }
        true
    }).collect()
}

/// Remove `jmp .LX` immediately followed by `.LX:`
fn eliminate_jumps_to_next(lines: Vec<String>) -> Vec<String> {
    let mut result = Vec::with_capacity(lines.len());
    let mut i = 0;

    while i < lines.len() {
        if i + 1 < lines.len() {
            let a = lines[i].trim();
            let b = lines[i + 1].trim();

            if let Some(target) = a.strip_prefix("jmp ") {
                let label = format!("{}:", target.trim());
                if b == label {
                    // jmp .LX; .LX: → just .LX:
                    i += 1;
                    continue;
                }
            }
        }
        result.push(lines[i].clone());
        i += 1;
    }
    result
}

// ── Helpers ──

fn parse_mov(s: &str) -> Option<(&str, &str)> {
    let s = s.strip_prefix("mov ")?;
    let (src, dst) = s.split_once(", ")?;
    Some((src.trim(), dst.trim()))
}

fn parse_push(s: &str) -> Option<&str> {
    s.strip_prefix("push ").map(|r| r.trim())
}

fn parse_pop(s: &str) -> Option<&str> {
    s.strip_prefix("pop ").map(|r| r.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_self_move() {
        let input = "  mov %rax, %rax\n  mov %rcx, %rdx\n";
        let output = optimize_asm(input);
        assert!(!output.contains("mov %rax, %rax"));
        assert!(output.contains("mov %rcx, %rdx"));
    }

    #[test]
    fn test_push_pop_same() {
        let input = "  push %rax\n  pop %rax\n";
        let output = optimize_asm(input);
        assert!(!output.contains("push"));
        assert!(!output.contains("pop"));
    }

    #[test]
    fn test_push_pop_different() {
        let input = "  push %rax\n  pop %rcx\n";
        let output = optimize_asm(input);
        assert!(output.contains("mov %rax, %rcx"));
        assert!(!output.contains("push"));
    }

    #[test]
    fn test_jump_to_next() {
        let input = "  jmp .L1\n.L1:\n  mov $1, %rax\n";
        let output = optimize_asm(input);
        assert!(!output.contains("jmp .L1"));
        assert!(output.contains(".L1:"));
    }
}
