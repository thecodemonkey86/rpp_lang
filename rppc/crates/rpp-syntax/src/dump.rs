//! Textform des Syntaxbaums für den Vergleich mit dem Parser in R++ (`rpp-compiler`,
//! Meilenstein M6). Ein Knoten je Zeile: Einrückung (zwei Leerzeichen je Tiefe), Art,
//! optional ein Text, optional die Position `@zeile:spalte`. Listen stehen immer als
//! eigener Knoten da (auch leer), Optionales nur, wenn es vorhanden ist.

use crate::ast::*;
use crate::lexer::Span;
use std::fmt::Write;

struct D {
    out: String,
    depth: usize,
}

fn flags(list: &[(bool, &str)]) -> String {
    list.iter().filter(|(on, _)| *on).map(|(_, s)| *s).collect::<Vec<_>>().join(" ")
}

fn prim(p: Primitive) -> &'static str {
    match p {
        Primitive::Bool => "bool",
        Primitive::Char => "char",
        Primitive::Byte => "byte",
        Primitive::String => "string",
        Primitive::I8 => "i8",
        Primitive::I16 => "i16",
        Primitive::I32 => "i32",
        Primitive::I64 => "i64",
        Primitive::Ui8 => "ui8",
        Primitive::Ui16 => "ui16",
        Primitive::Ui32 => "ui32",
        Primitive::Ui64 => "ui64",
        Primitive::F32 => "f32",
        Primitive::F64 => "f64",
    }
}

fn binop(op: BinaryOp) -> &'static str {
    use BinaryOp::*;
    match op {
        Mul => "*",
        Div => "/",
        Rem => "%",
        Add => "+",
        Sub => "-",
        Shl => "<<",
        Shr => ">>",
        Lt => "<",
        Gt => ">",
        Le => "<=",
        Ge => ">=",
        Eq => "==",
        Ne => "!=",
        BitAnd => "&",
        BitXor => "^",
        BitOr => "|",
        And => "&&",
        Or => "||",
    }
}

impl D {
    fn line(&mut self, kind: &str, text: &str, span: Option<Span>) {
        for _ in 0..self.depth {
            self.out.push_str("  ");
        }
        self.out.push_str(kind);
        if !text.is_empty() {
            self.out.push(' ');
            self.out.push_str(text);
        }
        if let Some(s) = span {
            let _ = write!(self.out, " @{}:{}", s.line, s.col);
        }
        self.out.push('\n');
    }

    fn open(&mut self, kind: &str, text: &str, span: Option<Span>) {
        self.line(kind, text, span);
        self.depth += 1;
    }

    fn close(&mut self) {
        self.depth -= 1;
    }

    fn id(&mut self, i: &Ident) {
        self.line("id", &i.name, Some(i.span));
    }

    fn qname(&mut self, q: &QualifiedName) {
        self.open("qname", "", None);
        for p in &q.parts {
            self.id(p);
        }
        self.close();
    }

    fn unit(&mut self, u: &CompilationUnit) {
        self.open("unit", "", None);
        if let Some(p) = &u.package {
            self.open("package", "", None);
            self.qname(p);
            self.close();
        }
        for us in &u.usings {
            self.open("using", if us.alias.is_some() { "alias" } else { "" }, Some(us.span));
            if let Some(a) = &us.alias {
                self.id(a);
            }
            self.qname(&us.target);
            self.close();
        }
        for d in &u.decls {
            self.decl(d);
        }
        self.close();
    }

