from antlr4 import *
if "." in __name__:
    from .RPPParser import RPPParser
else:
    from RPPParser import RPPParser

# This class defines a complete listener for a parse tree produced by RPPParser.
class RPPListener(ParseTreeListener):

    # Enter a parse tree produced by RPPParser#compilationUnit.
    def enterCompilationUnit(self, ctx:RPPParser.CompilationUnitContext):
        pass

    # Exit a parse tree produced by RPPParser#compilationUnit.
    def exitCompilationUnit(self, ctx:RPPParser.CompilationUnitContext):
        pass


    # Enter a parse tree produced by RPPParser#packageDecl.
    def enterPackageDecl(self, ctx:RPPParser.PackageDeclContext):
        pass

    # Exit a parse tree produced by RPPParser#packageDecl.
    def exitPackageDecl(self, ctx:RPPParser.PackageDeclContext):
        pass


    # Enter a parse tree produced by RPPParser#usingDecl.
    def enterUsingDecl(self, ctx:RPPParser.UsingDeclContext):
        pass

    # Exit a parse tree produced by RPPParser#usingDecl.
    def exitUsingDecl(self, ctx:RPPParser.UsingDeclContext):
        pass


    # Enter a parse tree produced by RPPParser#qualifiedName.
    def enterQualifiedName(self, ctx:RPPParser.QualifiedNameContext):
        pass

    # Exit a parse tree produced by RPPParser#qualifiedName.
    def exitQualifiedName(self, ctx:RPPParser.QualifiedNameContext):
        pass


    # Enter a parse tree produced by RPPParser#typeDecl.
    def enterTypeDecl(self, ctx:RPPParser.TypeDeclContext):
        pass

    # Exit a parse tree produced by RPPParser#typeDecl.
    def exitTypeDecl(self, ctx:RPPParser.TypeDeclContext):
        pass


    # Enter a parse tree produced by RPPParser#mainDecl.
    def enterMainDecl(self, ctx:RPPParser.MainDeclContext):
        pass

    # Exit a parse tree produced by RPPParser#mainDecl.
    def exitMainDecl(self, ctx:RPPParser.MainDeclContext):
        pass


    # Enter a parse tree produced by RPPParser#identifier.
    def enterIdentifier(self, ctx:RPPParser.IdentifierContext):
        pass

    # Exit a parse tree produced by RPPParser#identifier.
    def exitIdentifier(self, ctx:RPPParser.IdentifierContext):
        pass


    # Enter a parse tree produced by RPPParser#classDecl.
    def enterClassDecl(self, ctx:RPPParser.ClassDeclContext):
        pass

    # Exit a parse tree produced by RPPParser#classDecl.
    def exitClassDecl(self, ctx:RPPParser.ClassDeclContext):
        pass


    # Enter a parse tree produced by RPPParser#typeParams.
    def enterTypeParams(self, ctx:RPPParser.TypeParamsContext):
        pass

    # Exit a parse tree produced by RPPParser#typeParams.
    def exitTypeParams(self, ctx:RPPParser.TypeParamsContext):
        pass


    # Enter a parse tree produced by RPPParser#typeParam.
    def enterTypeParam(self, ctx:RPPParser.TypeParamContext):
        pass

    # Exit a parse tree produced by RPPParser#typeParam.
    def exitTypeParam(self, ctx:RPPParser.TypeParamContext):
        pass


    # Enter a parse tree produced by RPPParser#constraint.
    def enterConstraint(self, ctx:RPPParser.ConstraintContext):
        pass

    # Exit a parse tree produced by RPPParser#constraint.
    def exitConstraint(self, ctx:RPPParser.ConstraintContext):
        pass


    # Enter a parse tree produced by RPPParser#classMember.
    def enterClassMember(self, ctx:RPPParser.ClassMemberContext):
        pass

    # Exit a parse tree produced by RPPParser#classMember.
    def exitClassMember(self, ctx:RPPParser.ClassMemberContext):
        pass


    # Enter a parse tree produced by RPPParser#accessLabel.
    def enterAccessLabel(self, ctx:RPPParser.AccessLabelContext):
        pass

    # Exit a parse tree produced by RPPParser#accessLabel.
    def exitAccessLabel(self, ctx:RPPParser.AccessLabelContext):
        pass


    # Enter a parse tree produced by RPPParser#fieldDecl.
    def enterFieldDecl(self, ctx:RPPParser.FieldDeclContext):
        pass

    # Exit a parse tree produced by RPPParser#fieldDecl.
    def exitFieldDecl(self, ctx:RPPParser.FieldDeclContext):
        pass


    # Enter a parse tree produced by RPPParser#methodDecl.
    def enterMethodDecl(self, ctx:RPPParser.MethodDeclContext):
        pass

    # Exit a parse tree produced by RPPParser#methodDecl.
    def exitMethodDecl(self, ctx:RPPParser.MethodDeclContext):
        pass


    # Enter a parse tree produced by RPPParser#abstractMethodDecl.
    def enterAbstractMethodDecl(self, ctx:RPPParser.AbstractMethodDeclContext):
        pass

    # Exit a parse tree produced by RPPParser#abstractMethodDecl.
    def exitAbstractMethodDecl(self, ctx:RPPParser.AbstractMethodDeclContext):
        pass


    # Enter a parse tree produced by RPPParser#constructorDecl.
    def enterConstructorDecl(self, ctx:RPPParser.ConstructorDeclContext):
        pass

    # Exit a parse tree produced by RPPParser#constructorDecl.
    def exitConstructorDecl(self, ctx:RPPParser.ConstructorDeclContext):
        pass


    # Enter a parse tree produced by RPPParser#destructorDecl.
    def enterDestructorDecl(self, ctx:RPPParser.DestructorDeclContext):
        pass

    # Exit a parse tree produced by RPPParser#destructorDecl.
    def exitDestructorDecl(self, ctx:RPPParser.DestructorDeclContext):
        pass


    # Enter a parse tree produced by RPPParser#copyDecl.
    def enterCopyDecl(self, ctx:RPPParser.CopyDeclContext):
        pass

    # Exit a parse tree produced by RPPParser#copyDecl.
    def exitCopyDecl(self, ctx:RPPParser.CopyDeclContext):
        pass


    # Enter a parse tree produced by RPPParser#mixinImport.
    def enterMixinImport(self, ctx:RPPParser.MixinImportContext):
        pass

    # Exit a parse tree produced by RPPParser#mixinImport.
    def exitMixinImport(self, ctx:RPPParser.MixinImportContext):
        pass


    # Enter a parse tree produced by RPPParser#mixinDecl.
    def enterMixinDecl(self, ctx:RPPParser.MixinDeclContext):
        pass

    # Exit a parse tree produced by RPPParser#mixinDecl.
    def exitMixinDecl(self, ctx:RPPParser.MixinDeclContext):
        pass


    # Enter a parse tree produced by RPPParser#mixinMember.
    def enterMixinMember(self, ctx:RPPParser.MixinMemberContext):
        pass

    # Exit a parse tree produced by RPPParser#mixinMember.
    def exitMixinMember(self, ctx:RPPParser.MixinMemberContext):
        pass


    # Enter a parse tree produced by RPPParser#requirement.
    def enterRequirement(self, ctx:RPPParser.RequirementContext):
        pass

    # Exit a parse tree produced by RPPParser#requirement.
    def exitRequirement(self, ctx:RPPParser.RequirementContext):
        pass


    # Enter a parse tree produced by RPPParser#enumDecl.
    def enterEnumDecl(self, ctx:RPPParser.EnumDeclContext):
        pass

    # Exit a parse tree produced by RPPParser#enumDecl.
    def exitEnumDecl(self, ctx:RPPParser.EnumDeclContext):
        pass


    # Enter a parse tree produced by RPPParser#enumVariant.
    def enterEnumVariant(self, ctx:RPPParser.EnumVariantContext):
        pass

    # Exit a parse tree produced by RPPParser#enumVariant.
    def exitEnumVariant(self, ctx:RPPParser.EnumVariantContext):
        pass


    # Enter a parse tree produced by RPPParser#paramList.
    def enterParamList(self, ctx:RPPParser.ParamListContext):
        pass

    # Exit a parse tree produced by RPPParser#paramList.
    def exitParamList(self, ctx:RPPParser.ParamListContext):
        pass


    # Enter a parse tree produced by RPPParser#param.
    def enterParam(self, ctx:RPPParser.ParamContext):
        pass

    # Exit a parse tree produced by RPPParser#param.
    def exitParam(self, ctx:RPPParser.ParamContext):
        pass


    # Enter a parse tree produced by RPPParser#returnType.
    def enterReturnType(self, ctx:RPPParser.ReturnTypeContext):
        pass

    # Exit a parse tree produced by RPPParser#returnType.
    def exitReturnType(self, ctx:RPPParser.ReturnTypeContext):
        pass


    # Enter a parse tree produced by RPPParser#type.
    def enterType(self, ctx:RPPParser.TypeContext):
        pass

    # Exit a parse tree produced by RPPParser#type.
    def exitType(self, ctx:RPPParser.TypeContext):
        pass


    # Enter a parse tree produced by RPPParser#ownership.
    def enterOwnership(self, ctx:RPPParser.OwnershipContext):
        pass

    # Exit a parse tree produced by RPPParser#ownership.
    def exitOwnership(self, ctx:RPPParser.OwnershipContext):
        pass


    # Enter a parse tree produced by RPPParser#baseType.
    def enterBaseType(self, ctx:RPPParser.BaseTypeContext):
        pass

    # Exit a parse tree produced by RPPParser#baseType.
    def exitBaseType(self, ctx:RPPParser.BaseTypeContext):
        pass


    # Enter a parse tree produced by RPPParser#typeName.
    def enterTypeName(self, ctx:RPPParser.TypeNameContext):
        pass

    # Exit a parse tree produced by RPPParser#typeName.
    def exitTypeName(self, ctx:RPPParser.TypeNameContext):
        pass


    # Enter a parse tree produced by RPPParser#typeArgs.
    def enterTypeArgs(self, ctx:RPPParser.TypeArgsContext):
        pass

    # Exit a parse tree produced by RPPParser#typeArgs.
    def exitTypeArgs(self, ctx:RPPParser.TypeArgsContext):
        pass


    # Enter a parse tree produced by RPPParser#typeArg.
    def enterTypeArg(self, ctx:RPPParser.TypeArgContext):
        pass

    # Exit a parse tree produced by RPPParser#typeArg.
    def exitTypeArg(self, ctx:RPPParser.TypeArgContext):
        pass


    # Enter a parse tree produced by RPPParser#tupleType.
    def enterTupleType(self, ctx:RPPParser.TupleTypeContext):
        pass

    # Exit a parse tree produced by RPPParser#tupleType.
    def exitTupleType(self, ctx:RPPParser.TupleTypeContext):
        pass


    # Enter a parse tree produced by RPPParser#typeSuffix.
    def enterTypeSuffix(self, ctx:RPPParser.TypeSuffixContext):
        pass

    # Exit a parse tree produced by RPPParser#typeSuffix.
    def exitTypeSuffix(self, ctx:RPPParser.TypeSuffixContext):
        pass


    # Enter a parse tree produced by RPPParser#arraySize.
    def enterArraySize(self, ctx:RPPParser.ArraySizeContext):
        pass

    # Exit a parse tree produced by RPPParser#arraySize.
    def exitArraySize(self, ctx:RPPParser.ArraySizeContext):
        pass


    # Enter a parse tree produced by RPPParser#primitiveType.
    def enterPrimitiveType(self, ctx:RPPParser.PrimitiveTypeContext):
        pass

    # Exit a parse tree produced by RPPParser#primitiveType.
    def exitPrimitiveType(self, ctx:RPPParser.PrimitiveTypeContext):
        pass


    # Enter a parse tree produced by RPPParser#block.
    def enterBlock(self, ctx:RPPParser.BlockContext):
        pass

    # Exit a parse tree produced by RPPParser#block.
    def exitBlock(self, ctx:RPPParser.BlockContext):
        pass


    # Enter a parse tree produced by RPPParser#statement.
    def enterStatement(self, ctx:RPPParser.StatementContext):
        pass

    # Exit a parse tree produced by RPPParser#statement.
    def exitStatement(self, ctx:RPPParser.StatementContext):
        pass


    # Enter a parse tree produced by RPPParser#localVarDecl.
    def enterLocalVarDecl(self, ctx:RPPParser.LocalVarDeclContext):
        pass

    # Exit a parse tree produced by RPPParser#localVarDecl.
    def exitLocalVarDecl(self, ctx:RPPParser.LocalVarDeclContext):
        pass


    # Enter a parse tree produced by RPPParser#refDecl.
    def enterRefDecl(self, ctx:RPPParser.RefDeclContext):
        pass

    # Exit a parse tree produced by RPPParser#refDecl.
    def exitRefDecl(self, ctx:RPPParser.RefDeclContext):
        pass


    # Enter a parse tree produced by RPPParser#tupleDecl.
    def enterTupleDecl(self, ctx:RPPParser.TupleDeclContext):
        pass

    # Exit a parse tree produced by RPPParser#tupleDecl.
    def exitTupleDecl(self, ctx:RPPParser.TupleDeclContext):
        pass


    # Enter a parse tree produced by RPPParser#discardStmt.
    def enterDiscardStmt(self, ctx:RPPParser.DiscardStmtContext):
        pass

    # Exit a parse tree produced by RPPParser#discardStmt.
    def exitDiscardStmt(self, ctx:RPPParser.DiscardStmtContext):
        pass


    # Enter a parse tree produced by RPPParser#exprStatement.
    def enterExprStatement(self, ctx:RPPParser.ExprStatementContext):
        pass

    # Exit a parse tree produced by RPPParser#exprStatement.
    def exitExprStatement(self, ctx:RPPParser.ExprStatementContext):
        pass


    # Enter a parse tree produced by RPPParser#ifStmt.
    def enterIfStmt(self, ctx:RPPParser.IfStmtContext):
        pass

    # Exit a parse tree produced by RPPParser#ifStmt.
    def exitIfStmt(self, ctx:RPPParser.IfStmtContext):
        pass


    # Enter a parse tree produced by RPPParser#whileStmt.
    def enterWhileStmt(self, ctx:RPPParser.WhileStmtContext):
        pass

    # Exit a parse tree produced by RPPParser#whileStmt.
    def exitWhileStmt(self, ctx:RPPParser.WhileStmtContext):
        pass


    # Enter a parse tree produced by RPPParser#doWhileStmt.
    def enterDoWhileStmt(self, ctx:RPPParser.DoWhileStmtContext):
        pass

    # Exit a parse tree produced by RPPParser#doWhileStmt.
    def exitDoWhileStmt(self, ctx:RPPParser.DoWhileStmtContext):
        pass


    # Enter a parse tree produced by RPPParser#forStmt.
    def enterForStmt(self, ctx:RPPParser.ForStmtContext):
        pass

    # Exit a parse tree produced by RPPParser#forStmt.
    def exitForStmt(self, ctx:RPPParser.ForStmtContext):
        pass


    # Enter a parse tree produced by RPPParser#forInit.
    def enterForInit(self, ctx:RPPParser.ForInitContext):
        pass

    # Exit a parse tree produced by RPPParser#forInit.
    def exitForInit(self, ctx:RPPParser.ForInitContext):
        pass


    # Enter a parse tree produced by RPPParser#forUpdate.
    def enterForUpdate(self, ctx:RPPParser.ForUpdateContext):
        pass

    # Exit a parse tree produced by RPPParser#forUpdate.
    def exitForUpdate(self, ctx:RPPParser.ForUpdateContext):
        pass


    # Enter a parse tree produced by RPPParser#forEachStmt.
    def enterForEachStmt(self, ctx:RPPParser.ForEachStmtContext):
        pass

    # Exit a parse tree produced by RPPParser#forEachStmt.
    def exitForEachStmt(self, ctx:RPPParser.ForEachStmtContext):
        pass


    # Enter a parse tree produced by RPPParser#switchStmt.
    def enterSwitchStmt(self, ctx:RPPParser.SwitchStmtContext):
        pass

    # Exit a parse tree produced by RPPParser#switchStmt.
    def exitSwitchStmt(self, ctx:RPPParser.SwitchStmtContext):
        pass


    # Enter a parse tree produced by RPPParser#caseClause.
    def enterCaseClause(self, ctx:RPPParser.CaseClauseContext):
        pass

    # Exit a parse tree produced by RPPParser#caseClause.
    def exitCaseClause(self, ctx:RPPParser.CaseClauseContext):
        pass


    # Enter a parse tree produced by RPPParser#pattern.
    def enterPattern(self, ctx:RPPParser.PatternContext):
        pass

    # Exit a parse tree produced by RPPParser#pattern.
    def exitPattern(self, ctx:RPPParser.PatternContext):
        pass


    # Enter a parse tree produced by RPPParser#tryStmt.
    def enterTryStmt(self, ctx:RPPParser.TryStmtContext):
        pass

    # Exit a parse tree produced by RPPParser#tryStmt.
    def exitTryStmt(self, ctx:RPPParser.TryStmtContext):
        pass


    # Enter a parse tree produced by RPPParser#catchClause.
    def enterCatchClause(self, ctx:RPPParser.CatchClauseContext):
        pass

    # Exit a parse tree produced by RPPParser#catchClause.
    def exitCatchClause(self, ctx:RPPParser.CatchClauseContext):
        pass


    # Enter a parse tree produced by RPPParser#expression.
    def enterExpression(self, ctx:RPPParser.ExpressionContext):
        pass

    # Exit a parse tree produced by RPPParser#expression.
    def exitExpression(self, ctx:RPPParser.ExpressionContext):
        pass


    # Enter a parse tree produced by RPPParser#assignOp.
    def enterAssignOp(self, ctx:RPPParser.AssignOpContext):
        pass

    # Exit a parse tree produced by RPPParser#assignOp.
    def exitAssignOp(self, ctx:RPPParser.AssignOpContext):
        pass


    # Enter a parse tree produced by RPPParser#conditionalExpr.
    def enterConditionalExpr(self, ctx:RPPParser.ConditionalExprContext):
        pass

    # Exit a parse tree produced by RPPParser#conditionalExpr.
    def exitConditionalExpr(self, ctx:RPPParser.ConditionalExprContext):
        pass


    # Enter a parse tree produced by RPPParser#coalesceExpr.
    def enterCoalesceExpr(self, ctx:RPPParser.CoalesceExprContext):
        pass

    # Exit a parse tree produced by RPPParser#coalesceExpr.
    def exitCoalesceExpr(self, ctx:RPPParser.CoalesceExprContext):
        pass


    # Enter a parse tree produced by RPPParser#binaryExpr.
    def enterBinaryExpr(self, ctx:RPPParser.BinaryExprContext):
        pass

    # Exit a parse tree produced by RPPParser#binaryExpr.
    def exitBinaryExpr(self, ctx:RPPParser.BinaryExprContext):
        pass


    # Enter a parse tree produced by RPPParser#unaryExpr.
    def enterUnaryExpr(self, ctx:RPPParser.UnaryExprContext):
        pass

    # Exit a parse tree produced by RPPParser#unaryExpr.
    def exitUnaryExpr(self, ctx:RPPParser.UnaryExprContext):
        pass


    # Enter a parse tree produced by RPPParser#postfixExpr.
    def enterPostfixExpr(self, ctx:RPPParser.PostfixExprContext):
        pass

    # Exit a parse tree produced by RPPParser#postfixExpr.
    def exitPostfixExpr(self, ctx:RPPParser.PostfixExprContext):
        pass


    # Enter a parse tree produced by RPPParser#postfixOp.
    def enterPostfixOp(self, ctx:RPPParser.PostfixOpContext):
        pass

    # Exit a parse tree produced by RPPParser#postfixOp.
    def exitPostfixOp(self, ctx:RPPParser.PostfixOpContext):
        pass


    # Enter a parse tree produced by RPPParser#primary.
    def enterPrimary(self, ctx:RPPParser.PrimaryContext):
        pass

    # Exit a parse tree produced by RPPParser#primary.
    def exitPrimary(self, ctx:RPPParser.PrimaryContext):
        pass


    # Enter a parse tree produced by RPPParser#tupleExpr.
    def enterTupleExpr(self, ctx:RPPParser.TupleExprContext):
        pass

    # Exit a parse tree produced by RPPParser#tupleExpr.
    def exitTupleExpr(self, ctx:RPPParser.TupleExprContext):
        pass


    # Enter a parse tree produced by RPPParser#arrayLiteral.
    def enterArrayLiteral(self, ctx:RPPParser.ArrayLiteralContext):
        pass

    # Exit a parse tree produced by RPPParser#arrayLiteral.
    def exitArrayLiteral(self, ctx:RPPParser.ArrayLiteralContext):
        pass


    # Enter a parse tree produced by RPPParser#lambda.
    def enterLambda(self, ctx:RPPParser.LambdaContext):
        pass

    # Exit a parse tree produced by RPPParser#lambda.
    def exitLambda(self, ctx:RPPParser.LambdaContext):
        pass


    # Enter a parse tree produced by RPPParser#objectCreation.
    def enterObjectCreation(self, ctx:RPPParser.ObjectCreationContext):
        pass

    # Exit a parse tree produced by RPPParser#objectCreation.
    def exitObjectCreation(self, ctx:RPPParser.ObjectCreationContext):
        pass


    # Enter a parse tree produced by RPPParser#castExpr.
    def enterCastExpr(self, ctx:RPPParser.CastExprContext):
        pass

    # Exit a parse tree produced by RPPParser#castExpr.
    def exitCastExpr(self, ctx:RPPParser.CastExprContext):
        pass


    # Enter a parse tree produced by RPPParser#copyExpr.
    def enterCopyExpr(self, ctx:RPPParser.CopyExprContext):
        pass

    # Exit a parse tree produced by RPPParser#copyExpr.
    def exitCopyExpr(self, ctx:RPPParser.CopyExprContext):
        pass


    # Enter a parse tree produced by RPPParser#staticRef.
    def enterStaticRef(self, ctx:RPPParser.StaticRefContext):
        pass

    # Exit a parse tree produced by RPPParser#staticRef.
    def exitStaticRef(self, ctx:RPPParser.StaticRefContext):
        pass


    # Enter a parse tree produced by RPPParser#range.
    def enterRange(self, ctx:RPPParser.RangeContext):
        pass

    # Exit a parse tree produced by RPPParser#range.
    def exitRange(self, ctx:RPPParser.RangeContext):
        pass


    # Enter a parse tree produced by RPPParser#argList.
    def enterArgList(self, ctx:RPPParser.ArgListContext):
        pass

    # Exit a parse tree produced by RPPParser#argList.
    def exitArgList(self, ctx:RPPParser.ArgListContext):
        pass


    # Enter a parse tree produced by RPPParser#argument.
    def enterArgument(self, ctx:RPPParser.ArgumentContext):
        pass

    # Exit a parse tree produced by RPPParser#argument.
    def exitArgument(self, ctx:RPPParser.ArgumentContext):
        pass


    # Enter a parse tree produced by RPPParser#literal.
    def enterLiteral(self, ctx:RPPParser.LiteralContext):
        pass

    # Exit a parse tree produced by RPPParser#literal.
    def exitLiteral(self, ctx:RPPParser.LiteralContext):
        pass



del RPPParser