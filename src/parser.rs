use crate::ast::*;
use crate::error::{CompileError, Span, did_you_mean};
use crate::lexer::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    struct_types: std::collections::HashMap<String, Type>,
    typedefs: std::collections::HashMap<String, Type>,
    enum_values: std::collections::HashMap<String, i64>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens, pos: 0,
            struct_types: std::collections::HashMap::new(),
            typedefs: std::collections::HashMap::new(),
            enum_values: std::collections::HashMap::new(),
        }
    }

    // ── Token access ──

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn at_eof(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos];
        if !self.at_eof() {
            self.pos += 1;
        }
        tok
    }

    fn span(&self) -> Span {
        self.peek().span
    }

    fn is_punct(&self, s: &str) -> bool {
        matches!(&self.peek().kind, TokenKind::Punct(p) if p == s)
    }

    fn is_keyword(&self, s: &str) -> bool {
        matches!(&self.peek().kind, TokenKind::Keyword(k) if k == s)
    }

    fn eat_punct(&mut self, s: &str) -> bool {
        if self.is_punct(s) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn eat_keyword(&mut self, s: &str) -> bool {
        if self.is_keyword(s) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_punct(&mut self, s: &str) -> Result<(), CompileError> {
        if !self.eat_punct(s) {
            return Err(CompileError::new(
                format!("expected '{}'", s),
                self.span(),
            ));
        }
        Ok(())
    }

    fn expect_ident(&mut self) -> Result<(String, Span), CompileError> {
        let tok = self.advance();
        match &tok.kind {
            TokenKind::Ident(name) => Ok((name.clone(), tok.span)),
            _ => Err(CompileError::new("expected identifier", tok.span)),
        }
    }

    // ── Parsing: types ──

    fn is_typename(&self) -> bool {
        if matches!(&self.peek().kind,
            TokenKind::Keyword(k) if matches!(k.as_str(),
                "void" | "char" | "short" | "int" | "long" | "float" | "double"
                | "signed" | "unsigned" | "struct" | "union" | "enum"
                | "static" | "extern" | "const" | "volatile"
                | "_Bool" | "typedef" | "inline" | "_Noreturn" | "typeof"
                | "_Alignas" | "_Atomic" | "_Thread_local"
                | "auto" | "register"
            )
        ) {
            return true;
        }
        // Check typedefs
        if let TokenKind::Ident(name) = &self.peek().kind {
            return self.typedefs.contains_key(name);
        }
        false
    }

    /// Returns (type, is_static, is_extern, is_typedef)
    fn parse_base_type_full(&mut self) -> Result<(Type, bool, bool, bool), CompileError> {
        let mut is_static = false;
        let mut is_extern = false;
        let mut is_typedef = false;
        let mut is_signed = true;
        let mut has_signed_explicit = false;
        let mut has_unsigned = false;
        let mut has_qualifier = false;
        let mut base = None;
        let mut long_count = 0;

        loop {
            if self.eat_keyword("static") {
                is_static = true;
                continue;
            }
            if self.eat_keyword("extern") {
                is_extern = true;
                continue;
            }
            if self.eat_keyword("typedef") {
                is_typedef = true;
                continue;
            }
            if self.eat_keyword("const") || self.eat_keyword("volatile")
                || self.eat_keyword("inline") || self.eat_keyword("_Noreturn")
                || self.eat_keyword("__cdecl") || self.eat_keyword("__stdcall")
                || self.eat_keyword("__restrict") || self.eat_keyword("__restrict__")
                || self.eat_keyword("__inline") || self.eat_keyword("restrict")
                || self.eat_keyword("auto") || self.eat_keyword("register")
                || self.eat_keyword("_Atomic")
                || self.eat_keyword("_Thread_local") || self.eat_keyword("__thread")
            {
                has_qualifier = true;
                continue;
            }
            // Skip _Alignas(value)
            if self.eat_keyword("_Alignas") {
                if self.eat_punct("(") {
                    let mut depth = 1;
                    while depth > 0 && !self.at_eof() {
                        if self.eat_punct("(") { depth += 1; }
                        else if self.eat_punct(")") { depth -= 1; }
                        else { self.advance(); }
                    }
                }
                continue;
            }
            // Skip __attribute__((..)) and __declspec(..)
            if self.eat_keyword("__attribute__") || self.eat_keyword("__declspec") {
                if self.eat_punct("(") {
                    let mut depth = 1;
                    while depth > 0 && !self.at_eof() {
                        if self.eat_punct("(") { depth += 1; }
                        else if self.eat_punct(")") { depth -= 1; }
                        else { self.advance(); }
                    }
                }
                continue;
            }
            if self.eat_keyword("signed") {
                is_signed = true;
                has_signed_explicit = true;
                continue;
            }
            if self.eat_keyword("unsigned") {
                has_unsigned = true;
                is_signed = false;
                continue;
            }
            if self.eat_keyword("long") {
                long_count += 1;
                continue;
            }
            if self.eat_keyword("short") {
                base = Some(Type::Short);
                continue;
            }
            if self.eat_keyword("void") { base = Some(Type::Void); continue; }
            if self.eat_keyword("_Bool") { base = Some(Type::Bool); continue; }
            if self.eat_keyword("char") { base = Some(Type::Char); continue; }
            if self.eat_keyword("int") { base = Some(Type::Int); continue; }
            if self.eat_keyword("float") { base = Some(Type::Float); continue; }
            if self.eat_keyword("double") { base = Some(Type::Double); continue; }
            if self.is_keyword("struct") { base = Some(self.parse_struct_type()?); continue; }
            if self.is_keyword("union") { base = Some(self.parse_union_type()?); continue; }
            if self.is_keyword("enum") { base = Some(self.parse_enum_type()?); continue; }
            if self.eat_keyword("typeof") {
                self.expect_punct("(")?;
                if self.is_typename() {
                    let (ty, _, _) = self.parse_base_type()?;
                    self.expect_punct(")")?;
                    base = Some(ty);
                } else {
                    // typeof(expr) — skip expression, default to int
                    let mut depth = 1;
                    while depth > 0 && !self.at_eof() {
                        if self.eat_punct("(") { depth += 1; }
                        else if self.eat_punct(")") { depth -= 1; }
                        else { self.advance(); }
                    }
                    base = Some(Type::Int);
                }
                continue;
            }
            // Check typedef names (only if no base type set yet)
            if base.is_none() {
                if let TokenKind::Ident(name) = &self.peek().kind {
                    if let Some(ty) = self.typedefs.get(name).cloned() {
                        self.advance();
                        base = Some(ty);
                        continue;
                    }
                }
            }
            break;
        }

        let ty = if long_count >= 1 {
            if has_unsigned { Type::ULong } else { Type::Long }
        } else if let Some(ty) = base {
            // Apply unsigned modifier
            if has_unsigned {
                match ty {
                    Type::Char => Type::UChar,
                    Type::Short => Type::UShort,
                    Type::Int => Type::UInt,
                    Type::Long => Type::ULong,
                    other => other,
                }
            } else {
                ty
            }
        } else if has_unsigned {
            Type::UInt
        } else if is_typedef || has_qualifier || is_static || is_extern || has_signed_explicit {
            // const x = 5; or signed; or static x; — defaults to int
            Type::Int
        } else {
            return Err(CompileError::new("expected type", self.span()));
        };

        let _ = is_signed; // TODO: unsigned types
        Ok((ty, is_static, is_extern, is_typedef))
    }

    fn parse_base_type(&mut self) -> Result<(Type, bool, bool), CompileError> {
        let (ty, is_static, is_extern, _is_typedef) = self.parse_base_type_full()?;
        Ok((ty, is_static, is_extern))
    }

    fn parse_struct_type(&mut self) -> Result<Type, CompileError> {
        self.expect_keyword("struct")?;
        // Skip __attribute__((..)) after struct keyword
        while self.eat_keyword("__attribute__") {
            if self.eat_punct("(") {
                let mut depth = 1;
                while depth > 0 && !self.at_eof() {
                    if self.eat_punct("(") { depth += 1; }
                    else if self.eat_punct(")") { depth -= 1; }
                    else { self.advance(); }
                }
            }
        }
        let name = if let TokenKind::Ident(_) = &self.peek().kind {
            let (n, _) = self.expect_ident()?;
            Some(n)
        } else {
            None
        };

        // Skip __attribute__((..)) before {
        while self.eat_keyword("__attribute__") {
            if self.eat_punct("(") {
                let mut depth = 1;
                while depth > 0 && !self.at_eof() {
                    if self.eat_punct("(") { depth += 1; }
                    else if self.eat_punct(")") { depth -= 1; }
                    else { self.advance(); }
                }
            }
        }

        if self.eat_punct("{") {
            // Pre-register struct name for self-referential types
            if let Some(ref n) = name {
                if !self.struct_types.contains_key(n) {
                    self.struct_types.insert(n.clone(), Type::Struct { name: name.clone(), members: Vec::new() });
                }
            }
            let mut members = Vec::new();
            let mut offset = 0;
            while !self.is_punct("}") {
                let (ty, _, _) = self.parse_base_type()?;
                let mut ty = ty;
                loop {
                    if self.eat_punct("*") { ty = Type::Ptr(Box::new(ty)); }
                    else if self.eat_keyword("const") || self.eat_keyword("volatile") {}
                    else { break; }
                }

                // Handle function pointer: type (*name)(params)
                let mname = if self.is_punct("(") {
                    self.advance(); // skip (
                    // Skip CJSON_CDECL etc
                    while self.is_keyword("__cdecl") || self.is_keyword("__stdcall") {
                        self.advance();
                    }
                    while self.eat_punct("*") { ty = Type::Ptr(Box::new(ty)); }
                    let (n, _) = self.expect_ident()?;
                    self.expect_punct(")")?;
                    // Skip function params
                    if self.eat_punct("(") {
                        let mut depth = 1;
                        while depth > 0 && !self.at_eof() {
                            if self.eat_punct("(") { depth += 1; }
                            else if self.eat_punct(")") { depth -= 1; }
                            else { self.advance(); }
                        }
                    }
                    ty = Type::Ptr(Box::new(Type::Void)); // treat fn ptr as void*
                    n
                } else if self.is_punct(":") || self.is_punct(";") {
                    // Anonymous bitfield or anonymous struct/union member
                    String::new()
                } else {
                    let (n, _) = self.expect_ident()?;
                    n
                };

                // Handle bitfield: int x : 3;
                if self.eat_punct(":") {
                    // Skip bitfield width expression
                    let _ = self.parse_ternary()?;
                }

                let mty = self.parse_type_suffix(ty.clone())?;
                let align = mty.size().max(1);
                offset = (offset + align - 1) / align * align;
                members.push(StructMember { name: mname, ty: mty.clone(), offset });
                offset += mty.size();

                // Handle comma-separated members: int a, b;
                while self.eat_punct(",") {
                    let mut extra_ty = ty.clone();
                    while self.eat_punct("*") { extra_ty = Type::Ptr(Box::new(extra_ty)); }
                    let (ename, _) = self.expect_ident()?;
                    // Bitfield in comma list
                    if self.eat_punct(":") { let _ = self.parse_ternary()?; }
                    let ety = self.parse_type_suffix(extra_ty)?;
                    let ealign = ety.size().max(1);
                    offset = (offset + ealign - 1) / ealign * ealign;
                    members.push(StructMember { name: ename, ty: ety.clone(), offset });
                    offset += ety.size();
                }
                self.expect_punct(";")?;
            }
            self.expect_punct("}")?;
            let ty = Type::Struct { name: name.clone(), members };
            if let Some(ref n) = name {
                self.struct_types.insert(n.clone(), ty.clone());
            }
            Ok(ty)
        } else {
            // Forward reference: look up previously defined struct
            if let Some(ref n) = name {
                if let Some(ty) = self.struct_types.get(n).cloned() {
                    return Ok(ty);
                }
            }
            // Unknown struct — return empty
            Ok(Type::Struct { name, members: Vec::new() })
        }
    }

    fn parse_union_type(&mut self) -> Result<Type, CompileError> {
        self.expect_keyword("union")?;
        let name = if let TokenKind::Ident(_) = &self.peek().kind {
            let (n, _) = self.expect_ident()?;
            Some(n)
        } else {
            None
        };

        let members = if self.eat_punct("{") {
            let mut members = Vec::new();
            while !self.is_punct("}") {
                let (base_ty, _, _) = self.parse_base_type()?;
                let mut ty = base_ty.clone();
                while self.eat_punct("*") { ty = Type::Ptr(Box::new(ty)); }
                // Anonymous struct/union member or anonymous bitfield
                if self.is_punct(";") || self.is_punct(":") {
                    if self.eat_punct(":") { let _ = self.parse_ternary()?; }
                    self.expect_punct(";")?;
                    // Anonymous member — add with empty name
                    members.push(StructMember { name: String::new(), ty, offset: 0 });
                    continue;
                }
                let (mname, _) = self.expect_ident()?;
                if self.eat_punct(":") { let _ = self.parse_ternary()?; }
                let ty = self.parse_type_suffix(ty)?;
                members.push(StructMember { name: mname, ty, offset: 0 });
                while self.eat_punct(",") {
                    let mut ety = base_ty.clone();
                    while self.eat_punct("*") { ety = Type::Ptr(Box::new(ety)); }
                    let (en, _) = self.expect_ident()?;
                    if self.eat_punct(":") { let _ = self.parse_ternary()?; }
                    let ety = self.parse_type_suffix(ety)?;
                    members.push(StructMember { name: en, ty: ety, offset: 0 });
                }
                self.expect_punct(";")?;
            }
            self.expect_punct("}")?;
            members
        } else {
            Vec::new()
        };

        Ok(Type::Union { name, members })
    }

    fn parse_enum_type(&mut self) -> Result<Type, CompileError> {
        self.expect_keyword("enum")?;
        let name = if let TokenKind::Ident(_) = &self.peek().kind {
            let (n, _) = self.expect_ident()?;
            Some(n)
        } else {
            None
        };
        if self.eat_punct("{") {
            let mut val: i64 = 0;
            while !self.is_punct("}") {
                let (ename, _) = self.expect_ident()?;
                if self.eat_punct("=") {
                    // Parse constant expression for enum value
                    let expr = self.parse_ternary()?;
                    if let Some(v) = eval_const_expr(&expr) {
                        val = v;
                    }
                }
                self.enum_values.insert(ename, val);
                val += 1;
                if !self.eat_punct(",") {
                    break;
                }
            }
            self.expect_punct("}")?;
        }
        Ok(Type::Enum(name))
    }

    fn expect_keyword(&mut self, s: &str) -> Result<(), CompileError> {
        if !self.eat_keyword(s) {
            return Err(CompileError::new(format!("expected '{}'", s), self.span()));
        }
        Ok(())
    }

    /// Parse pointer stars and array brackets after a base type
    fn parse_declarator(&mut self, base: Type) -> Result<(Type, String), CompileError> {
        let mut ty = base;
        loop {
            if self.eat_punct("*") {
                ty = Type::Ptr(Box::new(ty));
            } else if self.eat_keyword("const") || self.eat_keyword("volatile")
                || self.eat_keyword("__restrict") || self.eat_keyword("__restrict__")
                || self.eat_keyword("restrict") {
                // skip qualifiers
            } else if self.eat_keyword("_Alignas") || self.eat_keyword("__attribute__") {
                // skip _Alignas(n) and __attribute__((...))
                if self.eat_punct("(") {
                    let mut depth = 1;
                    while depth > 0 && !self.at_eof() {
                        if self.eat_punct("(") { depth += 1; }
                        else if self.eat_punct(")") { depth -= 1; }
                        else { self.advance(); }
                    }
                }
            } else {
                break;
            }
        }
        // Handle grouped declarator: (*name), (*name)[size], (name[3])[4], (*name(params))(params)
        let name = if self.eat_punct("(") {
            while self.eat_punct("*") { ty = Type::Ptr(Box::new(ty)); }
            let (n, _) = self.expect_ident()?;
            // Parse inner suffix: array [3] or function params (...)
            if self.is_punct("[") {
                let inner_ty = self.parse_type_suffix(Type::Void)?;
                if let Type::Array(_, _) = inner_ty {
                    ty = self.rebuild_array_type(ty, &inner_ty);
                }
            } else if self.is_punct("(") {
                // Function params inside grouped declarator: (*fnptr(params))
                self.advance();
                let mut depth = 1;
                while depth > 0 && !self.at_eof() {
                    if self.eat_punct("(") { depth += 1; }
                    else if self.eat_punct(")") { depth -= 1; }
                    else { self.advance(); }
                }
                ty = Type::Ptr(Box::new(ty)); // treat as function pointer
            }
            self.expect_punct(")")?;
            // Parse outer suffix: array or function params
            if self.is_punct("(") {
                // Outer function params: return type's params
                self.advance();
                let mut depth = 1;
                while depth > 0 && !self.at_eof() {
                    if self.eat_punct("(") { depth += 1; }
                    else if self.eat_punct(")") { depth -= 1; }
                    else { self.advance(); }
                }
            }
            ty = self.parse_type_suffix(ty)?;
            n
        } else {
            let (n, _) = self.expect_ident()?;
            ty = self.parse_type_suffix(ty)?;
            n
        };
        Ok((ty, name))
    }

    fn rebuild_array_type(&self, base: Type, inner: &Type) -> Type {
        // For (name[3])[4], the inner is Array(Void, 3), outer is Array(base, 4)
        // Result should be Array(Array(base, 4), 3)
        match inner {
            Type::Array(_, size) => Type::Array(Box::new(base), *size),
            _ => base,
        }
    }

    fn parse_type_suffix(&mut self, base: Type) -> Result<Type, CompileError> {
        let mut ty = base;
        while self.eat_punct("[") {
            if self.is_punct("]") {
                // Empty brackets: int arr[] — size 0 (inferred later)
                self.advance();
                ty = Type::Array(Box::new(ty), 0);
                continue;
            }
            // Try to parse as constant expression for array size
            let expr = self.parse_ternary()?;
            let size = eval_const_expr(&expr).unwrap_or(0) as usize;
            self.expect_punct("]")?;
            ty = Type::Array(Box::new(ty), size);
        }
        Ok(ty)
    }

    // ── Parsing: expressions ──

    pub fn parse_expr(&mut self) -> Result<Expr, CompileError> {
        let mut lhs = self.parse_assign()?;
        while self.eat_punct(",") {
            let span = self.span();
            let rhs = self.parse_assign()?;
            lhs = Expr::Binary(BinOp::Comma, Box::new(lhs), Box::new(rhs), span);
        }
        Ok(lhs)
    }

    fn parse_assign(&mut self) -> Result<Expr, CompileError> {
        let mut lhs = self.parse_ternary()?;

        let op = if self.is_punct("=") { Some(BinOp::Assign) }
            else if self.is_punct("+=") { Some(BinOp::AddAssign) }
            else if self.is_punct("-=") { Some(BinOp::SubAssign) }
            else if self.is_punct("*=") { Some(BinOp::MulAssign) }
            else if self.is_punct("/=") { Some(BinOp::DivAssign) }
            else if self.is_punct("%=") { Some(BinOp::ModAssign) }
            else if self.is_punct("&=") { Some(BinOp::BitAndAssign) }
            else if self.is_punct("|=") { Some(BinOp::BitOrAssign) }
            else if self.is_punct("^=") { Some(BinOp::BitXorAssign) }
            else if self.is_punct("<<=") { Some(BinOp::ShlAssign) }
            else if self.is_punct(">>=") { Some(BinOp::ShrAssign) }
            else { None };

        if let Some(op) = op {
            let span = self.span();
            self.advance();
            let rhs = self.parse_assign()?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs), span);
        }

        Ok(lhs)
    }

    fn parse_ternary(&mut self) -> Result<Expr, CompileError> {
        let cond = self.parse_logor()?;
        if self.eat_punct("?") {
            let span = self.span();
            // GNU extension: a ?: b means a ? a : b
            let then = if self.is_punct(":") {
                cond.clone()
            } else {
                self.parse_expr()?
            };
            self.expect_punct(":")?;
            let els = self.parse_ternary()?;
            Ok(Expr::Cond(Box::new(cond), Box::new(then), Box::new(els), span))
        } else {
            Ok(cond)
        }
    }

    fn parse_logor(&mut self) -> Result<Expr, CompileError> {
        let mut lhs = self.parse_logand()?;
        while self.is_punct("||") {
            let span = self.span();
            self.advance();
            let rhs = self.parse_logand()?;
            lhs = Expr::Binary(BinOp::LogOr, Box::new(lhs), Box::new(rhs), span);
        }
        Ok(lhs)
    }

    fn parse_logand(&mut self) -> Result<Expr, CompileError> {
        let mut lhs = self.parse_bitor()?;
        while self.is_punct("&&") {
            let span = self.span();
            self.advance();
            let rhs = self.parse_bitor()?;
            lhs = Expr::Binary(BinOp::LogAnd, Box::new(lhs), Box::new(rhs), span);
        }
        Ok(lhs)
    }

    fn parse_bitor(&mut self) -> Result<Expr, CompileError> {
        let mut lhs = self.parse_bitxor()?;
        while self.is_punct("|") && !self.is_punct("||") {
            let span = self.span();
            self.advance();
            let rhs = self.parse_bitxor()?;
            lhs = Expr::Binary(BinOp::BitOr, Box::new(lhs), Box::new(rhs), span);
        }
        Ok(lhs)
    }

    fn parse_bitxor(&mut self) -> Result<Expr, CompileError> {
        let mut lhs = self.parse_bitand()?;
        while self.is_punct("^") && !self.is_punct("^=") {
            let span = self.span();
            self.advance();
            let rhs = self.parse_bitand()?;
            lhs = Expr::Binary(BinOp::BitXor, Box::new(lhs), Box::new(rhs), span);
        }
        Ok(lhs)
    }

    fn parse_bitand(&mut self) -> Result<Expr, CompileError> {
        let mut lhs = self.parse_equality()?;
        while self.is_punct("&") && !self.is_punct("&&") && !self.is_punct("&=") {
            let span = self.span();
            self.advance();
            let rhs = self.parse_equality()?;
            lhs = Expr::Binary(BinOp::BitAnd, Box::new(lhs), Box::new(rhs), span);
        }
        Ok(lhs)
    }

    fn parse_equality(&mut self) -> Result<Expr, CompileError> {
        let mut lhs = self.parse_relational()?;
        loop {
            if self.is_punct("==") {
                let span = self.span();
                self.advance();
                let rhs = self.parse_relational()?;
                lhs = Expr::Binary(BinOp::Eq, Box::new(lhs), Box::new(rhs), span);
            } else if self.is_punct("!=") {
                let span = self.span();
                self.advance();
                let rhs = self.parse_relational()?;
                lhs = Expr::Binary(BinOp::Ne, Box::new(lhs), Box::new(rhs), span);
            } else {
                break;
            }
        }
        Ok(lhs)
    }

    fn parse_relational(&mut self) -> Result<Expr, CompileError> {
        let mut lhs = self.parse_shift()?;
        loop {
            if self.is_punct("<=") {
                let span = self.span(); self.advance();
                let rhs = self.parse_shift()?;
                lhs = Expr::Binary(BinOp::Le, Box::new(lhs), Box::new(rhs), span);
            } else if self.is_punct(">=") {
                let span = self.span(); self.advance();
                let rhs = self.parse_shift()?;
                lhs = Expr::Binary(BinOp::Ge, Box::new(lhs), Box::new(rhs), span);
            } else if self.is_punct("<") && !self.is_punct("<<") {
                let span = self.span(); self.advance();
                let rhs = self.parse_shift()?;
                lhs = Expr::Binary(BinOp::Lt, Box::new(lhs), Box::new(rhs), span);
            } else if self.is_punct(">") && !self.is_punct(">>") {
                let span = self.span(); self.advance();
                let rhs = self.parse_shift()?;
                lhs = Expr::Binary(BinOp::Gt, Box::new(lhs), Box::new(rhs), span);
            } else {
                break;
            }
        }
        Ok(lhs)
    }

    fn parse_shift(&mut self) -> Result<Expr, CompileError> {
        let mut lhs = self.parse_add()?;
        loop {
            if self.is_punct("<<") && !self.is_punct("<<=") {
                let span = self.span(); self.advance();
                let rhs = self.parse_add()?;
                lhs = Expr::Binary(BinOp::Shl, Box::new(lhs), Box::new(rhs), span);
            } else if self.is_punct(">>") && !self.is_punct(">>=") {
                let span = self.span(); self.advance();
                let rhs = self.parse_add()?;
                lhs = Expr::Binary(BinOp::Shr, Box::new(lhs), Box::new(rhs), span);
            } else {
                break;
            }
        }
        Ok(lhs)
    }

    fn parse_add(&mut self) -> Result<Expr, CompileError> {
        let mut lhs = self.parse_mul()?;
        loop {
            if self.is_punct("+") && !self.is_punct("++") && !self.is_punct("+=") {
                let span = self.span(); self.advance();
                let rhs = self.parse_mul()?;
                lhs = Expr::Binary(BinOp::Add, Box::new(lhs), Box::new(rhs), span);
            } else if self.is_punct("-") && !self.is_punct("--") && !self.is_punct("-=") && !self.is_punct("->") {
                let span = self.span(); self.advance();
                let rhs = self.parse_mul()?;
                lhs = Expr::Binary(BinOp::Sub, Box::new(lhs), Box::new(rhs), span);
            } else {
                break;
            }
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> Result<Expr, CompileError> {
        let mut lhs = self.parse_unary()?;
        loop {
            if self.is_punct("*") && !self.is_punct("*=") {
                let span = self.span(); self.advance();
                let rhs = self.parse_unary()?;
                lhs = Expr::Binary(BinOp::Mul, Box::new(lhs), Box::new(rhs), span);
            } else if self.is_punct("/") && !self.is_punct("/=") {
                let span = self.span(); self.advance();
                let rhs = self.parse_unary()?;
                lhs = Expr::Binary(BinOp::Div, Box::new(lhs), Box::new(rhs), span);
            } else if self.is_punct("%") && !self.is_punct("%=") {
                let span = self.span(); self.advance();
                let rhs = self.parse_unary()?;
                lhs = Expr::Binary(BinOp::Mod, Box::new(lhs), Box::new(rhs), span);
            } else {
                break;
            }
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, CompileError> {
        // __builtin_types_compatible_p(type1, type2)
        if let TokenKind::Ident(name) = &self.peek().kind {
            if name.starts_with("__builtin_") {
                let name = name.clone();
                let span = self.span();
                self.advance();
                if self.eat_punct("(") {
                    // Skip all arguments (may contain types)
                    let mut depth = 1;
                    while depth > 0 && !self.at_eof() {
                        if self.eat_punct("(") { depth += 1; }
                        else if self.eat_punct(")") { depth -= 1; }
                        else { self.advance(); }
                    }
                }
                // Return 0 as default for unknown builtins
                return Ok(Expr::IntLit(0, span));
            }
        }
        // Labels-as-values: &&label (GCC extension)
        if self.is_punct("&&") {
            let span = self.span(); self.advance();
            let (name, _) = self.expect_ident()?;
            // Return as address of label — treat as 0 for now
            return Ok(Expr::IntLit(0, span));
        }
        // Unary + (no-op, just skip)
        if self.is_punct("+") && !self.is_punct("++") && !self.is_punct("+=") {
            self.advance();
            return self.parse_unary();
        }
        if self.is_punct("-") && !self.is_punct("--") && !self.is_punct("-=") && !self.is_punct("->") {
            let span = self.span(); self.advance();
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary(UnaryOp::Neg, Box::new(expr), span));
        }
        if self.is_punct("!") && !self.is_punct("!=") {
            let span = self.span(); self.advance();
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary(UnaryOp::Not, Box::new(expr), span));
        }
        if self.is_punct("~") {
            let span = self.span(); self.advance();
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary(UnaryOp::BitNot, Box::new(expr), span));
        }
        if self.is_punct("&") && !self.is_punct("&&") && !self.is_punct("&=") {
            let span = self.span(); self.advance();
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary(UnaryOp::Addr, Box::new(expr), span));
        }
        if self.is_punct("*") && !self.is_punct("*=") {
            let span = self.span(); self.advance();
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary(UnaryOp::Deref, Box::new(expr), span));
        }
        if self.is_punct("++") {
            let span = self.span(); self.advance();
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary(UnaryOp::PreInc, Box::new(expr), span));
        }
        if self.is_punct("--") {
            let span = self.span(); self.advance();
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary(UnaryOp::PreDec, Box::new(expr), span));
        }
        // Cast expression: (type)expr
        if self.is_punct("(") {
            // Look ahead: is the next token a type name?
            let saved_pos = self.pos;
            self.advance(); // skip (
            if self.is_typename() {
                let span = self.span();
                let (ty, _, _) = self.parse_base_type()?;
                let mut ty = ty;
                // Handle pointer and array: (int *), (int[]), (int[3])
                loop {
                    if self.eat_punct("*") { ty = Type::Ptr(Box::new(ty)); }
                    else if self.eat_keyword("const") || self.eat_keyword("volatile") {}
                    else { break; }
                }
                // Handle array suffix: (int[3]) or (int[])
                ty = self.parse_type_suffix(ty)?;
                if self.eat_punct(")") {
                    // Check for compound literal: (Type){init}
                    if self.is_punct("{") {
                        let items = self.parse_init_list()?;
                        return Ok(Expr::InitList(items, span));
                    }
                    // It's a cast
                    let expr = self.parse_unary()?;
                    return Ok(Expr::Cast(ty, Box::new(expr), span));
                }
            }
            // Not a cast, restore position
            self.pos = saved_pos;
        }
        // _Generic(expr, type: val, type: val, default: val)
        if self.eat_keyword("_Generic") {
            let span = self.span();
            self.expect_punct("(")?;
            let _ctrl = self.parse_assign()?; // control expression
            self.expect_punct(",")?;
            // Parse associations — return first non-default value
            let mut result = None;
            loop {
                if self.eat_keyword("default") {
                    self.expect_punct(":")?;
                    let val = self.parse_assign()?;
                    if result.is_none() { result = Some(val); }
                } else if self.is_typename() {
                    let _ = self.parse_base_type()?;
                    while self.eat_punct("*") {}
                    self.expect_punct(":")?;
                    let val = self.parse_assign()?;
                    if result.is_none() { result = Some(val); }
                } else {
                    break;
                }
                if !self.eat_punct(",") { break; }
            }
            self.expect_punct(")")?;
            return Ok(result.unwrap_or(Expr::IntLit(0, span)));
        }
        if self.eat_keyword("sizeof") || self.eat_keyword("_Alignof") || self.eat_keyword("__alignof__") {
            let span = self.span();
            if self.eat_punct("(") {
                if self.is_typename() {
                    let (ty, _, _) = self.parse_base_type()?;
                    let mut ty = ty;
                    while self.eat_punct("*") { ty = Type::Ptr(Box::new(ty)); }
                    // Handle complex: sizeof(int(*)[4]) — grouped pointer+array
                    if self.is_punct("(") {
                        // Parse grouped: (*) or (*)[size]
                        self.advance();
                        while self.eat_punct("*") { ty = Type::Ptr(Box::new(ty)); }
                        if !self.is_punct(")") { self.advance(); } // skip identifier if present
                        self.expect_punct(")")?;
                        ty = self.parse_type_suffix(ty)?;
                    } else {
                        ty = self.parse_type_suffix(ty)?;
                    }
                    self.expect_punct(")")?;
                    return Ok(Expr::Sizeof(Box::new(SizeofArg::Type(ty)), span));
                }
                let expr = self.parse_expr()?;
                self.expect_punct(")")?;
                return Ok(Expr::Sizeof(Box::new(SizeofArg::Expr(expr)), span));
            }
            let expr = self.parse_unary()?;
            return Ok(Expr::Sizeof(Box::new(SizeofArg::Expr(expr)), span));
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, CompileError> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.eat_punct("[") {
                let span = self.span();
                let idx = self.parse_expr()?;
                self.expect_punct("]")?;
                expr = Expr::Index(Box::new(expr), Box::new(idx), span);
                continue;
            }
            if self.eat_punct("(") {
                let span = self.span();
                let mut args = Vec::new();
                if !self.is_punct(")") {
                    args.push(self.parse_assign()?);
                    while self.eat_punct(",") {
                        if self.is_punct(")") { break; } // trailing comma
                        args.push(self.parse_assign()?);
                    }
                }
                self.expect_punct(")")?;
                expr = Expr::Call(Box::new(expr), args, span);
                continue;
            }
            if self.eat_punct(".") {
                let span = self.span();
                let (member, _) = self.expect_ident()?;
                expr = Expr::Member(Box::new(expr), member, span);
                continue;
            }
            if self.eat_punct("->") {
                let span = self.span();
                let (member, _) = self.expect_ident()?;
                expr = Expr::Arrow(Box::new(expr), member, span);
                continue;
            }
            if self.is_punct("++") {
                let span = self.span(); self.advance();
                expr = Expr::Unary(UnaryOp::PostInc, Box::new(expr), span);
                continue;
            }
            if self.is_punct("--") {
                let span = self.span(); self.advance();
                expr = Expr::Unary(UnaryOp::PostDec, Box::new(expr), span);
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, CompileError> {
        let tok = self.peek().clone();
        match &tok.kind {
            TokenKind::Int(n) => {
                let n = *n;
                self.advance();
                Ok(Expr::IntLit(n, tok.span))
            }
            TokenKind::Float(f) => {
                let f = *f;
                self.advance();
                Ok(Expr::FloatLit(f, tok.span))
            }
            TokenKind::Str(s) => {
                let mut s = s.clone();
                self.advance();
                // String concatenation: "hello" " world" → "hello world"
                while let TokenKind::Str(next) = &self.peek().kind {
                    s.push_str(next);
                    self.advance();
                }
                Ok(Expr::StrLit(s, tok.span))
            }
            TokenKind::Char(c) => {
                let c = *c;
                self.advance();
                Ok(Expr::CharLit(c, tok.span))
            }
            TokenKind::Ident(name) => {
                let name = name.clone();
                // Check if it's an enum constant
                if let Some(&val) = self.enum_values.get(&name) {
                    self.advance();
                    return Ok(Expr::IntLit(val, tok.span));
                }
                self.advance();
                Ok(Expr::Var(name, tok.span))
            }
            TokenKind::Punct(p) if p == "(" => {
                self.advance();
                // Statement expression: ({ stmt; stmt; expr })
                if self.is_punct("{") {
                    let span = self.span();
                    self.advance(); // skip {
                    let mut stmts = Vec::new();
                    while !self.is_punct("}") && !self.at_eof() {
                        stmts.push(self.parse_stmt()?);
                    }
                    self.expect_punct("}")?;
                    self.expect_punct(")")?;
                    // Last statement should be an expression
                    let last_expr = match stmts.pop() {
                        Some(Stmt::Expr(e, _)) => e,
                        Some(Stmt::Return(Some(e), _)) => e,
                        Some(other) => {
                            stmts.push(other);
                            Expr::IntLit(0, span)
                        }
                        None => Expr::IntLit(0, span),
                    };
                    return Ok(Expr::StmtExpr(stmts, Box::new(last_expr), span));
                }
                let expr = self.parse_expr()?;
                self.expect_punct(")")?;
                Ok(expr)
            }
            _ => Err(CompileError::new(
                format!("unexpected token: {:?}", tok.kind),
                tok.span,
            )),
        }
    }

    // ── Parsing: statements ──

    fn parse_stmt(&mut self) -> Result<Stmt, CompileError> {
        if self.eat_punct(";") {
            return Ok(Stmt::Null);
        }

        if self.is_punct("{") {
            return self.parse_block();
        }

        if self.is_keyword("return") {
            let span = self.span();
            self.advance();
            if self.eat_punct(";") {
                return Ok(Stmt::Return(None, span));
            }
            let expr = self.parse_expr()?;
            self.expect_punct(";")?;
            return Ok(Stmt::Return(Some(expr), span));
        }

        if self.is_keyword("if") {
            let span = self.span();
            self.advance();
            self.expect_punct("(")?;
            let cond = self.parse_expr()?;
            self.expect_punct(")")?;
            let then = Box::new(self.parse_stmt()?);
            let els = if self.eat_keyword("else") {
                Some(Box::new(self.parse_stmt()?))
            } else {
                None
            };
            return Ok(Stmt::If(cond, then, els, span));
        }

        if self.is_keyword("while") {
            let span = self.span();
            self.advance();
            self.expect_punct("(")?;
            let cond = self.parse_expr()?;
            self.expect_punct(")")?;
            let body = Box::new(self.parse_stmt()?);
            return Ok(Stmt::While(cond, body, span));
        }

        if self.is_keyword("do") {
            let span = self.span();
            self.advance();
            let body = Box::new(self.parse_stmt()?);
            self.expect_keyword("while")?;
            self.expect_punct("(")?;
            let cond = self.parse_expr()?;
            self.expect_punct(")")?;
            self.expect_punct(";")?;
            return Ok(Stmt::DoWhile(body, cond, span));
        }

        if self.is_keyword("for") {
            let span = self.span();
            self.advance();
            self.expect_punct("(")?;

            let init = if self.eat_punct(";") {
                None
            } else if self.is_typename() {
                let decl = self.parse_var_decl()?;
                Some(Box::new(decl))
            } else {
                let expr = self.parse_expr()?;
                self.expect_punct(";")?;
                Some(Box::new(Stmt::Expr(expr, span)))
            };

            let cond = if self.is_punct(";") {
                None
            } else {
                Some(self.parse_expr()?)
            };
            self.expect_punct(";")?;

            let inc = if self.is_punct(")") {
                None
            } else {
                Some(self.parse_expr()?)
            };
            self.expect_punct(")")?;

            let body = Box::new(self.parse_stmt()?);
            return Ok(Stmt::For(init, cond, inc, body, span));
        }

        if self.is_keyword("switch") {
            let span = self.span();
            self.advance();
            self.expect_punct("(")?;
            let cond = self.parse_expr()?;
            self.expect_punct(")")?;
            let body = Box::new(self.parse_stmt()?);
            return Ok(Stmt::Switch(cond, body, span));
        }

        if self.is_keyword("case") {
            let span = self.span();
            self.advance();
            // Always parse case value as constant expression
            let expr = self.parse_ternary()?;
            let val = eval_const_expr(&expr)
                .ok_or_else(|| CompileError::new("case requires constant expression", expr.span()))?;
            // GCC extension: case 0 ... 5: (range case)
            if self.eat_punct("...") {
                // Parse end of range, just use the end value
                let _end_val = match &self.peek().kind {
                    TokenKind::Int(n) => { let n = *n; self.advance(); n }
                    _ => {
                        let expr = self.parse_ternary()?;
                        eval_const_expr(&expr).unwrap_or(val)
                    }
                };
                // For simplicity, only use the start value — not a full range match
                // A proper implementation would generate multiple case labels
            }
            self.expect_punct(":")?;
            let stmt = Box::new(self.parse_stmt()?);
            return Ok(Stmt::Case(val, stmt, span));
        }

        if self.is_keyword("default") {
            let span = self.span();
            self.advance();
            self.expect_punct(":")?;
            let stmt = Box::new(self.parse_stmt()?);
            return Ok(Stmt::Default(stmt, span));
        }

        if self.is_keyword("break") {
            let span = self.span();
            self.advance();
            self.expect_punct(";")?;
            return Ok(Stmt::Break(span));
        }

        if self.is_keyword("continue") {
            let span = self.span();
            self.advance();
            self.expect_punct(";")?;
            return Ok(Stmt::Continue(span));
        }

        if self.is_keyword("goto") {
            let span = self.span();
            self.advance();
            // Computed goto: goto *expr;
            if self.eat_punct("*") {
                let _ = self.parse_expr()?;
                self.expect_punct(";")?;
                return Ok(Stmt::Null); // treat as no-op for now
            }
            let (label, _) = self.expect_ident()?;
            self.expect_punct(";")?;
            return Ok(Stmt::Goto(label, span));
        }

        // Label: ident ':'
        if let TokenKind::Ident(_) = &self.peek().kind {
            if self.tokens.get(self.pos + 1).map_or(false, |t| matches!(&t.kind, TokenKind::Punct(p) if p == ":")) {
                let (name, _) = self.expect_ident()?;
                self.expect_punct(":")?;
                let span = self.span();
                let stmt = Box::new(self.parse_stmt()?);
                return Ok(Stmt::Label(name, stmt, span));
            }
        }

        // Inline assembly: asm("..."), asm volatile("..."), __asm__("...")
        if self.is_keyword("asm") {
            self.advance();
            while self.eat_keyword("inline") || self.eat_keyword("volatile")
                || self.eat_keyword("__volatile__") || self.eat_keyword("goto") {}
            if self.eat_punct("(") {
                let mut depth = 1;
                while depth > 0 && !self.at_eof() {
                    if self.eat_punct("(") { depth += 1; }
                    else if self.eat_punct(")") { depth -= 1; }
                    else { self.advance(); }
                }
            }
            if self.is_punct(";") { self.advance(); }
            return Ok(Stmt::Null);
        }

        // Check for keyword typos (did you mean?) — only for identifiers 4+ chars
        // that appear at statement position and the next token looks wrong for an expression
        if let TokenKind::Ident(name) = &self.peek().kind {
            if name.len() >= 4 {
                let stmt_keywords = &[
                    "return", "break", "continue", "switch", "while",
                    "struct", "double", "float", "unsigned", "signed",
                    "static", "extern", "typedef", "default",
                ];
                if let Some(suggestion) = did_you_mean(name, stmt_keywords) {
                    let span = self.span();
                    return Err(CompileError::new(
                        format!("unknown identifier '{}'", name),
                        span,
                    ).with_hint(format!("did you mean '{}'?", suggestion)));
                }
            }
        }

        // Variable declaration
        if self.is_typename() {
            return self.parse_var_decl();
        }

        // Expression statement
        let span = self.span();
        let expr = self.parse_expr()?;
        self.expect_punct(";")?;
        Ok(Stmt::Expr(expr, span))
    }

    fn parse_var_decl(&mut self) -> Result<Stmt, CompileError> {
        let span = self.span();
        let (base_ty, is_static, is_extern, is_typedef) = self.parse_base_type_full()?;

        // Type-only declaration: enum { ... }; or struct { ... };
        if self.eat_punct(";") {
            return Ok(Stmt::Null);
        }

        // Local typedef: typedef int T;
        if is_typedef {
            let (ty, name) = self.parse_declarator(base_ty)?;
            self.typedefs.insert(name, ty);
            self.expect_punct(";")?;
            return Ok(Stmt::Null);
        }

        let mut decls = Vec::new();
        loop {
            let (mut ty, name) = self.parse_declarator(base_ty.clone())?;

            // Local function declaration: int foo(int x);
            if self.is_punct("(") {
                self.advance();
                let mut depth = 1;
                while depth > 0 && !self.at_eof() {
                    if self.eat_punct("(") { depth += 1; }
                    else if self.eat_punct(")") { depth -= 1; }
                    else { self.advance(); }
                }
                self.expect_punct(";")?;
                return Ok(Stmt::Null);
            }

            let init = if self.eat_punct("=") {
                if self.is_punct("{") {
                    let init_list = self.parse_init_list()?;
                    if let Type::Array(base, 0) = &ty {
                        ty = Type::Array(base.clone(), init_list.len());
                    }
                    Some(Expr::InitList(init_list, span))
                } else {
                    Some(self.parse_assign()?)
                }
            } else {
                None
            };

            decls.push(VarDecl { name, ty, init, is_static, is_extern });

            if !self.eat_punct(",") {
                break;
            }
        }
        self.expect_punct(";")?;

        if decls.len() == 1 {
            Ok(Stmt::VarDecl(decls.remove(0), span))
        } else {
            // Multiple declarations → Block of VarDecls
            let stmts = decls.into_iter().map(|d| Stmt::VarDecl(d, span)).collect();
            Ok(Stmt::Block(stmts, span))
        }
    }

    fn parse_init_list(&mut self) -> Result<Vec<Expr>, CompileError> {
        self.expect_punct("{")?;
        let mut items = Vec::new();
        while !self.is_punct("}") && !self.at_eof() {
            // Skip designated initializers: .field = or .field[n] = or [index] =
            while self.is_punct(".") || self.is_punct("[") {
                if self.eat_punct(".") {
                    self.advance(); // skip field name
                } else if self.eat_punct("[") {
                    while !self.is_punct("]") && !self.at_eof() { self.advance(); }
                    self.expect_punct("]")?;
                }
            }
            if self.is_punct("=") && !self.is_punct("==") {
                self.advance(); // skip =
            }

            if self.is_punct("{") {
                items.push(Expr::InitList(self.parse_init_list()?, self.span()));
            } else {
                items.push(self.parse_assign()?);
            }
            if !self.eat_punct(",") { break; }
        }
        self.expect_punct("}")?;
        Ok(items)
    }

    fn parse_block(&mut self) -> Result<Stmt, CompileError> {
        let span = self.span();
        self.expect_punct("{")?;
        let mut stmts = Vec::new();
        while !self.is_punct("}") && !self.at_eof() {
            stmts.push(self.parse_stmt()?);
        }
        self.expect_punct("}")?;
        Ok(Stmt::Block(stmts, span))
    }

    // ── Parsing: top-level ──

    pub fn parse_program(&mut self) -> Result<TranslationUnit, CompileError> {
        let mut decls = Vec::new();
        while !self.at_eof() {
            decls.push(self.parse_top_level()?);
        }
        Ok(TranslationUnit {
            decls,
            struct_types: self.struct_types.clone(),
            typedefs: self.typedefs.clone(),
        })
    }

    fn parse_top_level(&mut self) -> Result<TopLevel, CompileError> {
        let span = self.span();
        let (base_ty, is_static, is_extern, is_typedef) = self.parse_base_type_full()?;

        // Struct/union/enum definition without variable (e.g. "struct Foo { ... };")
        if self.eat_punct(";") {
            return Ok(TopLevel::GlobalVar(VarDecl {
                name: String::new(), ty: base_ty, init: None,
                is_static, is_extern,
            }, span));
        }

        let saved_base_ty = base_ty.clone();
        let (ty, name) = self.parse_declarator(base_ty)?;

        // Handle typedef (including comma-separated: typedef int A, B[4];)
        if is_typedef {
            self.typedefs.insert(name.clone(), ty.clone());
            while self.eat_punct(",") {
                let (extra_ty, extra_name) = self.parse_declarator(saved_base_ty.clone())?;
                self.typedefs.insert(extra_name, extra_ty);
            }
            self.expect_punct(";")?;
            return Ok(TopLevel::GlobalVar(VarDecl {
                name: String::new(), ty: Type::Void, init: None,
                is_static: false, is_extern: false,
            }, span));
        }

        // Function definition: if declarator already consumed params (grouped declarator),
        // we see { directly — treat as function with no explicit params at this level
        if self.is_punct("{") && !is_typedef {
            let body = self.parse_block()?;
            return Ok(TopLevel::FuncDef {
                name, return_ty: ty, params: vec![], body, is_static, span,
            });
        }

        // Function definition or declaration
        if self.is_punct("(") {
            self.advance();
            let mut params = Vec::new();
            let mut is_variadic = false;

            if !self.is_punct(")") {
                if !(self.is_keyword("void") && self.tokens.get(self.pos + 1).map_or(false, |t| matches!(&t.kind, TokenKind::Punct(p) if p == ")"))) {
                    loop {
                        if self.eat_punct("...") {
                            is_variadic = true;
                            break;
                        }
                        let (pty, _, _) = self.parse_base_type()?;
                        // Handle pointer types like char *, const char * const
                        let mut pty = pty;
                        loop {
                            if self.eat_punct("*") {
                                pty = Type::Ptr(Box::new(pty));
                            } else if self.eat_keyword("const") || self.eat_keyword("volatile")
                                || self.eat_keyword("__restrict") || self.eat_keyword("__restrict__")
                                || self.eat_keyword("restrict") {
                                // skip qualifiers
                            } else {
                                break;
                            }
                        }
                        // Handle function pointer params: void (*fn)(int)
                        let pname = if self.is_punct("(") {
                            self.advance(); // skip (
                            while self.eat_punct("*") { pty = Type::Ptr(Box::new(pty)); }
                            let n = if let TokenKind::Ident(_) = &self.peek().kind {
                                let (n, _) = self.expect_ident()?; n
                            } else { String::new() };
                            self.expect_punct(")")?;
                            // Skip function params
                            if self.eat_punct("(") {
                                let mut depth = 1;
                                while depth > 0 && !self.at_eof() {
                                    if self.eat_punct("(") { depth += 1; }
                                    else if self.eat_punct(")") { depth -= 1; }
                                    else { self.advance(); }
                                }
                            }
                            pty = Type::Ptr(Box::new(Type::Void));
                            n
                        } else if let TokenKind::Ident(_) = &self.peek().kind {
                            let (n, _) = self.expect_ident()?;
                            if self.eat_punct("[") {
                                while !self.is_punct("]") && !self.at_eof() { self.advance(); }
                                self.expect_punct("]")?;
                                pty = Type::Ptr(Box::new(pty));
                            } else if self.is_punct("(") {
                                // Function param: int x() — param decays to function pointer
                                self.advance();
                                let mut depth = 1;
                                while depth > 0 && !self.at_eof() {
                                    if self.eat_punct("(") { depth += 1; }
                                    else if self.eat_punct(")") { depth -= 1; }
                                    else { self.advance(); }
                                }
                                pty = Type::Ptr(Box::new(pty));
                            }
                            n
                        } else {
                            String::new()
                        };
                        params.push((pty, pname));
                        if !self.eat_punct(",") {
                            break;
                        }
                    }
                } else {
                    // void params
                    self.advance(); // skip 'void'
                }
            }
            self.expect_punct(")")?;

            // Function body or declaration
            if self.is_punct("{") {
                let body = self.parse_block()?;
                return Ok(TopLevel::FuncDef {
                    name, return_ty: ty, params, body, is_static, span,
                });
            }

            self.expect_punct(";")?;
            return Ok(TopLevel::FuncDecl {
                name, return_ty: ty, params, is_variadic, span,
            });
        }

        // Global variable
        let init = if self.eat_punct("=") {
            if self.is_punct("{") {
                let items = self.parse_init_list()?;
                Some(Expr::InitList(items, span))
            } else {
                Some(self.parse_assign()?)
            }
        } else {
            None
        };
        // Handle comma-separated global declarations: int a, b[4];
        // For now, skip additional declarators
        while self.eat_punct(",") {
            let _ = self.parse_declarator(ty.clone())?;
            if self.eat_punct("=") {
                if self.is_punct("{") { let _ = self.parse_init_list()?; }
                else { let _ = self.parse_assign()?; }
            }
        }
        self.expect_punct(";")?;

        Ok(TopLevel::GlobalVar(VarDecl {
            name, ty, init, is_static, is_extern,
        }, span))
    }
}

fn eval_const_expr(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::IntLit(v, _) => Some(*v),
        Expr::CharLit(v, _) => Some(*v),
        Expr::Unary(UnaryOp::Neg, e, _) => eval_const_expr(e).map(|v| -v),
        Expr::Unary(UnaryOp::BitNot, e, _) => eval_const_expr(e).map(|v| !v),
        Expr::Unary(UnaryOp::Not, e, _) => eval_const_expr(e).map(|v| if v == 0 { 1 } else { 0 }),
        Expr::Binary(op, l, r, _) => {
            let l = eval_const_expr(l)?;
            let r = eval_const_expr(r)?;
            Some(match op {
                BinOp::Add => l + r,
                BinOp::Sub => l - r,
                BinOp::Mul => l * r,
                BinOp::Div if r != 0 => l / r,
                BinOp::Mod if r != 0 => l % r,
                BinOp::BitAnd => l & r,
                BinOp::BitOr => l | r,
                BinOp::BitXor => l ^ r,
                BinOp::Shl => l << (r & 63),
                BinOp::Shr => l >> (r & 63),
                BinOp::Eq => if l == r { 1 } else { 0 },
                BinOp::Ne => if l != r { 1 } else { 0 },
                BinOp::Lt => if l < r { 1 } else { 0 },
                BinOp::Le => if l <= r { 1 } else { 0 },
                BinOp::Gt => if l > r { 1 } else { 0 },
                BinOp::Ge => if l >= r { 1 } else { 0 },
                BinOp::LogAnd => if l != 0 && r != 0 { 1 } else { 0 },
                BinOp::LogOr => if l != 0 || r != 0 { 1 } else { 0 },
                _ => return None,
            })
        }
        Expr::Cond(c, t, e, _) => {
            let c = eval_const_expr(c)?;
            if c != 0 { eval_const_expr(t) } else { eval_const_expr(e) }
        }
        _ => None,
    }
}
