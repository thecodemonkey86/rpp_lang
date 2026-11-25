//! Syntax von R++: bedingte Kompilierung, Lexer, Parser und AST.
//!
//! Der Einstiegspunkt ist [`parse`]. Er erkennt dieselbe Sprache wie die normative
//! Grammatik `rpp-grammatik/RPP.g4` zusammen mit den Prüfungen, die dort als
//! „späterer Prüfschritt“ vermerkt und im Python-Testlauf umgesetzt sind
//! (erlaubte Formen von Ausdrucksanweisungen, Kapitel 8, R-8.12).

pub mod ast;
pub mod dump;
pub mod lexer;
pub mod parser;
pub mod preprocess;

use std::collections::HashSet;

pub use lexer::Span;
pub use parser::SyntaxError;

/// Liest eine Quelldatei: Direktiven auflösen, zerlegen, parsen, Ausdrucksanweisungen prüfen.
/// Bei einem Fehler wird der erste Fehler geliefert.
pub fn parse(source: &str, symbols: &HashSet<String>) -> Result<ast::CompilationUnit, SyntaxError> {
    let text = preprocess::preprocess(source, symbols).map_err(|e| SyntaxError {
        span: Span { line: e.line, col: 0 },
        message: format!("Directive: {}", e.message),
    })?;
    let tokens = lexer::lex(&text);
    let mut p = parser::Parser::new(tokens);
    let unit = p.compilation_unit().map_err(|_| p.error())?;
    let mut errors = Vec::new();
    check::unit(&unit, &mut errors);
    match errors.into_iter().min_by_key(|e| e.span) {
        Some(e) => Err(e),
        None => Ok(unit),
    }
}

/// Prüft, dass jede Ausdrucksanweisung ohne Zuweisung ein Aufruf oder `++`/`--` ist
/// (R-8.12). Entspricht `check_expr_statements` in `rpp-grammatik/tools/run_tests.py`.
mod check {
    use crate::ast::*;
    use crate::SyntaxError;

    pub fn unit(u: &CompilationUnit, errors: &mut Vec<SyntaxError>) {
        for decl in &u.decls {
            match decl {
                TopDecl::Class(c) => members(&c.members, errors),
                TopDecl::Mixin(m) => members(&m.members, errors),
                TopDecl::Enum(_) => {}
                TopDecl::Main(m) => block(&m.body, errors),
            }
        }
    }

    fn members(ms: &[ClassMember], errors: &mut Vec<SyntaxError>) {
        for m in ms {
            match m {
                ClassMember::Field(f) => {
                    if let Some(e) = &f.init {
                        expr(e, errors);
                    }
                }
                ClassMember::Method(m) => {
                    if let Some(b) = &m.body {
                        block(b, errors);
                    }
                }
                ClassMember::Constructor(c) => {
                    for a in c.base_args.iter().flatten() {
                        expr(&a.expr, errors);
                    }
                    block(&c.body, errors);
                }
                ClassMember::Destructor(b, _) | ClassMember::Copy(_, b, _) => block(b, errors),
                ClassMember::Access(..) | ClassMember::Import(_) => {}
            }
        }
    }

    fn block(b: &Block, errors: &mut Vec<SyntaxError>) {
        for s in &b.stmts {
            stmt(s, errors);
        }
    }

    fn expr_stmt(s: &ExprStmt, span: crate::Span, errors: &mut Vec<SyntaxError>) {
        match s {
            ExprStmt::Assign { target, value, .. } => {
                expr(target, errors);
                expr(value, errors);
            }
            ExprStmt::Expr(e) => {
                let allowed = matches!(
                    e.kind,
                    ExprKind::Call { .. }
                        | ExprKind::PostInc(_)
                        | ExprKind::PostDec(_)
                        | ExprKind::Unary { op: UnaryOp::PreInc | UnaryOp::PreDec, .. }
                );
                if !allowed {
                    errors.push(SyntaxError {
                        span: crate::Span { line: span.line, col: span.col },
                        message: "Expression cannot be used as a statement (only assignments, calls, ++ and -- are allowed)".into(),
                    });
                }
                expr(e, errors);
            }
        }
    }

