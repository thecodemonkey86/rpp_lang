//! Parser für R++, von Hand geschrieben (rekursiver Abstieg, Vorrangparser für
//! Ausdrücke). Er erkennt dieselbe Sprache wie `rpp-grammatik/RPP.g4`.
//!
//! Wo ANTLR mit unbegrenzter Vorausschau zwischen Alternativen entscheidet, probiert
//! dieser Parser die Alternativen in der Reihenfolge der Grammatik aus und setzt bei
//! einem Fehlschlag zurück ([`Parser::attempt`]). Bei einer Anweisung reicht ein
//! Versuch bis zum abschließenden `;`, sodass die erste Alternative gewinnt, die die
//! ganze Anweisung liest – wie die Auflösung von ANTLR nach der kleinsten Alternative.
//! Gemeldet wird der Fehler, der am weitesten im Quelltext liegt.

use crate::ast::*;
use crate::lexer::{Keyword as K, Punct as P, Span, Token, TokenKind as T};

/// Ein Syntaxfehler mit Position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub span: Span,
    pub message: String,
}

impl std::fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Line {}:{} {}", self.span.line, self.span.col, self.message)
    }
}

/// Markiert einen Fehlschlag; die Meldung steht in [`Parser::furthest`].
#[derive(Debug)]
pub(crate) struct Fail;

type PResult<T> = Result<T, Fail>;

pub(crate) struct Parser {
    toks: Vec<Token>,
    pos: usize,
    furthest: Option<(usize, String)>,
    next_id: u32,
    literals: Vec<LiteralSite>,
}

/// Zusammenhang, in dem ein Typ steht.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TypeCtx {
    /// gewöhnlich: Typargumente und `?` werden immer gelesen
    Plain,
    /// nach `as`: `<` und `?` können auch zum umgebenden Ausdruck gehören
    AfterAs,
}

impl Parser {
    pub(crate) fn new(toks: Vec<Token>) -> Parser {
        Parser { toks, pos: 0, furthest: None, next_id: 0, literals: Vec::new() }
    }

    fn expr_id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    pub(crate) fn error(&self) -> SyntaxError {
        let (pos, message) = self.furthest.clone().unwrap_or((self.pos, "Syntax error".into()));
        let pos = pos.min(self.toks.len() - 1);
        SyntaxError { span: self.toks[pos].span, message }
    }

    // ------------------------------------------------------------ Hilfen

    fn kind(&self, offset: usize) -> &T {
        let i = (self.pos + offset).min(self.toks.len() - 1);
        &self.toks[i].kind
    }

    fn span(&self) -> Span {
        self.toks[self.pos.min(self.toks.len() - 1)].span
    }

    fn bump(&mut self) {
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
    }

    fn fail<V>(&mut self, expected: &str) -> PResult<V> {
        let found = match self.kind(0) {
            T::Error(msg) => msg.clone(),
            other => format!("found {other}"),
        };
        let message = format!("Expected {expected}, {found}");
        match &self.furthest {
            Some((p, _)) if *p >= self.pos => {}
            _ => self.furthest = Some((self.pos, message)),
        }
        Err(Fail)
    }

    fn is_p(&self, p: P) -> bool {
        *self.kind(0) == T::Punct(p)
    }

    fn is_p_at(&self, offset: usize, p: P) -> bool {
        *self.kind(offset) == T::Punct(p)
    }

    fn is_k(&self, k: K) -> bool {
        *self.kind(0) == T::Keyword(k)
    }

