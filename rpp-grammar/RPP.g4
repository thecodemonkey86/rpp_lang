// R++ – ausführbare Grammatik (ANTLR4), Stand: Spezifikationsgerüst Entwurf 0.15
//
// Bedingte Kompilierung (#if / #elif / #else / #endif) wird vor dem Parsen von
// tools/preprocess.py aufgelöst; die Grammatik sieht keine Direktiven.
//
// Regeln, die die Grammatik nicht ausdrückt und die ein späterer Prüfschritt
// übernimmt (im Testlauf: tools/run_tests.py):
//   - Eine Ausdrucksanweisung der Form postfixExpr muss mit einem Aufruf
//     oder mit ++ / -- enden.
//   - "own" ist nur an Parametern und Lambdas erlaubt.
//   - "var" verlangt eine Initialisierung.
//   - "void" ist als Typargument nur an letzter Stelle von Func<...> erlaubt.
//   - Die Größe eines festen Arrays (i32[N]) muss zur Compilezeit feststehen.
//   - Öffentliche Member exportierter Typen verwenden in ihrer Signatur nur
//     exportierte Typen.
//   - Der Initialisierer einer ref-Deklaration ist ein Ort (Variable, Feld-
//     oder Indexzugriff), kein temporärer Wert.
//   - Bei "e as T" ist T eine Klasse oder ein Mixin ohne "?"; das Ergebnis
//     ist nullbar.
//   - Enum-Werte sind eindeutig und liegen im Bereich von i32; ein Enum mit
//     Zahlwerten hat keine Varianten mit Daten.
//   - "&" in Typargumenten nur bei den Parametern von Func, nicht beim
//     Rückgabetyp und nicht bei anderen generischen Typen.
//   - Ein zurückgegebener Borrow (Rückgabetyp ref T) geht von this aus;
//     "return &e;" nur bei ref T&.
//   - Vor "::" steht ein Typ, vor "." ein Wert oder ein Paketname
//     (statische Member, Konstanten und Enum-Varianten nur mit "::").

grammar RPP;

// ---------------------------------------------------------------- Dateien

compilationUnit : packageDecl? usingDecl* (typeDecl | mainDecl)* EOF ;
packageDecl     : PACKAGE qualifiedName ';' ;
// using a.b.C; bindet einen Typ ein, using X = a.b.C; unter anderem Namen. Kein Platzhalter.
usingDecl       : USING (identifier '=')? qualifiedName ';' ;
qualifiedName   : identifier ('.' identifier)* ;
// export macht einen Typ in anderen Projekten nutzbar.
typeDecl        : EXPORT? (classDecl | externClassDecl | handleDecl | mixinDecl | enumDecl) ;
mainDecl        : I32 MAIN '(' STRING '[' ']' identifier ')' block ;

// "main" ist kein reserviertes Wort und darf auch als Name vorkommen.
identifier      : Identifier | MAIN ;

// ------------------------------------------------- Klassen, Mixins, Enums

classDecl
    : ABSTRACT? CLASS identifier typeParams? (':' typeName)? '{' classMember* '}'
    ;
typeParams      : '<' typeParam (',' typeParam)* '>' ;
// Mehrere Constraints werden mit & verbunden: T : Comparable & Printable.
// new() verlangt einen parameterlosen Konstruktor und erlaubt new T().
typeParam       : identifier (':' constraint ('&' constraint)*)? ;
constraint      : typeName | NEW '(' ')' ;

classMember
    : accessLabel
    | fieldDecl
    | methodDecl
    | abstractMethodDecl
    | constructorDecl
    | destructorDecl
    | copyDecl
    | mixinImport
    ;
accessLabel     : (PUBLIC | PRIVATE | PROTECTED) ':' ;
fieldDecl       : (STATIC? CONST)? type identifier ('=' expression)? ';' ;
// Generische Methoden: T max<T : Comparable>(T a, T b). Typargumente werden
// beim Aufruf nur abgeleitet, nie ausgeschrieben.
// nodiscard: Das Ergebnis darf auch mit discard nicht verworfen werden.
methodDecl
    : NODISCARD? (STATIC | VIRTUAL | OVERRIDE)? returnType identifier typeParams?
      '(' paramList? ')' MUT? block
    ;