    fn stmt(s: &Stmt, errors: &mut Vec<SyntaxError>) {
        match &s.kind {
            StmtKind::Block(b) => block(b, errors),
            StmtKind::LocalVar(d) => {
                if let Some(e) = &d.init {
                    expr(e, errors);
                }
            }
            StmtKind::Ref { init, .. } => expr(init, errors),
            StmtKind::Tuple(d) => expr(&d.init, errors),
            StmtKind::Expr(e) => expr_stmt(e, s.span, errors),
            StmtKind::Discard(e) | StmtKind::Throw(e) => expr(e, errors),
            StmtKind::If { cond, then, otherwise } => {
                expr(cond, errors);
                stmt(then, errors);
                if let Some(o) = otherwise {
                    stmt(o, errors);
                }
            }
            StmtKind::While { cond, body } | StmtKind::DoWhile { body, cond } => {
                expr(cond, errors);
                stmt(body, errors);
            }
            StmtKind::For { init, cond, update, body } => {
                match init {
                    Some(ForInit::Var(d)) => {
                        if let Some(e) = &d.init {
                            expr(e, errors);
                        }
                    }
                    Some(ForInit::Tuple(d)) => expr(&d.init, errors),
                    Some(ForInit::Exprs(list)) => {
                        for e in list {
                            expr_stmt(e, s.span, errors);
                        }
                    }
                    None => {}
                }
                if let Some(c) = cond {
                    expr(c, errors);
                }
                for e in update {
                    expr_stmt(e, s.span, errors);
                }
                stmt(body, errors);
            }
            StmtKind::ForEach { iter, body, .. } => {
                expr(iter, errors);
                stmt(body, errors);
            }
            StmtKind::Switch { subject, cases } => {
                expr(subject, errors);
                for c in cases {
                    for s in &c.body {
                        stmt(s, errors);
                    }
                }
            }
            StmtKind::Return { value, .. } => {
                if let Some(v) = value {
                    expr(v, errors);
                }
            }
            StmtKind::Break | StmtKind::Continue => {}
            StmtKind::Try { body, catches, finally } => {
                block(body, errors);
                for c in catches {
                    block(&c.body, errors);
                }
                if let Some(f) = finally {
                    block(f, errors);
                }
            }
        }
    }

