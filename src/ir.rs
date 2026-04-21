/// Intermediate Representation for rcc
///
/// Linear IR with virtual registers (infinite supply).
/// Each function is a list of basic blocks.
/// Each basic block is a list of instructions ending with a terminator.

use std::fmt;

/// Virtual register ID
pub type VReg = u32;

/// IR type sizes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrType {
    I8,
    I16,
    I32,
    I64,
    F64,
    Ptr,
    Void,
}

impl IrType {
    pub fn size(&self) -> usize {
        match self {
            IrType::I8 => 1,
            IrType::I16 => 2,
            IrType::I32 => 4,
            IrType::I64 | IrType::F64 | IrType::Ptr => 8,
            IrType::Void => 0,
        }
    }

    pub fn from_ast_type(ty: &crate::ast::Type) -> Self {
        use crate::ast::Type;
        match ty {
            Type::Void => IrType::Void,
            Type::Bool | Type::Char | Type::UChar => IrType::I8,
            Type::Short | Type::UShort => IrType::I16,
            Type::Int | Type::UInt | Type::Enum(_) => IrType::I32,
            Type::Long | Type::ULong => IrType::I64,
            Type::Float | Type::Double => IrType::F64,
            Type::Ptr(_) | Type::Array(_, _) | Type::Func { .. } => IrType::Ptr,
            Type::Struct { .. } | Type::Union { .. } => IrType::I64,
        }
    }
}

/// Label for basic blocks
pub type Label = u32;

/// IR Instruction
#[derive(Debug, Clone)]
pub enum Inst {
    /// %dst = phi [(val, label), ...] — SSA phi node
    Phi(VReg, Vec<(VReg, Label)>),
    /// %dst = constant integer
    Const(VReg, i64, IrType),
    /// %dst = alloca size — stack allocate, returns pointer
    Alloca(VReg, usize),
    /// %dst = load [%addr], type
    Load(VReg, VReg, IrType),
    /// store %val, [%addr], type
    Store(VReg, VReg, IrType),
    /// %dst = binary op %lhs, %rhs
    BinOp(VReg, BinIrOp, VReg, VReg, IrType),
    /// %dst = unary op %src
    UnOp(VReg, UnIrOp, VReg, IrType),
    /// %dst = compare op %lhs, %rhs
    Cmp(VReg, CmpOp, VReg, VReg, IrType),
    /// %dst = call func(%args...)  — func is a string name or vreg
    Call(VReg, String, Vec<VReg>),
    /// %dst = call indirect %func_ptr(%args...)
    CallIndirect(VReg, VReg, Vec<VReg>),
    /// %dst = lea global_name — load effective address of global
    LeaGlobal(VReg, String),
    /// %dst = lea label (string literal etc)
    LeaLabel(VReg, Label),
    /// %dst = get element ptr: %base + %index * stride
    GetElementPtr(VReg, VReg, VReg, usize),
    /// %dst = add %base, offset (for struct member access)
    AddImm(VReg, VReg, i64),
    /// %dst = sign-extend or zero-extend %src from one type to another
    Ext(VReg, VReg, IrType, IrType),
    /// %dst = truncate %src
    Trunc(VReg, VReg, IrType, IrType),
    /// nop / comment
    Comment(String),
}

/// Binary operations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinIrOp {
    Add, Sub, Mul, Div, Mod,
    And, Or, Xor,
    Shl, Shr,
}

/// Unary operations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnIrOp {
    Neg, Not, BitNot,
}

/// Comparison operations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CmpOp {
    Eq, Ne, Lt, Le, Gt, Ge,
}

/// Basic block terminator
#[derive(Debug, Clone)]
pub enum Terminator {
    /// return %val (or void)
    Ret(Option<VReg>),
    /// unconditional jump
    Jump(Label),
    /// conditional branch: if %cond goto true_label else false_label
    Branch(VReg, Label, Label),
}

/// A basic block
#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub label: Label,
    pub insts: Vec<Inst>,
    pub term: Terminator,
}

/// String literal or read-only data
#[derive(Debug, Clone)]
pub struct IrData {
    pub label: Label,
    pub bytes: Vec<u8>,
}

/// An IR function
#[derive(Debug, Clone)]
pub struct IrFunction {
    pub name: String,
    pub params: Vec<(VReg, IrType)>,
    pub return_ty: IrType,
    pub blocks: Vec<BasicBlock>,
    pub next_vreg: VReg,
    pub next_label: Label,
}

impl IrFunction {
    pub fn new(name: &str, return_ty: IrType) -> Self {
        Self {
            name: name.to_string(),
            params: Vec::new(),
            return_ty,
            blocks: Vec::new(),
            next_vreg: 0,
            next_label: 0,
        }
    }