    fn eat_p(&mut self, p: P) -> bool {
        if self.is_p(p) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn eat_k(&mut self, k: K) -> bool {
        if self.is_k(k) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_p(&mut self, p: P) -> PResult<()> {
        if self.eat_p(p) {
            Ok(())
        } else {
            self.fail(&format!("'{}'", p.as_str()))
        }
    }

    fn expect_k(&mut self, k: K) -> PResult<()> {
        if self.eat_k(k) {
            Ok(())
        } else {
            self.fail(&format!("'{}'", k.as_str()))
        }
    }

    /// Probiert `f`; bei einem Fehlschlag werden Position und Literalstellen zurückgesetzt.
    fn attempt<V>(&mut self, f: impl FnOnce(&mut Self) -> PResult<V>) -> Option<V> {
        let save = self.pos;
        let literals = self.literals.len();
        match f(self) {
            Ok(v) => Some(v),
            Err(Fail) => {
                self.pos = save;
                self.literals.truncate(literals);
                None
            }
        }
    }

    fn ident(&mut self) -> PResult<Ident> {
        if let T::Ident(name) = self.kind(0) {
            let id = Ident { name: name.clone(), span: self.span() };
            self.bump();
            Ok(id)
        } else {
            self.fail("name")
        }
    }

    fn is_ident(&self, offset: usize) -> bool {
        matches!(self.kind(offset), T::Ident(_))
    }

    fn qualified_name(&mut self) -> PResult<QualifiedName> {
        let mut parts = vec![self.ident()?];
        while self.is_p(P::Dot) && self.is_ident(1) {
            self.bump();
            parts.push(self.ident()?);
        }
        Ok(QualifiedName { parts })
    }

    // ------------------------------------------------------------ Dateien

    pub(crate) fn compilation_unit(&mut self) -> PResult<CompilationUnit> {
        let mut package = None;
        if self.eat_k(K::Package) {
            package = Some(self.qualified_name()?);
            self.expect_p(P::Semi)?;
        }
        let mut usings = Vec::new();
        while self.is_k(K::Using) {
            let span = self.span();
            self.bump();
            let mut alias = None;
            if self.is_ident(0) && self.is_p_at(1, P::Assign) {
                alias = Some(self.ident()?);
                self.bump();
            }
            let target = self.qualified_name()?;
            self.expect_p(P::Semi)?;
            usings.push(Using { alias, target, span });
        }
        let mut decls = Vec::new();
        while *self.kind(0) != T::Eof {
            if self.is_k(K::I32) {
                decls.push(TopDecl::Main(self.main_decl()?));
            } else {
                decls.push(self.type_decl()?);
            }
        }
        Ok(CompilationUnit { package, usings, decls, literals: std::mem::take(&mut self.literals) })
    }

    fn main_decl(&mut self) -> PResult<MainDecl> {
        let span = self.span();
        self.expect_k(K::I32)?;
        if !matches!(self.kind(0), T::Ident(n) if n == "main") {
            return self.fail("'main'");
        }
        self.bump();
        self.expect_p(P::LParen)?;
        self.expect_k(K::String)?;
        self.expect_p(P::LBracket)?;
        self.expect_p(P::RBracket)?;
        let args = self.ident()?;
        self.expect_p(P::RParen)?;
        let body = self.block()?;
        Ok(MainDecl { args, body, span })
    }

    fn type_decl(&mut self) -> PResult<TopDecl> {
        let export = self.eat_k(K::Export);
        let span = self.span();
        if self.is_k(K::Abstract) || self.is_k(K::Class) {
            let is_abstract = self.eat_k(K::Abstract);
            self.expect_k(K::Class)?;
            let name = self.ident()?;
            let type_params = self.type_params_opt()?;
            let base = if self.eat_p(P::Colon) { Some(self.type_name(TypeCtx::Plain)?) } else { None };
            self.expect_p(P::LBrace)?;
            let mut members = Vec::new();
            while !self.eat_p(P::RBrace) {
                members.push(self.class_member()?);
            }
            Ok(TopDecl::Class(ClassDecl { export, is_abstract, name, type_params, base, members, span }))
        } else if self.eat_k(K::Mixin) {
            let name = self.ident()?;
            let type_params = self.type_params_opt()?;
            self.expect_p(P::LBrace)?;
            let mut members = Vec::new();
            while !self.eat_p(P::RBrace) {
                members.push(self.mixin_member()?);
            }
            Ok(TopDecl::Mixin(MixinDecl { export, name, type_params, members, span }))
        } else if self.eat_k(K::Enum) {
            let name = self.ident()?;
            self.expect_p(P::LBrace)?;
            let mut variants = vec![self.enum_variant()?];
            loop {
                if self.eat_p(P::RBrace) {
                    break;
                }
                self.expect_p(P::Comma)?;
                if self.eat_p(P::RBrace) {
                    break;
                }
                variants.push(self.enum_variant()?);
            }
            Ok(TopDecl::Enum(EnumDecl { export, name, variants, span }))
        } else {
            self.fail("type declaration")
        }
    }

    fn enum_variant(&mut self) -> PResult<EnumVariant> {
        let name = self.ident()?;
        let payload = if self.eat_p(P::LParen) {
            let params = self.param_list()?;
            self.expect_p(P::RParen)?;
            VariantPayload::Data(params)
        } else if self.eat_p(P::Assign) {
            let neg = self.eat_p(P::Minus);
            match self.kind(0).clone() {
                T::Int(v) => {
                    self.bump();
                    VariantPayload::Value(if neg { format!("-{v}") } else { v })
                }
                _ => return self.fail("integer"),
            }
        } else {
            VariantPayload::None
        };
        Ok(EnumVariant { name, payload })
    }

    fn type_params_opt(&mut self) -> PResult<Vec<TypeParam>> {
        let mut params = Vec::new();
        if self.eat_p(P::Lt) {
            loop {
                let name = self.ident()?;
                let mut constraints = Vec::new();
                if self.eat_p(P::Colon) {
                    loop {
                        if self.eat_k(K::New) {
                            self.expect_p(P::LParen)?;
                            self.expect_p(P::RParen)?;
                            constraints.push(Constraint::New);
                        } else {
                            constraints.push(Constraint::Type(self.type_name(TypeCtx::Plain)?));
                        }
                        if !self.eat_p(P::Amp) {
                            break;
                        }
                    }
                }
                params.push(TypeParam { name, constraints });
                if !self.eat_p(P::Comma) {
                    break;
                }
            }
            self.expect_p(P::Gt)?;
        }
        Ok(params)
    }

    // ------------------------------------------------------------ Member

    fn class_member(&mut self) -> PResult<ClassMember> {
        let span = self.span();
        for (k, access) in [(K::Public, Access::Public), (K::Private, Access::Private), (K::Protected, Access::Protected)] {
            if self.is_k(k) {
                self.bump();
                self.expect_p(P::Colon)?;
                return Ok(ClassMember::Access(access, span));
            }
        }
        if self.eat_k(K::Constructor) {
            let params = self.params()?;
            let mut base_args = None;
            if self.eat_p(P::Colon) {
                self.expect_k(K::Base)?;
                base_args = Some(self.args()?);
            }
            let body = self.block()?;
            return Ok(ClassMember::Constructor(ConstructorDecl { params, base_args, body, span }));
        }
        if self.eat_k(K::Destructor) {
            self.expect_p(P::LParen)?;
            self.expect_p(P::RParen)?;
            return Ok(ClassMember::Destructor(self.block()?, span));
        }
        if self.eat_k(K::Copy) {
            self.expect_p(P::LParen)?;
            let param = self.param()?;
            self.expect_p(P::RParen)?;
            return Ok(ClassMember::Copy(param, self.block()?, span));
        }
        if self.eat_k(K::Import) {
            let ty = self.type_name(TypeCtx::Plain)?;
            self.expect_p(P::Semi)?;
            return Ok(ClassMember::Import(ty));
        }
        if let Some(field) = self.attempt(|p| p.field_decl()) {
            return Ok(ClassMember::Field(field));
        }
        Ok(ClassMember::Method(self.method_decl(false)?))
    }

    fn mixin_member(&mut self) -> PResult<ClassMember> {
        if self.eat_k(K::Import) {
            let ty = self.type_name(TypeCtx::Plain)?;
            self.expect_p(P::Semi)?;
            return Ok(ClassMember::Import(ty));
        }
        Ok(ClassMember::Method(self.method_decl(true)?))
    }

    fn field_decl(&mut self) -> PResult<FieldDecl> {
        let mut is_static = false;
        let mut is_const = false;
        if self.eat_k(K::Static) {
            is_static = true;
            self.expect_k(K::Const)?;
            is_const = true;
        } else if self.eat_k(K::Const) {
            is_const = true;
        }
        let ty = self.ty(TypeCtx::Plain)?;
        let name = self.ident()?;
        let init = if self.eat_p(P::Assign) { Some(self.expression()?) } else { None };
        self.expect_p(P::Semi)?;
        Ok(FieldDecl { is_static, is_const, ty, name, init })
    }

    /// methodDecl und abstractMethodDecl, im Mixin auch requirement.
    fn method_decl(&mut self, in_mixin: bool) -> PResult<MethodDecl> {
        let nodiscard = self.eat_k(K::Nodiscard);
        let mut modifier = None;
        if !in_mixin && self.eat_k(K::Abstract) {
            modifier = Some(MethodModifier::Abstract);
        } else if self.eat_k(K::Static) {
            modifier = Some(MethodModifier::Static);
        } else if self.eat_k(K::Virtual) {
            modifier = Some(MethodModifier::Virtual);
        } else if self.eat_k(K::Override) {
            modifier = Some(MethodModifier::Override);
        }
        let ret = self.return_type()?;
        let name = self.ident()?;
        let type_params = self.type_params_opt()?;
        let params = self.params()?;
        let mut_this = self.eat_k(K::Mut);
        let without_body = modifier == Some(MethodModifier::Abstract) || (in_mixin && modifier.is_none());
        let body = if without_body && self.eat_p(P::Semi) {
            None
        } else if modifier == Some(MethodModifier::Abstract) {
            return self.fail("';'");
        } else {
            Some(self.block()?)
        };
        Ok(MethodDecl { nodiscard, modifier, ret, name, type_params, params, mut_this, body })
    }

    fn params(&mut self) -> PResult<Vec<Param>> {
        self.expect_p(P::LParen)?;
        if self.eat_p(P::RParen) {
            return Ok(Vec::new());
        }
        let list = self.param_list()?;
        self.expect_p(P::RParen)?;
        Ok(list)
    }

    fn param_list(&mut self) -> PResult<Vec<Param>> {
        let mut list = vec![self.param()?];
        while self.eat_p(P::Comma) {
            list.push(self.param()?);
        }
        Ok(list)
    }

    fn param(&mut self) -> PResult<Param> {
        let mutable = self.eat_k(K::Mut);
        let ty = self.ty(TypeCtx::Plain)?;
        let name = self.ident()?;
        Ok(Param { ty, mutable, name })
    }

    // ------------------------------------------------------------ Typen

    fn primitive(&self) -> Option<Primitive> {
        let T::Keyword(k) = self.kind(0) else { return None };
        Some(match k {
            K::Bool => Primitive::Bool,
            K::Char => Primitive::Char,
            K::Byte => Primitive::Byte,
            K::String => Primitive::String,
            K::I8 => Primitive::I8,
            K::I16 => Primitive::I16,
            K::I32 => Primitive::I32,
            K::I64 => Primitive::I64,
            K::Ui8 => Primitive::Ui8,
            K::Ui16 => Primitive::Ui16,
            K::Ui32 => Primitive::Ui32,
            K::Ui64 => Primitive::Ui64,
            K::F32 => Primitive::F32,
            K::F64 => Primitive::F64,
            _ => return None,
        })
    }

    fn return_type(&mut self) -> PResult<ReturnType> {
        if self.eat_k(K::Void) {
            return Ok(ReturnType::Void);
        }
        if self.eat_k(K::Ref) {
            let mutable = self.eat_k(K::Mut);
            let span = self.span();
            let base = self.base_type(TypeCtx::Plain)?;
            let suffixes = self.type_suffixes(TypeCtx::Plain)?;
            return Ok(ReturnType::Ref { ty: Type { ownership: None, base, suffixes, span }, mutable });
        }
        Ok(ReturnType::Type(self.ty(TypeCtx::Plain)?))
    }

    fn ty(&mut self, ctx: TypeCtx) -> PResult<Type> {
        let span = self.span();
        let ownership = if self.eat_k(K::Own) {
            Some(Ownership::Own)
        } else if self.eat_k(K::Shared) {
            Some(Ownership::Shared)
        } else if self.eat_k(K::Weak) {
            Some(Ownership::Weak)
        } else {
            None
        };
        let base = self.base_type(ctx)?;
        let suffixes = self.type_suffixes(ctx)?;
        Ok(Type { ownership, base, suffixes, span })
    }

    fn base_type(&mut self, ctx: TypeCtx) -> PResult<BaseType> {
        if let Some(prim) = self.primitive() {
            self.bump();
            return Ok(BaseType::Primitive(prim));
        }
        if self.eat_p(P::LParen) {
            let mut types = vec![self.ty(TypeCtx::Plain)?];
            self.expect_p(P::Comma)?;
            types.push(self.ty(TypeCtx::Plain)?);
            while self.eat_p(P::Comma) {
                types.push(self.ty(TypeCtx::Plain)?);
            }
            self.expect_p(P::RParen)?;
            return Ok(BaseType::Tuple(types));
        }
        if self.is_ident(0) {
            return Ok(BaseType::Named(self.type_name(ctx)?));
        }
        self.fail("type")
    }

    fn type_name(&mut self, ctx: TypeCtx) -> PResult<TypeName> {
        let name = self.qualified_name()?;
        let args = if !self.is_p(P::Lt) {
            Vec::new()
        } else if ctx == TypeCtx::AfterAs {
            self.attempt(|p| p.type_args()).unwrap_or_default()
        } else {
            self.type_args()?
        };
        Ok(TypeName { name, args })
    }

    fn type_args(&mut self) -> PResult<Vec<TypeArg>> {
        self.expect_p(P::Lt)?;
        let mut args = Vec::new();
        loop {
            if self.eat_k(K::Void) {
                args.push(TypeArg::Void);
            } else {
                let mutable = self.eat_k(K::Mut);
                let ty = self.ty(TypeCtx::Plain)?;
                args.push(TypeArg::Type { ty, mutable });
            }
            if !self.eat_p(P::Comma) {
                break;
            }
        }
        self.expect_p(P::Gt)?;
        Ok(args)
    }

    fn type_suffixes(&mut self, ctx: TypeCtx) -> PResult<Vec<TypeSuffix>> {
        let mut suffixes = Vec::new();
        loop {
            if self.is_p(P::Question) {
                // Nach `as` gehört `?` zum Bedingungsausdruck, wenn danach ein Ausdruck beginnt.
                if ctx == TypeCtx::AfterAs && self.starts_expression(1) {
                    break;
                }
                self.bump();
                suffixes.push(TypeSuffix::Nullable);
            } else if self.eat_p(P::LBracket) {
                if self.eat_p(P::RBracket) {
                    suffixes.push(TypeSuffix::Vector);
                    continue;
                }
                let size = self.array_size()?;
                self.expect_p(P::RBracket)?;
                suffixes.push(TypeSuffix::Array(size));
            } else {
                break;
            }
        }
        Ok(suffixes)
    }

    fn array_size(&mut self) -> PResult<ArraySize> {
        if let T::Int(v) = self.kind(0).clone() {
            self.bump();
            return Ok(ArraySize::Int(v));
        }
        if let Some(s) = self.attempt(|p| p.static_ref()) {
            return Ok(ArraySize::Static(s));
        }
        Ok(ArraySize::Name(self.ident()?))
    }

    /// Kann an `offset` ein Ausdruck beginnen?
    fn starts_expression(&self, offset: usize) -> bool {
        match self.kind(offset) {
            T::Ident(_) | T::Int(_) | T::Float(_) | T::Str(_) | T::Char(_) => true,
            T::Keyword(k) => matches!(
                k,
                K::This | K::Base | K::New | K::Cast | K::Copy | K::True | K::False | K::Null | K::Own
                    | K::Bool | K::Char | K::Byte | K::String | K::I8 | K::I16 | K::I32 | K::I64
                    | K::Ui8 | K::Ui16 | K::Ui32 | K::Ui64 | K::F32 | K::F64
            ),
            T::Punct(p) => matches!(
                p,
                P::LParen | P::LBracket | P::Bang | P::Tilde | P::Minus | P::PlusPlus | P::MinusMinus
            ),
            _ => false,
        }
    }

    // ------------------------------------------------------------ Anweisungen

    pub(crate) fn block(&mut self) -> PResult<Block> {
        let span = self.span();
        self.expect_p(P::LBrace)?;
        let mut stmts = Vec::new();
        while !self.is_p(P::RBrace) {
            stmts.push(self.statement()?);
        }
        let end = self.span();
        self.bump();
        Ok(Block { stmts, span, end })
    }

    fn statement(&mut self) -> PResult<Stmt> {
        let span = self.span();
        let kind = self.statement_kind()?;
        Ok(Stmt { kind, span })
    }

    fn statement_kind(&mut self) -> PResult<StmtKind> {
        match self.kind(0).clone() {
            T::Punct(P::LBrace) => return Ok(StmtKind::Block(self.block()?)),
            T::Keyword(K::If) => {
                self.bump();
                let cond = self.paren_expr()?;
                let then = Box::new(self.statement()?);
                let otherwise = if self.eat_k(K::Else) { Some(Box::new(self.statement()?)) } else { None };
                return Ok(StmtKind::If { cond, then, otherwise });
            }
            T::Keyword(K::While) => {
                self.bump();
                let cond = self.paren_expr()?;
                let body = Box::new(self.statement()?);
                return Ok(StmtKind::While { cond, body });
            }
            T::Keyword(K::Do) => {
                self.bump();
                let body = Box::new(self.statement()?);
                self.expect_k(K::While)?;
                let cond = self.paren_expr()?;
                self.expect_p(P::Semi)?;
                return Ok(StmtKind::DoWhile { body, cond });
            }
            T::Keyword(K::For) => return self.for_stmt(),
            T::Keyword(K::Switch) => return self.switch_stmt(),
            T::Keyword(K::Return) => {
                self.bump();
                if self.eat_p(P::Semi) {
                    return Ok(StmtKind::Return { borrow_mut: false, value: None });
                }
                let borrow_mut = self.eat_k(K::Mut);
                let value = Some(self.expression()?);
                self.expect_p(P::Semi)?;
                return Ok(StmtKind::Return { borrow_mut, value });
            }
            T::Keyword(K::Break) => {
                self.bump();
                self.expect_p(P::Semi)?;
                return Ok(StmtKind::Break);
            }
            T::Keyword(K::Continue) => {
                self.bump();
                self.expect_p(P::Semi)?;
                return Ok(StmtKind::Continue);
            }
            T::Keyword(K::Throw) => {
                self.bump();
                let e = self.expression()?;
                self.expect_p(P::Semi)?;
                return Ok(StmtKind::Throw(e));
            }
            T::Keyword(K::Try) => return self.try_stmt(),
            T::Keyword(K::Discard) => {
                self.bump();
                let e = self.expression()?;
                self.expect_p(P::Semi)?;
                return Ok(StmtKind::Discard(e));
            }
            T::Keyword(K::Ref) => {
                let kind = self.ref_decl()?;
                self.expect_p(P::Semi)?;
                return Ok(kind);
            }
            _ => {}
        }
        // localVarDecl, tupleDecl und exprStatement: die erste Alternative, die die
        // ganze Anweisung liest, gewinnt.
        if let Some(decl) = self.attempt(|p| {
            let d = p.local_var_decl()?;
            p.expect_p(P::Semi)?;
            Ok(d)
        }) {
            return Ok(StmtKind::LocalVar(decl));
        }
        if self.is_p(P::LParen) {
            if let Some(decl) = self.attempt(|p| {
                let d = p.tuple_decl()?;
                p.expect_p(P::Semi)?;
                Ok(d)
            }) {
                return Ok(StmtKind::Tuple(decl));
            }
        }
        let e = self.expr_statement()?;
        self.expect_p(P::Semi)?;
        Ok(StmtKind::Expr(e))
    }

    fn paren_expr(&mut self) -> PResult<Expr> {
        self.expect_p(P::LParen)?;
        let e = self.expression()?;
        self.expect_p(P::RParen)?;
        Ok(e)
    }

    fn local_var_decl(&mut self) -> PResult<LocalVarDecl> {
        let is_const = self.eat_k(K::Const);
        let ty = if self.eat_k(K::Var) { VarType::Var } else { VarType::Type(self.ty(TypeCtx::Plain)?) };
        let name = self.ident()?;
        let init = if self.eat_p(P::Assign) { Some(self.expression()?) } else { None };
        Ok(LocalVarDecl { is_const, ty, name, init })
    }

    fn ref_decl(&mut self) -> PResult<StmtKind> {
        self.expect_k(K::Ref)?;
        let mutable = self.eat_k(K::Mut);
        let span = self.span();
        let base = self.base_type(TypeCtx::Plain)?;
        let suffixes = self.type_suffixes(TypeCtx::Plain)?;
        let ty = Type { ownership: None, base, suffixes, span };
        let name = self.ident()?;
        self.expect_p(P::Assign)?;
        if mutable {
            self.expect_k(K::Mut)?;
        }
        let init = self.expression()?;
        Ok(StmtKind::Ref { ty, mutable, name, init })
    }

    fn tuple_decl(&mut self) -> PResult<TupleDecl> {
        self.expect_p(P::LParen)?;
        let mut vars = Vec::new();
        loop {
            let ty = self.ty(TypeCtx::Plain)?;
            let name = self.ident()?;
            vars.push((ty, name));
            if !self.eat_p(P::Comma) {
                break;
            }
        }
        if vars.len() < 2 {
            return self.fail("','");
        }
        self.expect_p(P::RParen)?;
        self.expect_p(P::Assign)?;
        let init = self.expression()?;
        Ok(TupleDecl { vars, init })
    }

    fn assign_op(&self) -> Option<AssignOp> {
        Some(match self.kind(0) {
            T::Punct(P::Assign) => AssignOp::Assign,
            T::Punct(P::PlusEq) => AssignOp::Add,
            T::Punct(P::MinusEq) => AssignOp::Sub,
            T::Punct(P::StarEq) => AssignOp::Mul,
            T::Punct(P::SlashEq) => AssignOp::Div,
            T::Punct(P::PercentEq) => AssignOp::Rem,
            _ => return None,
        })
    }

    /// exprStatement : unaryExpr assignOp expression | ('++'|'--') unaryExpr | postfixExpr
    fn expr_statement(&mut self) -> PResult<ExprStmt> {
        let target = self.unary()?;
        if let Some(op) = self.assign_op() {
            self.bump();
            let value = self.expression()?;
            return Ok(ExprStmt::Assign { target, op, value });
        }
        match &target.kind {
            ExprKind::Unary { op: UnaryOp::PreInc | UnaryOp::PreDec, .. } => Ok(ExprStmt::Expr(target)),
            ExprKind::Unary { .. } => self.fail("assignment"),
            _ => Ok(ExprStmt::Expr(target)),
        }
    }

    fn for_stmt(&mut self) -> PResult<StmtKind> {
        self.expect_k(K::For)?;
        self.expect_p(P::LParen)?;
        // forEachStmt : FOR '(' MUT? (type | VAR) identifier ':' MUT? expression ')' statement
        if let Some((ty, mutable, name)) = self.attempt(|p| {
            let mutable = p.eat_k(K::Mut);
            let ty = if p.eat_k(K::Var) { VarType::Var } else { VarType::Type(p.ty(TypeCtx::Plain)?) };
            let name = p.ident()?;
            p.expect_p(P::Colon)?;
            Ok((ty, mutable, name))
        }) {
            let borrow_mut = self.eat_k(K::Mut);
            let iter = self.expression()?;
            self.expect_p(P::RParen)?;
            let body = Box::new(self.statement()?);
            return Ok(StmtKind::ForEach { ty, mutable, name, borrow_mut, iter, body });
        }
        let init = if self.is_p(P::Semi) {
            None
        } else if let Some(d) = self.attempt(|p| {
            let d = p.local_var_decl()?;
            if !p.is_p(P::Semi) {
                return p.fail("';'");
            }
            Ok(d)
        }) {
            Some(ForInit::Var(d))
        } else if let Some(d) = self.attempt(|p| {
            let d = p.tuple_decl()?;
            if !p.is_p(P::Semi) {
                return p.fail("';'");
            }
            Ok(d)
        }) {
            Some(ForInit::Tuple(d))
        } else {
            Some(ForInit::Exprs(self.expr_statement_list()?))
        };
        self.expect_p(P::Semi)?;
        let cond = if self.is_p(P::Semi) { None } else { Some(self.expression()?) };
        self.expect_p(P::Semi)?;
        let update = if self.is_p(P::RParen) { Vec::new() } else { self.expr_statement_list()? };
        self.expect_p(P::RParen)?;
        let body = Box::new(self.statement()?);
        Ok(StmtKind::For { init, cond, update, body })
    }

    fn expr_statement_list(&mut self) -> PResult<Vec<ExprStmt>> {
        let mut list = vec![self.expr_statement()?];
        while self.eat_p(P::Comma) {
            list.push(self.expr_statement()?);
        }
        Ok(list)
    }

    fn switch_stmt(&mut self) -> PResult<StmtKind> {
        self.expect_k(K::Switch)?;
        let subject = self.paren_expr()?;
        self.expect_p(P::LBrace)?;
        let mut cases = Vec::new();
        while !self.eat_p(P::RBrace) {
            let span = self.span();
            let mut patterns = Vec::new();
            let is_default = if self.eat_k(K::Default) {
                true
            } else {
                self.expect_k(K::Case)?;
                patterns.push(self.pattern()?);
                while self.eat_p(P::Comma) {
                    patterns.push(self.pattern()?);
                }
                false
            };
            self.expect_p(P::Colon)?;
            let mut body = Vec::new();
            while !(self.is_k(K::Case) || self.is_k(K::Default) || self.is_p(P::RBrace)) {
                body.push(self.statement()?);
            }
            cases.push(CaseClause { patterns, is_default, body, span });
        }
        Ok(StmtKind::Switch { subject, cases })
    }

    fn pattern(&mut self) -> PResult<Pattern> {
        let negative = self.is_p(P::Minus);
        let lit_at = usize::from(negative);
        if let Some(lit) = self.literal_at(lit_at) {
            if negative {
                self.bump();
            }
            self.bump();
            let id = self.expr_id();
            self.literals.push(LiteralSite { id, literal: lit.clone(), context: LiteralContext::Pattern { negative } });
            return Ok(Pattern::Literal { id, negative, lit });
        }
        if negative {
            self.bump();
            return self.fail("literal");
        }
        let name = match self.attempt(|p| p.static_ref()) {
            Some(s) => PatternName::Static(s),
            None => PatternName::Simple(self.ident()?),
        };
        let bindings = if self.eat_p(P::LParen) {
            let mut ids = vec![self.ident()?];
            while self.eat_p(P::Comma) {
                ids.push(self.ident()?);
            }
            self.expect_p(P::RParen)?;
            Some(ids)
        } else {
            None
        };
        Ok(Pattern::Name { name, bindings })
    }

    fn try_stmt(&mut self) -> PResult<StmtKind> {
        self.expect_k(K::Try)?;
        let body = self.block()?;
        let mut catches = Vec::new();
        while self.eat_k(K::Catch) {
            self.expect_p(P::LParen)?;
            let ty = self.ty(TypeCtx::Plain)?;
            let name = self.ident()?;
            self.expect_p(P::RParen)?;
            catches.push(CatchClause { ty, name, body: self.block()? });
        }
        let finally = if self.eat_k(K::Finally) {
            Some(self.block()?)
        } else if catches.is_empty() {
            return self.fail("'catch' or 'finally'");
        } else {
            None
        };
        Ok(StmtKind::Try { body, catches, finally })
    }

    // ------------------------------------------------------------ Ausdrücke

    /// expression : lambda | conditionalExpr
    pub(crate) fn expression(&mut self) -> PResult<Expr> {
        if self.is_k(K::Own) || self.is_p(P::LParen) {
            let span = self.span();
            if let Some((own, params)) = self.attempt(|p| p.lambda_head()) {
                let body = if self.is_p(P::LBrace) {
                    LambdaBody::Block(self.block()?)
                } else {
                    LambdaBody::Expr(Box::new(self.expression()?))
                };
                return Ok(Expr { kind: ExprKind::Lambda { own, params, body }, span, id: self.expr_id() });
            }
        }
        self.conditional()
    }

    /// OWN? '(' paramList? ')' '=>'
    fn lambda_head(&mut self) -> PResult<(bool, Vec<Param>)> {
        let own = self.eat_k(K::Own);
        let params = self.params()?;
        self.expect_p(P::Arrow)?;
        Ok((own, params))
    }

    fn conditional(&mut self) -> PResult<Expr> {
        let cond = self.coalesce()?;
        if self.is_p(P::Question) {
            let span = cond.span;
            self.bump();
            let then = self.expression()?;
            self.expect_p(P::Colon)?;
            let otherwise = self.expression()?;
            return Ok(Expr {
                kind: ExprKind::Conditional { cond: Box::new(cond), then: Box::new(then), otherwise: Box::new(otherwise) },
                span,
                id: self.expr_id(),
            });
        }
        Ok(cond)
    }

    fn coalesce(&mut self) -> PResult<Expr> {
        let left = self.binary(0)?;
        if self.eat_p(P::QuestionQuestion) {
            let right = self.coalesce()?;
            let span = left.span;
            return Ok(Expr { kind: ExprKind::Coalesce { left: Box::new(left), right: Box::new(right) }, span, id: self.expr_id() });
        }
        Ok(left)
    }

    /// Nächster zweistelliger Operator: (Operator, Anzahl Tokens, Vorrang × 2).
    /// `as` hat Stufe 10a, also 21.
    fn binary_op(&self) -> Option<(Option<BinaryOp>, usize, u8)> {
        let op = match self.kind(0) {
            T::Keyword(K::As) => return Some((None, 1, 21)),
            T::Punct(P::Lt) if self.is_p_at(1, P::Lt) => return Some((Some(BinaryOp::Shl), 2, 22)),
            T::Punct(P::Gt) if self.is_p_at(1, P::Gt) => return Some((Some(BinaryOp::Shr), 2, 22)),
            T::Punct(P::Star) => BinaryOp::Mul,
            T::Punct(P::Slash) => BinaryOp::Div,
            T::Punct(P::Percent) => BinaryOp::Rem,
            T::Punct(P::Plus) => BinaryOp::Add,
            T::Punct(P::Minus) => BinaryOp::Sub,
            T::Punct(P::Lt) => BinaryOp::Lt,
            T::Punct(P::Gt) => BinaryOp::Gt,
            T::Punct(P::LtEq) => BinaryOp::Le,
            T::Punct(P::GtEq) => BinaryOp::Ge,
            T::Punct(P::EqEq) => BinaryOp::Eq,
            T::Punct(P::NotEq) => BinaryOp::Ne,
            T::Punct(P::Amp) => BinaryOp::BitAnd,
            T::Punct(P::Caret) => BinaryOp::BitXor,
            T::Punct(P::Pipe) => BinaryOp::BitOr,
            T::Punct(P::AndAnd) => BinaryOp::And,
            T::Punct(P::OrOr) => BinaryOp::Or,
            _ => return None,
        };
        Some((Some(op), 1, op.precedence() * 2))
    }

    fn binary(&mut self, min_prec: u8) -> PResult<Expr> {
        let mut left = self.unary()?;
        while let Some((op, width, prec)) = self.binary_op() {
            if prec < min_prec {
                break;
            }
            let span = left.span;
            for _ in 0..width {
                self.bump();
            }
            left = match op {
                None => {
                    let ty = self.ty(TypeCtx::AfterAs)?;
                    Expr { kind: ExprKind::As { expr: Box::new(left), ty }, span, id: self.expr_id() }
                }
                Some(op) => {
                    let right = self.binary(prec + 1)?;
                    Expr { kind: ExprKind::Binary { op, left: Box::new(left), right: Box::new(right) }, span, id: self.expr_id() }
                }
            };
        }
        Ok(left)
    }

    fn unary(&mut self) -> PResult<Expr> {
        let span = self.span();
        let op = match self.kind(0) {
            T::Punct(P::Bang) => UnaryOp::Not,
            T::Punct(P::Tilde) => UnaryOp::BitNot,
            T::Punct(P::Minus) => UnaryOp::Neg,
            T::Punct(P::PlusPlus) => UnaryOp::PreInc,
            T::Punct(P::MinusMinus) => UnaryOp::PreDec,
            _ => return self.postfix(),
        };
        self.bump();
        let expr = Box::new(self.unary()?);
        let id = self.expr_id();
        if let (UnaryOp::Neg, ExprKind::Literal(lit @ (Literal::Int(_) | Literal::Float(_)))) = (op, &expr.kind) {
            self.literals.push(LiteralSite { id, literal: lit.clone(), context: LiteralContext::NegatedNumber });
        }
        Ok(Expr { kind: ExprKind::Unary { op, expr }, span, id })
    }

    fn postfix(&mut self) -> PResult<Expr> {
        let mut e = self.primary()?;
        loop {
            let span = e.span;
            if self.is_p(P::Dot) || self.is_p(P::QuestionDot) {
                let safe = self.is_p(P::QuestionDot);
                self.bump();
                let name = self.ident()?;
                e = Expr { kind: ExprKind::Member { object: Box::new(e), name, safe }, span, id: self.expr_id() };
            } else if self.is_p(P::LParen) {
                let args = self.args()?;
                e = Expr { kind: ExprKind::Call { callee: Box::new(e), args }, span, id: self.expr_id() };
            } else if self.eat_p(P::LBracket) {
                let from = if self.is_p(P::DotDot) { None } else { Some(Box::new(self.expression()?)) };
                if self.eat_p(P::DotDot) {
                    let to = if self.is_p(P::RBracket) { None } else { Some(Box::new(self.expression()?)) };
                    self.expect_p(P::RBracket)?;
                    e = Expr { kind: ExprKind::Slice { object: Box::new(e), from, to }, span, id: self.expr_id() };
                } else {
                    self.expect_p(P::RBracket)?;
                    let index = from.expect("an index without a range has an expression");
                    e = Expr { kind: ExprKind::Index { object: Box::new(e), index }, span, id: self.expr_id() };
                }
            } else if self.eat_p(P::PlusPlus) {
                e = Expr { kind: ExprKind::PostInc(Box::new(e)), span, id: self.expr_id() };
            } else if self.eat_p(P::MinusMinus) {
                e = Expr { kind: ExprKind::PostDec(Box::new(e)), span, id: self.expr_id() };
            } else {
                return Ok(e);
            }
        }
    }

    fn args(&mut self) -> PResult<Vec<Arg>> {
        self.expect_p(P::LParen)?;
        let mut args = Vec::new();
        if self.eat_p(P::RParen) {
            return Ok(args);
        }
        loop {
            let borrow_mut = self.eat_k(K::Mut);
            let expr = self.expression()?;
            args.push(Arg { borrow_mut, expr });
            if !self.eat_p(P::Comma) {
                break;
            }
        }
        self.expect_p(P::RParen)?;
        Ok(args)
    }

    fn literal_at(&self, offset: usize) -> Option<Literal> {
        Some(match self.kind(offset) {
            T::Int(v) => Literal::Int(v.clone()),
            T::Float(v) => Literal::Float(v.clone()),
            T::Str(v) => Literal::Str(v.clone()),
            T::Char(v) => Literal::Char(v.clone()),
            T::Keyword(K::True) => Literal::Bool(true),
            T::Keyword(K::False) => Literal::Bool(false),
            T::Keyword(K::Null) => Literal::Null,
            _ => return None,
        })
    }

    /// staticRef : (qualifiedName typeArgs? | primitiveType) '::' identifier
    fn static_ref(&mut self) -> PResult<StaticRef> {
        let span = self.span();
        let owner = if let Some(prim) = self.primitive() {
            self.bump();
            StaticOwner::Primitive(prim)
        } else {
            let name = self.qualified_name()?;
            let args = if self.is_p(P::Lt) { self.attempt(|p| p.type_args()).unwrap_or_default() } else { Vec::new() };
            StaticOwner::Named(TypeName { name, args })
        };
        self.expect_p(P::ColonColon)?;
        let member = self.ident()?;
        Ok(StaticRef { owner, member, span })
    }

    fn primary(&mut self) -> PResult<Expr> {
        let span = self.span();
        if let Some(lit) = self.literal_at(0) {
            self.bump();
            let id = self.expr_id();
            self.literals.push(LiteralSite { id, literal: lit.clone(), context: LiteralContext::Expression });
            return Ok(Expr { kind: ExprKind::Literal(lit), span, id });
        }
        let kind = match self.kind(0).clone() {
            T::Keyword(K::This) => {
                self.bump();
                ExprKind::This
            }
            T::Keyword(K::Base) => {
                self.bump();
                ExprKind::Base
            }
            T::Punct(P::LParen) => {
                self.bump();
                let first = self.expression()?;
                if self.eat_p(P::Comma) {
                    let mut items = vec![first, self.expression()?];
                    while self.eat_p(P::Comma) {
                        items.push(self.expression()?);
                    }
                    self.expect_p(P::RParen)?;
                    ExprKind::Tuple(items)
                } else {
                    self.expect_p(P::RParen)?;
                    ExprKind::Paren(Box::new(first))
                }
            }
            T::Punct(P::LBracket) => {
                self.bump();
                let mut items = Vec::new();
                if !self.eat_p(P::RBracket) {
                    items.push(self.expression()?);
                    while self.eat_p(P::Comma) {
                        items.push(self.expression()?);
                    }
                    self.expect_p(P::RBracket)?;
                }
                ExprKind::Array(items)
            }
            T::Keyword(K::New) => {
                self.bump();
                let ty = self.type_name(TypeCtx::Plain)?;
                let args = self.args()?;
                ExprKind::New { ty, args }
            }
            T::Keyword(K::Cast) => {
                self.bump();
                self.expect_p(P::Lt)?;
                let ty = self.ty(TypeCtx::Plain)?;
                self.expect_p(P::Gt)?;
                let expr = Box::new(self.paren_expr()?);
                ExprKind::Cast { ty, expr }
            }
            T::Keyword(K::Copy) => {
                self.bump();
                ExprKind::Copy(Box::new(self.paren_expr()?))
            }
            T::Ident(_) => match self.attempt(|p| p.static_ref()) {
                Some(s) => ExprKind::Static(s),
                None => ExprKind::Name(self.ident()?),
            },
            _ if self.primitive().is_some() => ExprKind::Static(self.static_ref()?),
            _ => return self.fail("expression"),
        };
        Ok(Expr { kind, span, id: self.expr_id() })
    }
}