abstractMethodDecl
    : NODISCARD? ABSTRACT returnType identifier typeParams? '(' paramList? ')' MUT? ';'
    ;
constructorDecl
    : CONSTRUCTOR '(' paramList? ')' (':' BASE '(' argList? ')')? block
    ;
destructorDecl  : DESTRUCTOR '(' ')' block ;
copyDecl        : COPY '(' param ')' block ;
mixinImport     : IMPORT typeName ';' ;

// C-Anbindung (Kapitel 15). Die Grammatik erlaubt in einer extern-Klasse alle Member und
// Methoden ohne Rumpf; welche davon zulässig sind, prüft R-15.2. "handle" ist nur hier ein
// Schlüsselwort (V-15.1) und sonst ein gewöhnlicher Name.
externSpec      : EXTERN '(' StringLiteral ')' ;
externClassDecl
    : externSpec ABSTRACT? CLASS identifier typeParams? (':' typeName)? '{' externMember* '}'
    ;
externMember    : classMember | externMethodDecl ;
externMethodDecl
    : NODISCARD? (STATIC | VIRTUAL | OVERRIDE)? returnType identifier typeParams?
      '(' paramList? ')' MUT? ';'
    ;
handleDecl      : externSpec {self._input.LT(1).text == "handle"}? Identifier identifier '(' identifier ')' ';' ;

mixinDecl       : MIXIN identifier typeParams? '{' mixinMember* '}' ;
mixinMember     : methodDecl | requirement | mixinImport ;
requirement     : NODISCARD? returnType identifier typeParams? '(' paramList? ')' MUT? ';' ;

enumDecl        : ENUM identifier '{' enumVariant (',' enumVariant)* ','? '}' ;
// Eine Variante trägt entweder Daten oder einen festen Zahlwert: Circle(f64 r), Red = 1
enumVariant
    : identifier ('(' paramList ')')?
    | identifier '=' '-'? IntLiteral
    ;

paramList       : param (',' param)* ;
// mut: veränderbar geliehen (void rename(mut Foo f)); ohne mut nur lesend.
param           : MUT? type identifier ;

// ------------------------------------------------------------------ Typen

// ref T / ref mut T : Die Methode liefert einen Borrow, der von this ausgeht (Kapitel 5, R-5.23).
returnType      : VOID | type | REF MUT? baseType typeSuffix* ;
type            : ownership? baseType typeSuffix* ;
ownership       : OWN | SHARED | WEAK ;
baseType        : primitiveType | typeName | tupleType ;
typeName        : qualifiedName typeArgs? ;
typeArgs        : '<' typeArg (',' typeArg)* '>' ;
// mut (veränderbarer Borrow-Parameter) ist nur in den Parametern von Func erlaubt:
// Func<mut Foo, void>. Das prüft ein späterer Schritt.
typeArg         : MUT? type | VOID ;
tupleType       : '(' type ',' type (',' type)* ')' ;
// T[] ist ein Vektor, T[16], T[N] oder T[Typ::N] ein festes Array (N: static const).
// Slices sind der Bibliothekstyp Slice<T>.
typeSuffix      : '?' | '[' arraySize? ']' ;
arraySize       : IntLiteral | identifier | staticRef ;
primitiveType
    : BOOL | CHAR | BYTE | STRING
    | I8 | I16 | I32 | I64
    | UI8 | UI16 | UI32 | UI64
    | F32 | F64
    ;

// ------------------------------------------------------------ Anweisungen

block           : '{' statement* '}' ;
statement
    : block
    | localVarDecl ';'
    | refDecl ';'
    | tupleDecl ';'
    | exprStatement ';'
    | discardStmt ';'
    | ifStmt
    | whileStmt
    | doWhileStmt
    | forStmt
    | forEachStmt
    | switchStmt
    | RETURN (MUT? expression)? ';'
    | BREAK ';'
    | CONTINUE ';'
    | THROW expression ';'
    | tryStmt
    ;

localVarDecl    : CONST? (type | VAR) identifier ('=' expression)? ;
// Benannter Borrow, gilt bis zum Ende des Blocks: lesend ohne Markierung,
// veränderbar mit mut vor Typ und Initialisierer. Keine Besitzform, kein var.
refDecl
    : REF baseType typeSuffix* identifier '=' expression
    | REF MUT baseType typeSuffix* identifier '=' MUT expression
    ;