    pub fn new_vreg(&mut self) -> VReg {
        let v = self.next_vreg;
        self.next_vreg += 1;
        v
    }

    pub fn new_label(&mut self) -> Label {
        let l = self.next_label;
        self.next_label += 1;
        l
    }
}

/// Global variable
#[derive(Debug, Clone)]
pub struct IrGlobal {
    pub name: String,
    pub size: usize,
    pub init: Option<Vec<u8>>,
}

/// Complete IR module (translation unit)
#[derive(Debug)]
pub struct IrModule {
    pub functions: Vec<IrFunction>,
    pub globals: Vec<IrGlobal>,
    pub data: Vec<IrData>,
}

impl IrModule {
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            globals: Vec::new(),
            data: Vec::new(),
        }
    }
}

// ── Pretty printer ──

impl fmt::Display for IrModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for g in &self.globals {
            if let Some(init) = &g.init {
                writeln!(f, "@{} = global [{} bytes] {:?}", g.name, g.size, init)?;
            } else {
                writeln!(f, "@{} = global [{} bytes] zeroinit", g.name, g.size)?;
            }
        }
        for d in &self.data {
            writeln!(f, ".data.L{} = {:?}", d.label, String::from_utf8_lossy(&d.bytes))?;
        }
        for func in &self.functions {
            writeln!(f, "\nfn {}({}) -> {:?} {{",
                func.name,
                func.params.iter().map(|(v, t)| format!("%{}: {:?}", v, t)).collect::<Vec<_>>().join(", "),
                func.return_ty,
            )?;
            for bb in &func.blocks {
                writeln!(f, "  .L{}:", bb.label)?;
                for inst in &bb.insts {
                    writeln!(f, "    {}", format_inst(inst))?;
                }
                writeln!(f, "    {}", format_term(&bb.term))?;
            }
            writeln!(f, "}}")?;
        }
        Ok(())
    }
}

fn format_inst(inst: &Inst) -> String {
    match inst {
        Inst::Phi(d, args) => {
            let args_str = args.iter().map(|(v, l)| format!("[%{}, .L{}]", v, l)).collect::<Vec<_>>().join(", ");
            format!("%{} = phi {}", d, args_str)
        }
        Inst::Const(d, v, t) => format!("%{} = const {:?} {}", d, t, v),
        Inst::Alloca(d, sz) => format!("%{} = alloca {}", d, sz),
        Inst::Load(d, addr, t) => format!("%{} = load {:?} [%{}]", d, t, addr),
        Inst::Store(v, addr, t) => format!("store {:?} %{}, [%{}]", t, v, addr),
        Inst::BinOp(d, op, l, r, t) => format!("%{} = {:?} {:?} %{}, %{}", d, op, t, l, r),
        Inst::UnOp(d, op, s, t) => format!("%{} = {:?} {:?} %{}", d, op, t, s),
        Inst::Cmp(d, op, l, r, t) => format!("%{} = {:?} {:?} %{}, %{}", d, op, t, l, r),
        Inst::Call(d, name, args) => format!("%{} = call {}({})", d, name,
            args.iter().map(|a| format!("%{}", a)).collect::<Vec<_>>().join(", ")),
        Inst::CallIndirect(d, f, args) => format!("%{} = call_indirect %{}({})", d, f,
            args.iter().map(|a| format!("%{}", a)).collect::<Vec<_>>().join(", ")),
        Inst::LeaGlobal(d, name) => format!("%{} = lea @{}", d, name),
        Inst::LeaLabel(d, l) => format!("%{} = lea .L{}", d, l),
        Inst::GetElementPtr(d, base, idx, stride) => format!("%{} = gep %{}, %{}, stride={}", d, base, idx, stride),
        Inst::AddImm(d, base, off) => format!("%{} = add %{}, {}", d, base, off),
        Inst::Ext(d, s, from, to) => format!("%{} = ext {:?}->{:?} %{}", d, from, to, s),
        Inst::Trunc(d, s, from, to) => format!("%{} = trunc {:?}->{:?} %{}", d, from, to, s),
        Inst::Comment(s) => format!("# {}", s),
    }
}

fn format_term(term: &Terminator) -> String {
    match term {
        Terminator::Ret(None) => "ret void".to_string(),
        Terminator::Ret(Some(v)) => format!("ret %{}", v),
        Terminator::Jump(l) => format!("jmp .L{}", l),
        Terminator::Branch(c, t, f) => format!("br %{}, .L{}, .L{}", c, t, f),
    }
}