    fn decl(&mut self, d: &TopDecl) {
        match d {
            TopDecl::Class(c) => {
                self.open("class", &flags(&[(c.export, "export"), (c.is_abstract, "abstract")]), Some(c.span));
                self.id(&c.name);
                self.tparams(&c.type_params);
                if let Some(b) = &c.base {
                    self.open("base", "", None);
                    self.type_name(b);
                    self.close();
                }
                self.members(&c.members);
                self.close();
            }
            TopDecl::Mixin(m) => {
                self.open("mixin", &flags(&[(m.export, "export")]), Some(m.span));
                self.id(&m.name);
                self.tparams(&m.type_params);
                self.members(&m.members);
                self.close();
            }
            TopDecl::Enum(e) => {
                self.open("enum", &flags(&[(e.export, "export")]), Some(e.span));
                self.id(&e.name);
                self.open("variants", "", None);
                for v in &e.variants {
                    let text = match &v.payload {
                        VariantPayload::Value(s) => format!("= {s}"),
                        _ => String::new(),
                    };
                    self.open("variant", &text, None);
                    self.id(&v.name);
                    if let VariantPayload::Data(ps) = &v.payload {
                        self.params(ps);
                    }
                    self.close();
                }
                self.close();
                self.close();
            }
            TopDecl::Main(m) => {
                self.open("main", "", Some(m.span));
                self.id(&m.args);
                self.block(&m.body);
                self.close();
            }
        }
    }

    fn tparams(&mut self, ps: &[TypeParam]) {
        self.open("tparams", "", None);
        for p in ps {
            self.open("tparam", "", None);
            self.id(&p.name);
            for c in &p.constraints {
                match c {
                    Constraint::New => self.line("new", "", None),
                    Constraint::Type(t) => self.type_name(t),
                }
            }
            self.close();
        }
        self.close();
    }

    fn members(&mut self, ms: &[ClassMember]) {
        self.open("members", "", None);
        for m in ms {
            match m {
                ClassMember::Access(a, span) => {
                    let t = match a {
                        Access::Public => "public",
                        Access::Private => "private",
                        Access::Protected => "protected",
                    };
                    self.line("access", t, Some(*span));
                }
                ClassMember::Field(f) => {
                    self.open("field", &flags(&[(f.is_static, "static"), (f.is_const, "const")]), None);
                    self.ty(&f.ty);
                    self.id(&f.name);
                    if let Some(e) = &f.init {
                        self.expr(e);
                    }
                    self.close();
                }
                ClassMember::Method(md) => {
                    let modifier = match md.modifier {
                        Some(MethodModifier::Static) => "static",
                        Some(MethodModifier::Virtual) => "virtual",
                        Some(MethodModifier::Override) => "override",
                        Some(MethodModifier::Abstract) => "abstract",
                        None => "",
                    };
                    self.open("method", &flags(&[(md.nodiscard, "nodiscard"), (!modifier.is_empty(), modifier), (md.mut_this, "mut")]), None);
                    match &md.ret {
                        ReturnType::Void => self.line("void", "", None),
                        ReturnType::Type(t) => self.ty(t),
                        ReturnType::Ref { ty, mutable } => {
                            self.open("refret", if *mutable { "mut" } else { "" }, None);
                            self.ty(ty);
                            self.close();
                        }
                    }
                    self.id(&md.name);
                    self.tparams(&md.type_params);
                    self.params(&md.params);
                    if let Some(b) = &md.body {
                        self.block(b);
                    }
                    self.close();
                }
                ClassMember::Constructor(c) => {
                    self.open("ctor", "", Some(c.span));
                    self.params(&c.params);
                    if let Some(a) = &c.base_args {
                        self.open("baseargs", "", None);
                        self.args(a);
                        self.close();
                    }
                    self.block(&c.body);
                    self.close();
                }
                ClassMember::Destructor(b, span) => {
                    self.open("dtor", "", Some(*span));
                    self.block(b);
                    self.close();
                }
                ClassMember::Copy(p, b, span) => {
                    self.open("copy", "", Some(*span));
                    self.param(p);
                    self.block(b);
                    self.close();
                }
                ClassMember::Import(t) => {
                    self.open("import", "", None);
                    self.type_name(t);
                    self.close();
                }
            }
        }
        self.close();
    }

    fn params(&mut self, ps: &[Param]) {
        self.open("params", "", None);
        for p in ps {
            self.param(p);
        }
        self.close();
    }

    fn param(&mut self, p: &Param) {
        self.open("param", if p.mutable { "mut" } else { "" }, None);
        self.ty(&p.ty);
        self.id(&p.name);
        self.close();
    }

