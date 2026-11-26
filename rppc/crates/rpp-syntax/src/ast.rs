//! Abstrakter Syntaxbaum. Er folgt der Grammatik `RPP.g4`, fasst aber gleichartige
//! Formen zusammen (z. B. alle Methodenarten in [`MethodDecl`]).

pub use crate::lexer::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QualifiedName {
    pub parts: Vec<Ident>,
}

impl QualifiedName {
    pub fn span(&self) -> Span {
        self.parts[0].span
    }
    pub fn to_dotted(&self) -> String {
        self.parts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(".")
    }
}

// ------------------------------------------------------------------ Dateien

#[derive(Debug, Clone, PartialEq)]
pub struct CompilationUnit {
    pub package: Option<QualifiedName>,
    pub usings: Vec<Using>,
    pub decls: Vec<TopDecl>,
    /// Literalstellen für die Vorbereitung der Ausführung nach der Typprüfung.
    pub literals: Vec<LiteralSite>,
}

/// `using a.b.C;` oder `using X = a.b.C;`
#[derive(Debug, Clone, PartialEq)]
pub struct Using {
    pub alias: Option<Ident>,
    pub target: QualifiedName,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TopDecl {
    Class(ClassDecl),
    Mixin(MixinDecl),
    Enum(EnumDecl),
    Main(MainDecl),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassDecl {
    pub export: bool,
    pub is_abstract: bool,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub base: Option<TypeName>,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MixinDecl {
    pub export: bool,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    /// nur `Method`, `Requirement` (als Methode ohne Rumpf) und `Import`
    pub members: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub export: bool,
    pub name: Ident,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VariantPayload {
    None,
    Data(Vec<Param>),
    /// Zahlwert als Text, mit Vorzeichen
    Value(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: Ident,
    pub payload: VariantPayload,
}

/// `i32 main(string[] args) { … }`
#[derive(Debug, Clone, PartialEq)]
pub struct MainDecl {
    pub args: Ident,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeParam {
    pub name: Ident,
    pub constraints: Vec<Constraint>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Constraint {
    Type(TypeName),
    New,
}

// ------------------------------------------------------------------ Member

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Public,
    Private,
    Protected,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClassMember {
    Access(Access, Span),
    Field(FieldDecl),
    Method(MethodDecl),
    Constructor(ConstructorDecl),
    Destructor(Block, Span),
    Copy(Param, Block, Span),
    Import(TypeName),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl {
    pub is_static: bool,
    pub is_const: bool,
    pub ty: Type,
    pub name: Ident,
    pub init: Option<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodModifier {
    Static,
    Virtual,
    Override,
    Abstract,
}

/// Methode, abstrakte Methode oder Anforderung eines Mixins (diese beiden ohne Rumpf).
#[derive(Debug, Clone, PartialEq)]
pub struct MethodDecl {
    pub nodiscard: bool,
    pub modifier: Option<MethodModifier>,
    pub ret: ReturnType,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub params: Vec<Param>,
    /// `mut` nach der Parameterliste: veränderbarer Borrow auf `this`
    pub mut_this: bool,
    pub body: Option<Block>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstructorDecl {
    pub params: Vec<Param>,
    /// `: base(…)`
    pub base_args: Option<Vec<Arg>>,
    pub body: Block,
    pub span: Span,
}

/// `MUT? type name`
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub ty: Type,
    pub mutable: bool,
    pub name: Ident,
}

// ------------------------------------------------------------------ Typen

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership {
    Own,
    Shared,
    Weak,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Primitive {
    Bool,
    Char,
    Byte,
    String,
    I8,
    I16,
    I32,
    I64,
    Ui8,
    Ui16,
    Ui32,
    Ui64,
    F32,
    F64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeName {
    pub name: QualifiedName,
    pub args: Vec<TypeArg>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeArg {
    /// Typ, mit `mut` bei veränderbaren Parametern von `Func`
    Type { ty: Type, mutable: bool },
    Void,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BaseType {
    Primitive(Primitive),
    Named(TypeName),
    Tuple(Vec<Type>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArraySize {
    Int(String),
    Name(Ident),
    Static(StaticRef),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeSuffix {
    Nullable,
    /// `T[]`
    Vector,
    /// `T[N]`
    Array(ArraySize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Type {
    pub ownership: Option<Ownership>,
    pub base: BaseType,
    pub suffixes: Vec<TypeSuffix>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReturnType {
    Void,
    Type(Type),
    /// `ref T` oder `ref mut T`: zurückgegebener Borrow (R-5.23)
    Ref { ty: Type, mutable: bool },
}

// ------------------------------------------------------------------ Anweisungen

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
    /// Position der schließenden Klammer
    pub end: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Assign,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VarType {
    Type(Type),
    Var,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalVarDecl {
    pub is_const: bool,
    pub ty: VarType,
    pub name: Ident,
    pub init: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TupleDecl {
    pub vars: Vec<(Type, Ident)>,
    pub init: Expr,
}

/// Ausdrucksanweisung (`exprStatement`)
#[derive(Debug, Clone, PartialEq)]
pub enum ExprStmt {
    Assign { target: Expr, op: AssignOp, value: Expr },
    /// Aufruf, `x++`, `++x` usw.
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ForInit {
    Var(LocalVarDecl),
    Tuple(TupleDecl),
    Exprs(Vec<ExprStmt>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Literal { id: u32, negative: bool, lit: Literal },
    /// Konstante oder Enum-Variante, optional mit Bindungen: `Circle(r)`, `Shape::Circle(r)`
    Name { name: PatternName, bindings: Option<Vec<Ident>> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PatternName {
    Simple(Ident),
    Static(StaticRef),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaseClause {
    /// leer bei `default`
    pub patterns: Vec<Pattern>,
    pub is_default: bool,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatchClause {
    pub ty: Type,
    pub name: Ident,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    Block(Block),
    LocalVar(LocalVarDecl),
    /// `ref T x = e;` oder `ref mut T x = mut e;`
    Ref { ty: Type, mutable: bool, name: Ident, init: Expr },
    Tuple(TupleDecl),
    Expr(ExprStmt),
    Discard(Expr),
    If { cond: Expr, then: Box<Stmt>, otherwise: Option<Box<Stmt>> },
    While { cond: Expr, body: Box<Stmt> },
    DoWhile { body: Box<Stmt>, cond: Expr },
    For { init: Option<ForInit>, cond: Option<Expr>, update: Vec<ExprStmt>, body: Box<Stmt> },
    ForEach { ty: VarType, mutable: bool, name: Ident, borrow_mut: bool, iter: Expr, body: Box<Stmt> },
    Switch { subject: Expr, cases: Vec<CaseClause> },
    Return { borrow_mut: bool, value: Option<Expr> },
    Break,
    Continue,
    Throw(Expr),
    Try { body: Block, catches: Vec<CatchClause>, finally: Option<Block> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

impl Stmt {
    /// Name der Grammatikregel, wie ihn der ANTLR-Parser liefern würde
    /// (für die Lesarten in `rpp-grammatik/tests/lesarten`).
    pub fn rule_name(&self) -> &'static str {
        match self.kind {
            StmtKind::Block(_) => "block",
            StmtKind::LocalVar(_) => "localVarDecl",
            StmtKind::Ref { .. } => "refDecl",
            StmtKind::Tuple(_) => "tupleDecl",
            StmtKind::Expr(_) => "exprStatement",
            StmtKind::Discard(_) => "discardStmt",
            StmtKind::If { .. } => "ifStmt",
            StmtKind::While { .. } => "whileStmt",
            StmtKind::DoWhile { .. } => "doWhileStmt",
            StmtKind::For { .. } => "forStmt",
            StmtKind::ForEach { .. } => "forEachStmt",
            StmtKind::Switch { .. } => "switchStmt",
            StmtKind::Return { .. } => "return",
            StmtKind::Break => "break",
            StmtKind::Continue => "continue",
            StmtKind::Throw(_) => "throw",
            StmtKind::Try { .. } => "tryStmt",
        }
    }
}

// ------------------------------------------------------------------ Ausdrücke

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Int(String),
    Float(String),
    Str(String),
    Char(String),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiteralSite {
    /// Eindeutig je Datei, aus demselben Nummernraum wie `Expr::id`.
    pub id: u32,
    pub literal: Literal,
    pub context: LiteralContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralContext {
    Expression,
    NegatedNumber,
    Pattern { negative: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    BitNot,
    Neg,
    PreInc,
    PreDec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Mul,
    Div,
    Rem,
    Add,
    Sub,
    Shl,
    Shr,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
    BitAnd,
    BitXor,
    BitOr,
    And,
    Or,
}

impl BinaryOp {
    /// Vorrangstufe nach Gerüst 3.2 (größer bindet stärker).
    pub fn precedence(self) -> u8 {
        use BinaryOp::*;
        match self {
            Mul | Div | Rem => 13,
            Add | Sub => 12,
            Shl | Shr => 11,
            Lt | Gt | Le | Ge => 10,
            Eq | Ne => 9,
            BitAnd => 8,
            BitXor => 7,
            BitOr => 6,
            And => 5,
            Or => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StaticOwner {
    Named(TypeName),
    Primitive(Primitive),
}

/// `Typ::name`
#[derive(Debug, Clone, PartialEq)]
pub struct StaticRef {
    pub owner: StaticOwner,
    pub member: Ident,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    /// `mut e`: veränderbarer Borrow
    pub borrow_mut: bool,
    pub expr: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LambdaBody {
    Expr(Box<Expr>),
    Block(Block),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Literal(Literal),
    Name(Ident),
    This,
    Base,
    Paren(Box<Expr>),
    Tuple(Vec<Expr>),
    Array(Vec<Expr>),
    New { ty: TypeName, args: Vec<Arg> },
    Cast { ty: Type, expr: Box<Expr> },
    Copy(Box<Expr>),
    Static(StaticRef),
    Lambda { own: bool, params: Vec<Param>, body: LambdaBody },
    Unary { op: UnaryOp, expr: Box<Expr> },
    Binary { op: BinaryOp, left: Box<Expr>, right: Box<Expr> },
    As { expr: Box<Expr>, ty: Type },
    Coalesce { left: Box<Expr>, right: Box<Expr> },
    Conditional { cond: Box<Expr>, then: Box<Expr>, otherwise: Box<Expr> },
    /// `a.b` oder `a?.b`
    Member { object: Box<Expr>, name: Ident, safe: bool },
    Call { callee: Box<Expr>, args: Vec<Arg> },
    Index { object: Box<Expr>, index: Box<Expr> },
    /// `a[from..to]`, beide Grenzen optional
    Slice { object: Box<Expr>, from: Option<Box<Expr>>, to: Option<Box<Expr>> },
    PostInc(Box<Expr>),
    PostDec(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
    /// eindeutig je Datei; Schlüssel für die Angaben der Prüfung an die Ausführung
    pub id: u32,
}
