use crate::error::Span;

/// C types
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Void,
    Bool,
    Char,
    UChar,
    Short,
    UShort,
    Int,
    UInt,
    Long,
    ULong,
    Float,
    Double,
    Ptr(Box<Type>),
    Array(Box<Type>, usize),
    Func {
        return_ty: Box<Type>,
        params: Vec<(Type, String)>,
        is_variadic: bool,
    },
    Struct {
        name: Option<String>,
        members: Vec<StructMember>,
    },
    Union {
        name: Option<String>,
        members: Vec<StructMember>,
    },
    Enum(Option<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructMember {
    pub name: String,
    pub ty: Type,
    pub offset: usize,
}

impl Type {
    pub fn size(&self) -> usize {
        match self {
            Type::Void => 0,
            Type::Bool | Type::Char | Type::UChar => 1,
            Type::Short | Type::UShort => 2,
            Type::Int | Type::UInt | Type::Float | Type::Enum(_) => 4,
            Type::Long | Type::ULong | Type::Double | Type::Ptr(_) => 8,
            Type::Array(base, len) => base.size() * len,
            Type::Struct { members, .. } => {
                members.last().map_or(0, |m| m.offset + m.ty.size())
            }
            Type::Union { members, .. } => {
                members.iter().map(|m| m.ty.size()).max().unwrap_or(0)
            }
            Type::Func { .. } => 8,
        }
    }

    pub fn is_unsigned(&self) -> bool {
        matches!(self, Type::UChar | Type::UShort | Type::UInt | Type::ULong | Type::Bool)
    }

    pub fn is_integer(&self) -> bool {
        matches!(self, Type::Bool | Type::Char | Type::UChar | Type::Short | Type::UShort
            | Type::Int | Type::UInt | Type::Long | Type::ULong | Type::Enum(_))
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Type::Float | Type::Double)
    }

    pub fn is_numeric(&self) -> bool {
        self.is_integer() || self.is_float()
    }

    pub fn is_ptr(&self) -> bool {
        matches!(self, Type::Ptr(_) | Type::Array(_, _))
    }

    pub fn base_type(&self) -> Option<&Type> {
        match self {
            Type::Ptr(base) | Type::Array(base, _) => Some(base),
            _ => None,
        }
    }
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Le, Gt, Ge,
    BitAnd, BitOr, BitXor,
    Shl, Shr,
    LogAnd, LogOr,
    Assign,
    AddAssign, SubAssign, MulAssign, DivAssign, ModAssign,
    BitAndAssign, BitOrAssign, BitXorAssign,
    ShlAssign, ShrAssign,
    Comma,
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOp {
    Neg,     // -
    Not,     // !
    BitNot,  // ~
    Addr,    // &
    Deref,   // *
    PreInc,  // ++x
    PreDec,  // --x
    PostInc, // x++
    PostDec, // x--
}

/// Expression node
#[derive(Debug, Clone)]
pub enum Expr {
    IntLit(i64, Span),
    FloatLit(f64, Span),
    StrLit(String, Span),
    CharLit(i64, Span),
    Var(String, Span),
    Binary(BinOp, Box<Expr>, Box<Expr>, Span),
    Unary(UnaryOp, Box<Expr>, Span),
    Call(Box<Expr>, Vec<Expr>, Span),
    Member(Box<Expr>, String, Span),     // expr.member
    Arrow(Box<Expr>, String, Span),      // expr->member
    Index(Box<Expr>, Box<Expr>, Span),   // expr[index]
    Cast(Type, Box<Expr>, Span),
    Sizeof(Box<SizeofArg>, Span),
    Cond(Box<Expr>, Box<Expr>, Box<Expr>, Span), // ternary ? :
    InitList(Vec<Expr>, Span), // {1, 2, 3}
    StmtExpr(Vec<Stmt>, Box<Expr>, Span), // ({ stmt; stmt; expr }) — GCC extension
}

#[derive(Debug, Clone)]
pub enum SizeofArg {
    Type(Type),
    Expr(Expr),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::IntLit(_, s) | Expr::FloatLit(_, s) | Expr::StrLit(_, s)
            | Expr::CharLit(_, s) | Expr::Var(_, s) | Expr::Binary(_, _, _, s)
            | Expr::Unary(_, _, s) | Expr::Call(_, _, s) | Expr::Member(_, _, s)
            | Expr::Arrow(_, _, s) | Expr::Index(_, _, s) | Expr::Cast(_, _, s)
            | Expr::Sizeof(_, s) | Expr::Cond(_, _, _, s)
            | Expr::InitList(_, s) | Expr::StmtExpr(_, _, s) => *s,
        }
    }
}

/// Statement node
#[derive(Debug, Clone)]
pub enum Stmt {
    Return(Option<Expr>, Span),
    Expr(Expr, Span),
    Block(Vec<Stmt>, Span),
    If(Expr, Box<Stmt>, Option<Box<Stmt>>, Span),
    While(Expr, Box<Stmt>, Span),
    DoWhile(Box<Stmt>, Expr, Span),
    For(Option<Box<Stmt>>, Option<Expr>, Option<Expr>, Box<Stmt>, Span),
    Switch(Expr, Box<Stmt>, Span),
    Case(i64, Box<Stmt>, Span),
    Default(Box<Stmt>, Span),
    Break(Span),
    Continue(Span),
    Goto(String, Span),
    Label(String, Box<Stmt>, Span),
    VarDecl(VarDecl, Span),
    Null,
}

/// Variable declaration
#[derive(Debug, Clone)]
pub struct VarDecl {
    pub name: String,
    pub ty: Type,
    pub init: Option<Expr>,
    pub is_static: bool,
    pub is_extern: bool,
}

/// Top-level declaration
#[derive(Debug, Clone)]
pub enum TopLevel {
    FuncDef {
        name: String,
        return_ty: Type,
        params: Vec<(Type, String)>,
        body: Stmt,
        is_static: bool,
        span: Span,
    },
    GlobalVar(VarDecl, Span),
    FuncDecl {
        name: String,
        return_ty: Type,
        params: Vec<(Type, String)>,
        is_variadic: bool,
        span: Span,
    },
}

/// The entire translation unit
#[derive(Debug)]
pub struct TranslationUnit {
    pub decls: Vec<TopLevel>,
    pub struct_types: std::collections::HashMap<String, Type>,
    pub typedefs: std::collections::HashMap<String, Type>,
}