    fn ty(&mut self, t: &Type) {
        let own = match t.ownership {
            Some(Ownership::Own) => "own",
            Some(Ownership::Shared) => "shared",
            Some(Ownership::Weak) => "weak",
            None => "",
        };
        self.open("type", own, Some(t.span));
        match &t.base {
            BaseType::Primitive(p) => self.line("prim", prim(*p), None),
            BaseType::Named(n) => self.type_name(n),
            BaseType::Tuple(ts) => {
                self.open("tuple", "", None);
                for x in ts {
                    self.ty(x);
                }
                self.close();
            }
        }
        for s in &t.suffixes {
            match s {
                TypeSuffix::Nullable => self.line("nullable", "", None),
                TypeSuffix::Vector => self.line("vector", "", None),
                TypeSuffix::Array(size) => {
                    self.open("array", "", None);
                    match size {
                        ArraySize::Int(v) => self.line("int", v, None),
                        ArraySize::Name(i) => self.id(i),
                        ArraySize::Static(s) => self.sref(s),
                    }
                    self.close();
                }
            }
        }
        self.close();
    }

    fn type_name(&mut self, n: &TypeName) {
        self.open("typename", "", None);
        self.qname(&n.name);
        self.open("targs", "", None);
        for a in &n.args {
            match a {
                TypeArg::Void => self.line("void", "", None),
                TypeArg::Type { ty, mutable } => {
                    self.open("targ", if *mutable { "mut" } else { "" }, None);
                    self.ty(ty);
                    self.close();
                }
            }
        }
        self.close();
        self.close();
    }

    fn sref(&mut self, s: &StaticRef) {
        self.open("sref", "", Some(s.span));
        match &s.owner {
            StaticOwner::Primitive(p) => self.line("prim", prim(*p), None),
            StaticOwner::Named(n) => self.type_name(n),
        }
        self.id(&s.member);
        self.close();
    }

    fn block(&mut self, b: &Block) {
        self.open("block", &format!("end={}:{}", b.end.line, b.end.col), Some(b.span));
        for s in &b.stmts {
            self.stmt(s);
        }
        self.close();
    }

    fn local(&mut self, d: &LocalVarDecl, span: Option<Span>) {
        self.open("local", if d.is_const { "const" } else { "" }, span);
        match &d.ty {
            VarType::Var => self.line("var", "", None),
            VarType::Type(t) => self.ty(t),
        }
        self.id(&d.name);
        if let Some(e) = &d.init {
            self.expr(e);
        }
        self.close();
    }

    fn tuple_decl(&mut self, d: &TupleDecl, span: Option<Span>) {
        self.open("tupledecl", "", span);
        self.open("vars", "", None);
        for (t, i) in &d.vars {
            self.open("tvar", "", None);
            self.ty(t);
            self.id(i);
            self.close();
        }
        self.close();
        self.expr(&d.init);
        self.close();
    }

    fn expr_stmt(&mut self, es: &ExprStmt) {
        match es {
            ExprStmt::Assign { target, op, value } => {
                let o = match op {
                    AssignOp::Assign => "=",
                    AssignOp::Add => "+=",
                    AssignOp::Sub => "-=",
                    AssignOp::Mul => "*=",
                    AssignOp::Div => "/=",
                    AssignOp::Rem => "%=",
                };
                self.open("assign", o, None);
                self.expr(target);
                self.expr(value);
                self.close();
            }
            ExprStmt::Expr(e) => {
                self.open("eval", "", None);
                self.expr(e);
                self.close();
            }
        }
    }

    fn exprs(&mut self, list: &[ExprStmt]) {
        self.open("exprs", "", None);
        for e in list {
            self.expr_stmt(e);
        }
        self.close();
    }