tupleDecl       : '(' type identifier (',' type identifier)+ ')' '=' expression ;

// Absichtliches Verwerfen eines Ergebnisses: discard list.pop();
discardStmt     : DISCARD expression ;

// Nur Zuweisung, Aufruf, ++ und -- sind als Anweisung erlaubt. Zuweisungen
// gibt es nur hier (und im for-Kopf), nicht als Teil eines Ausdrucks.
exprStatement
    : unaryExpr assignOp expression
    | ('++' | '--') unaryExpr
    | postfixExpr
    ;

ifStmt          : IF '(' expression ')' statement (ELSE statement)? ;
whileStmt       : WHILE '(' expression ')' statement ;
doWhileStmt     : DO statement WHILE '(' expression ')' ';' ;
forStmt         : FOR '(' forInit? ';' expression? ';' forUpdate? ')' statement ;
// Pro Deklaration eine Variable; mehrere Schleifenvariablen per Tupel-Deklaration.
forInit         : localVarDecl | tupleDecl | exprStatement (',' exprStatement)* ;
forUpdate       : exprStatement (',' exprStatement)* ;
forEachStmt
    : FOR '(' MUT? (type | VAR) identifier ':' MUT? expression ')' statement
    ;

switchStmt      : SWITCH '(' expression ')' '{' caseClause* '}' ;
caseClause      : (CASE pattern (',' pattern)* | DEFAULT) ':' statement* ;
// Case-Marken sind Konstanten oder Enum-Varianten, keine beliebigen Ausdrücke.
// Qualifiziert nur mit "::": case Color::Blue, case Shape::Circle(r).
pattern
    : '-'? literal
    | (identifier | staticRef) ('(' identifier (',' identifier)* ')')?
    ;

// Ein try braucht mindestens ein catch oder ein finally.
tryStmt         : TRY block (catchClause+ (FINALLY block)? | FINALLY block) ;
catchClause     : CATCH '(' type identifier ')' block ;

// -------------------------------------------------------------- Ausdrücke

// Ein Lambda steht auf Ausdrucksebene: Sein Rumpf reicht bis zum Ende des
// umgebenden Ausdrucks. Als Operand muss es geklammert werden.
expression
    : lambda
    | conditionalExpr
    ;
assignOp        : '=' | '+=' | '-=' | '*=' | '/=' | '%=' ;

conditionalExpr : coalesceExpr ('?' expression ':' expression)? ;
coalesceExpr    : binaryExpr ('??' coalesceExpr)? ;

// Die Reihenfolge der Alternativen legt den Vorrang fest (oben bindet stärker).
// Schiebeoperatoren bestehen aus zwei Tokens, damit List<List<i32>> lesbar bleibt.
binaryExpr
    : binaryExpr ('*' | '/' | '%') binaryExpr
    | binaryExpr ('+' | '-') binaryExpr
    | binaryExpr ('<' '<' | '>' '>') binaryExpr
    | binaryExpr AS type                    // prüfender Downcast: s as Circle
    | binaryExpr ('<' | '>' | '<=' | '>=') binaryExpr
    | binaryExpr ('==' | '!=') binaryExpr
    | binaryExpr '&' binaryExpr
    | binaryExpr '^' binaryExpr
    | binaryExpr '|' binaryExpr
    | binaryExpr '&&' binaryExpr
    | binaryExpr '||' binaryExpr
    | unaryExpr
    ;

unaryExpr
    : ('!' | '~' | '-' | '++' | '--') unaryExpr
    | postfixExpr
    ;
postfixExpr     : primary postfixOp* ;
postfixOp
    : ('.' | '?.') identifier
    | '(' argList? ')'
    | '[' expression ']'
    | '[' range ']'
    | '++'
    | '--'
    ;

primary
    : literal
    | identifier
    | THIS
    | BASE
    | '(' expression ')'
    | tupleExpr
    | arrayLiteral
    | objectCreation
    | castExpr
    | copyExpr
    | staticRef
    ;