    fn expr(e: &Expr, errors: &mut Vec<SyntaxError>) {
        match &e.kind {
            ExprKind::Literal(_) | ExprKind::Name(_) | ExprKind::This | ExprKind::Base | ExprKind::Static(_) => {}
            ExprKind::Paren(x) | ExprKind::Copy(x) | ExprKind::PostInc(x) | ExprKind::PostDec(x) => expr(x, errors),
            ExprKind::Cast { expr: x, .. } | ExprKind::Unary { expr: x, .. } | ExprKind::As { expr: x, .. } => {
                expr(x, errors)
            }
            ExprKind::Member { object, .. } => expr(object, errors),
            ExprKind::Tuple(xs) | ExprKind::Array(xs) => xs.iter().for_each(|x| expr(x, errors)),
            ExprKind::New { args, .. } => args.iter().for_each(|a| expr(&a.expr, errors)),
            ExprKind::Call { callee, args } => {
                expr(callee, errors);
                args.iter().for_each(|a| expr(&a.expr, errors));
            }
            ExprKind::Lambda { body, .. } => match body {
                LambdaBody::Expr(x) => expr(x, errors),
                LambdaBody::Block(b) => block(b, errors),
            },
            ExprKind::Binary { left, right, .. } | ExprKind::Coalesce { left, right } => {
                expr(left, errors);
                expr(right, errors);
            }
            ExprKind::Conditional { cond, then, otherwise } => {
                expr(cond, errors);
                expr(then, errors);
                expr(otherwise, errors);
            }
            ExprKind::Index { object, index } => {
                expr(object, errors);
                expr(index, errors);
            }
            ExprKind::Slice { object, from, to } => {
                expr(object, errors);
                if let Some(f) = from {
                    expr(f, errors);
                }
                if let Some(t) = to {
                    expr(t, errors);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ast::*;
    use super::*;

    /// Liest `x = <ausdruck>;` und gibt den Ausdruck in Klammerschreibweise zurück.
    fn shape(expr: &str) -> String {
        let src = format!("class T {{ void m() {{ x = {expr}; }} }}");
        let unit = parse(&src, &preprocess::default_symbols()).unwrap_or_else(|e| panic!("{expr}: {e}"));
        let TopDecl::Class(c) = &unit.decls[0] else { unreachable!() };
        let ClassMember::Method(m) = &c.members[0] else { unreachable!() };
        let StmtKind::Expr(ExprStmt::Assign { value, .. }) = &m.body.as_ref().unwrap().stmts[0].kind else {
            unreachable!()
        };
        show(value)
    }

    fn show(e: &Expr) -> String {
        match &e.kind {
            ExprKind::Literal(Literal::Int(v)) => v.clone(),
            ExprKind::Literal(Literal::Bool(b)) => b.to_string(),
            ExprKind::Name(n) => n.name.clone(),
            ExprKind::Binary { op, left, right } => format!("({} {:?} {})", show(left), op, show(right)),
            ExprKind::Unary { op, expr } => format!("({op:?} {})", show(expr)),
            ExprKind::As { expr, .. } => format!("({} as T)", show(expr)),
            ExprKind::Coalesce { left, right } => format!("({} ?? {})", show(left), show(right)),
            ExprKind::Conditional { cond, then, otherwise } => {
                format!("({} ? {} : {})", show(cond), show(then), show(otherwise))
            }
            ExprKind::Call { callee, args } => format!("{}({})", show(callee), args.len()),
            ExprKind::Member { object, name, safe } => {
                format!("{}{}{}", show(object), if *safe { "?." } else { "." }, name.name)
            }
            ExprKind::Static(s) => format!("::{}", s.member.name),
            ExprKind::Paren(x) => show(x),
            other => format!("{other:?}"),
        }
    }

    #[test]
    fn vorrang_und_assoziativitaet() {
        assert_eq!(shape("2 + 3 * 4"), "(2 Add (3 Mul 4))");
        assert_eq!(shape("20 - 4 - 3"), "((20 Sub 4) Sub 3)");
        assert_eq!(shape("1 << 2 + 1"), "(1 Shl (2 Add 1))");
        assert_eq!(shape("true || false && false"), "(true Or (false And false))");
        assert_eq!(shape("1 | 2 ^ 3 & 1"), "(1 BitOr (2 BitXor (3 BitAnd 1)))");
        assert_eq!(shape("-2 * 3"), "((Neg 2) Mul 3)");
        assert_eq!(shape("a ?? b ?? c"), "(a ?? (b ?? c))");
        assert_eq!(shape("y ?? z ? 1 : 2"), "((y ?? z) ? 1 : 2)");
        assert_eq!(shape("a + b as Foo"), "((a Add b) as T)");
        assert_eq!(shape("s as Foo < y"), "((s as T) Lt y)");
        assert_eq!(shape("a < b as Foo"), "(a Lt (b as T))");
        assert_eq!(shape("a < b > c.d()"), "((a Lt b) Gt c.d(0))");
        assert_eq!(shape("a >> 2 > b"), "((a Shr 2) Gt b)");
        assert_eq!(shape("s as Foo ? a : b"), "((s as T) ? a : b)");
        assert_eq!(shape("x?.y.z()"), "x?.y.z(0)");
        assert_eq!(shape("i32::MAX - 1"), "(::MAX Sub 1)");
    }
}