    fn stmt(&mut self, s: &Stmt) {
        let sp = Some(s.span);
        match &s.kind {
            StmtKind::Block(b) => self.block(b),
            StmtKind::LocalVar(d) => self.local(d, sp),
            StmtKind::Ref { ty, mutable, name, init } => {
                self.open("ref", if *mutable { "mut" } else { "" }, sp);
                self.ty(ty);
                self.id(name);
                self.expr(init);
                self.close();
            }
            StmtKind::Tuple(d) => self.tuple_decl(d, sp),
            StmtKind::Expr(es) => {
                self.open("exprstmt", "", sp);
                self.expr_stmt(es);
                self.close();
            }
            StmtKind::Discard(e) => {
                self.open("discard", "", sp);
                self.expr(e);
                self.close();
            }
            StmtKind::If { cond, then, otherwise } => {
                self.open("if", "", sp);
                self.expr(cond);
                self.stmt(then);
                if let Some(o) = otherwise {
                    self.stmt(o);
                }
                self.close();
            }
            StmtKind::While { cond, body } => {
                self.open("while", "", sp);
                self.expr(cond);
                self.stmt(body);
                self.close();
            }
            StmtKind::DoWhile { body, cond } => {
                self.open("dowhile", "", sp);
                self.stmt(body);
                self.expr(cond);
                self.close();
            }
            StmtKind::For { init, cond, update, body } => {
                self.open("for", "", sp);
                match init {
                    None => self.line("noinit", "", None),
                    Some(ForInit::Var(d)) => self.local(d, None),
                    Some(ForInit::Tuple(d)) => self.tuple_decl(d, None),
                    Some(ForInit::Exprs(list)) => self.exprs(list),
                }
                match cond {
                    None => self.line("nocond", "", None),
                    Some(c) => self.expr(c),
                }
                self.exprs(update);
                self.stmt(body);
                self.close();
            }
            StmtKind::ForEach { ty, mutable, name, borrow_mut, iter, body } => {
                self.open("foreach", &flags(&[(*mutable, "mut"), (*borrow_mut, "borrow")]), sp);
                match ty {
                    VarType::Var => self.line("var", "", None),
                    VarType::Type(t) => self.ty(t),
                }
                self.id(name);
                self.expr(iter);
                self.stmt(body);
                self.close();
            }
            StmtKind::Switch { subject, cases } => {
                self.open("switch", "", sp);
                self.expr(subject);
                for c in cases {
                    self.open("case", if c.is_default { "default" } else { "" }, Some(c.span));
                    self.open("patterns", "", None);
                    for p in &c.patterns {
                        match p {
                            Pattern::Literal { negative, lit, .. } => {
                                self.open("plit", if *negative { "-" } else { "" }, None);
                                self.literal(lit, None);
                                self.close();
                            }
                            Pattern::Name { name, bindings } => {
                                self.open("pname", "", None);
                                match name {
                                    PatternName::Simple(i) => self.id(i),
                                    PatternName::Static(s) => self.sref(s),
                                }
                                if let Some(bs) = bindings {
                                    self.open("bindings", "", None);
                                    for b in bs {
                                        self.id(b);
                                    }
                                    self.close();
                                }
                                self.close();
                            }
                        }
                    }
                    self.close();
                    self.open("body", "", None);
                    for s in &c.body {
                        self.stmt(s);
                    }
                    self.close();
                    self.close();
                }
                self.close();
            }
            StmtKind::Return { borrow_mut, value } => {
                self.open("return", if *borrow_mut { "mut" } else { "" }, sp);
                if let Some(v) = value {
                    self.expr(v);
                }
                self.close();
            }
            StmtKind::Break => self.line("break", "", sp),
            StmtKind::Continue => self.line("continue", "", sp),
            StmtKind::Throw(e) => {
                self.open("throw", "", sp);
                self.expr(e);
                self.close();
            }
            StmtKind::Try { body, catches, finally } => {
                self.open("try", "", sp);
                self.block(body);
                for c in catches {
                    self.open("catch", "", None);
                    self.ty(&c.ty);
                    self.id(&c.name);
                    self.block(&c.body);
                    self.close();
                }
                if let Some(f) = finally {
                    self.open("finally", "", None);
                    self.block(f);
                    self.close();
                }
                self.close();
            }
        }
    }

    fn literal(&mut self, lit: &Literal, span: Option<Span>) {
        match lit {
            Literal::Int(v) => self.line("int", v, span),
            Literal::Float(v) => self.line("float", v, span),
            Literal::Str(v) => self.line("str", v, span),
            Literal::Char(v) => self.line("char", v, span),
            Literal::Bool(b) => self.line("bool", if *b { "true" } else { "false" }, span),
            Literal::Null => self.line("null", "", span),
        }
    }