tupleExpr       : '(' expression (',' expression)+ ')' ;
arrayLiteral    : '[' (expression (',' expression)*)? ']' ;
lambda          : OWN? '(' paramList? ')' '=>' (expression | block) ;
objectCreation  : NEW typeName '(' argList? ')' ;
castExpr        : CAST '<' type '>' '(' expression ')' ;
// Explizite Kopie: liefert ein neues Objekt mit eindeutigem Besitz.
copyExpr        : COPY '(' expression ')' ;
// Statischer Zugriff: Methoden, Konstanten und Enum-Varianten eines Typs,
// z. B. Math::max(a, b), Shape::PI, Color::Red, List<Foo>::create().
// Eindeutig, weil in einem Ausdruck nach '>' sonst kein '::' folgen kann.
// Auch an primitiven Typen: i32::MAX, i32::parse(s), string::join(sep, parts).
staticRef       : (qualifiedName typeArgs? | primitiveType) '::' identifier ;

// Bereich für Slices, Ende exklusiv: a[1..3], a[..3], a[1..], a[..]
range           : expression? '..' expression? ;

argList         : argument (',' argument)* ;
argument        : MUT? expression ;

literal
    : IntLiteral | FloatLiteral | StringLiteral | CharLiteral
    | TRUE | FALSE | NULL
    ;

// ------------------------------------------------------------------ Lexer

ABSTRACT : 'abstract' ;   AS : 'as' ;               BASE : 'base' ;           BOOL : 'bool' ;
BREAK : 'break' ;         BYTE : 'byte' ;           CASE : 'case' ;
CAST : 'cast' ;           CATCH : 'catch' ;         CHAR : 'char' ;
CLASS : 'class' ;         CONST : 'const' ;         CONSTRUCTOR : 'constructor' ;
CONTINUE : 'continue' ;   COPY : 'copy' ;           DEFAULT : 'default' ;
DESTRUCTOR : 'destructor' ; DISCARD : 'discard' ;  DO : 'do' ;             ELSE : 'else' ;
ENUM : 'enum' ;           EXPORT : 'export' ;       EXTERN : 'extern' ;       F32 : 'f32' ;
F64 : 'f64' ;
FALSE : 'false' ;         FINALLY : 'finally' ;     FOR : 'for' ;
I8 : 'i8' ;               I16 : 'i16' ;             I32 : 'i32' ;
I64 : 'i64' ;             IF : 'if' ;               IMPORT : 'import' ;
MIXIN : 'mixin' ;         MUT : 'mut' ;             NEW : 'new' ;             NODISCARD : 'nodiscard' ;             NULL : 'null' ;
OVERRIDE : 'override' ;   OWN : 'own' ;             PACKAGE : 'package' ;
PRIVATE : 'private' ;     PROTECTED : 'protected' ; PUBLIC : 'public' ;
REF : 'ref' ;             RETURN : 'return' ;       SHARED : 'shared' ;       STATIC : 'static' ;
STRING : 'string' ;       SWITCH : 'switch' ;       THIS : 'this' ;
THROW : 'throw' ;         TRUE : 'true' ;           TRY : 'try' ;
UI8 : 'ui8' ;             UI16 : 'ui16' ;           UI32 : 'ui32' ;
UI64 : 'ui64' ;           USING : 'using' ;         VAR : 'var' ;
VIRTUAL : 'virtual' ;     VOID : 'void' ;           WEAK : 'weak' ;
WHILE : 'while' ;

MAIN : 'main' ;

Identifier    : [A-Za-z_] [A-Za-z_0-9]* ;
// Ziffern dürfen mit _ getrennt werden (1_000_000), aber nicht am Rand.
FloatLiteral  : Digits '.' Digits ([eE] [+-]? Digits)? ;   // 1.0e10, 2.5E-3; nicht 1e10
IntLiteral    : '0x' HexDigits | Digits ;
fragment Digits    : [0-9] ([0-9_]* [0-9])? ;
fragment HexDigits : [0-9A-Fa-f] ([0-9A-Fa-f_]* [0-9A-Fa-f])? ;
StringLiteral : '"' (~["\\\r\n] | '\\' .)* '"' ;
CharLiteral   : '\'' (~['\\\r\n] | '\\' .) '\'' ;

LineComment   : '//' ~[\r\n]* -> skip ;
BlockComment  : '/*' .*? '*/' -> skip ;
Whitespace    : [ \t\r\n]+ -> skip ;