    fn args(&mut self, a: &[Arg]) {
        self.open("args", "", None);
        for x in a {
            self.open("arg", if x.borrow_mut { "mut" } else { "" }, None);
            self.expr(&x.expr);
            self.close();
        }
        self.close();
    }

    fn expr(&mut self, e: &Expr) {
        let sp = Some(e.span);
        match &e.kind {
            ExprKind::Literal(l) => self.literal(l, sp),
            ExprKind::Name(i) => self.line("name", &i.name, sp),
            ExprKind::This => self.line("this", "", sp),
            ExprKind::Base => self.line("base", "", sp),
            ExprKind::Paren(x) => {
                self.open("paren", "", sp);
                self.expr(x);
                self.close();
            }
            ExprKind::Tuple(items) | ExprKind::Array(items) => {
                self.open(if matches!(e.kind, ExprKind::Tuple(_)) { "tuple" } else { "array" }, "", sp);
                for x in items {
                    self.expr(x);
                }
                self.close();
            }
            ExprKind::New { ty, args } => {
                self.open("new", "", sp);
                self.type_name(ty);
                self.args(args);
                self.close();
            }
            ExprKind::Cast { ty, expr } => {
                self.open("cast", "", sp);
                self.ty(ty);
                self.expr(expr);
                self.close();
            }
            ExprKind::Copy(x) => {
                self.open("copy", "", sp);
                self.expr(x);
                self.close();
            }
            ExprKind::Static(s) => {
                self.open("static", "", sp);
                self.sref(s);
                self.close();
            }
            ExprKind::Lambda { own, params, body } => {
                self.open("lambda", if *own { "own" } else { "" }, sp);
                self.params(params);
                match body {
                    LambdaBody::Block(b) => self.block(b),
                    LambdaBody::Expr(x) => self.expr(x),
                }
                self.close();
            }
            ExprKind::Unary { op, expr } => {
                let o = match op {
                    UnaryOp::Not => "!",
                    UnaryOp::BitNot => "~",
                    UnaryOp::Neg => "-",
                    UnaryOp::PreInc => "++",
                    UnaryOp::PreDec => "--",
                };
                self.open("unary", o, sp);
                self.expr(expr);
                self.close();
            }
            ExprKind::Binary { op, left, right } => {
                self.open("binary", binop(*op), sp);
                self.expr(left);
                self.expr(right);
                self.close();
            }
            ExprKind::As { expr, ty } => {
                self.open("as", "", sp);
                self.expr(expr);
                self.ty(ty);
                self.close();
            }
            ExprKind::Coalesce { left, right } => {
                self.open("coalesce", "", sp);
                self.expr(left);
                self.expr(right);
                self.close();
            }
            ExprKind::Conditional { cond, then, otherwise } => {
                self.open("cond", "", sp);
                self.expr(cond);
                self.expr(then);
                self.expr(otherwise);
                self.close();
            }
            ExprKind::Member { object, name, safe } => {
                self.open("member", if *safe { "safe" } else { "" }, sp);
                self.expr(object);
                self.id(name);
                self.close();
            }
            ExprKind::Call { callee, args } => {
                self.open("call", "", sp);
                self.expr(callee);
                self.args(args);
                self.close();
            }
            ExprKind::Index { object, index } => {
                self.open("index", "", sp);
                self.expr(object);
                self.expr(index);
                self.close();
            }
            ExprKind::Slice { object, from, to } => {
                self.open("slice", "", sp);
                self.expr(object);
                match from {
                    Some(x) => self.expr(x),
                    None => self.line("none", "", None),
                }
                match to {
                    Some(x) => self.expr(x),
                    None => self.line("none", "", None),
                }
                self.close();
            }
            ExprKind::PostInc(x) | ExprKind::PostDec(x) => {
                self.open(if matches!(e.kind, ExprKind::PostInc(_)) { "postinc" } else { "postdec" }, "", sp);
                self.expr(x);
                self.close();
            }
        }
    }
}

/// Textform einer Datei.
pub fn unit(u: &CompilationUnit) -> String {
    let mut d = D { out: String::new(), depth: 0 };
    d.unit(u);
    d.out
}
