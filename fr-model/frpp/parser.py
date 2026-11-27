"""Parser für die Textform von FR++.

    Programm   ::= Klasse* 'main' Block
    Klasse     ::= 'class' C (':' D)? '{' (Feld | Methode | Destruktor)* '}'
    Feld       ::= Typ f ';'
    Methode    ::= Typ m '(' Parameter? ')' ('ro' | 'mut') '{' Anw* 'return' Ausdr ';' '}'
    Destruktor ::= 'destructor' '(' ')' Block
    Typ        ::= 'int' | ('own' | 'shared' | 'weak' | 'ref' | 'mut') C

Anweisungen und Ausdrücke wie in Kapitel 5, Abschnitt 3.1, dazu als
Erweiterung für Tests: print(e); und die int-Operatoren + - <.
"""

import re

from . import ast as A

KEYWORDS = {
    "class", "main", "destructor", "return", "int", "own", "shared", "weak",
    "ref", "mut", "ro", "copy", "move", "new", "share", "demote", "replace",
    "if", "else", "while", "borrow", "upgrade", "print",
}

TOKEN = re.compile(r"(\s+)|(//[^\n]*)|(\d+)|([A-Za-z_]\w*)|(.)")


class ParseError(Exception):
    def __init__(self, msg, line):
        super().__init__(f"Zeile {line}: {msg}")
        self.line = line


def tokenize(src):
    tokens = []
    line = 1
    for m in TOKEN.finditer(src):
        space, _comment, num, ident, sym = m.groups()
        if space:
            line += space.count("\n")
        elif num:
            tokens.append(("num", int(num), line))
        elif ident:
            tokens.append(("kw" if ident in KEYWORDS else "id", ident, line))
        elif sym:
            tokens.append(("sym", sym, line))
    tokens.append(("eof", None, line))
    return tokens


class Parser:
    def __init__(self, src):
        self.toks = tokenize(src)
        self.i = 0

    # -------------------------------------------------------------- Hilfen
    def peek(self, k=0):
        return self.toks[self.i + k]

    def line(self):
        return self.peek()[2]

    def at(self, value, k=0):
        t = self.peek(k)
        return t[0] in ("kw", "sym") and t[1] == value

    def take(self, value):
        if not self.at(value):
            t = self.peek()
            raise ParseError(f"'{value}' erwartet, gefunden {t[1]!r}", t[2])
        self.i += 1

    def ident(self):
        t = self.peek()
        if t[0] != "id":
            raise ParseError(f"Name erwartet, gefunden {t[1]!r}", t[2])
        self.i += 1
        return t[1]

    # -------------------------------------------------------------- Programm
    def program(self):
        classes = {}
        while self.at("class"):
            c = self.klass()
            if c.name in classes:
                raise ParseError(f"Klasse {c.name} doppelt", c.line)
            classes[c.name] = c
        self.take("main")
        main = self.block()
        if self.peek()[0] != "eof":
            raise ParseError("Ende der Datei erwartet", self.line())
        return A.Program(classes, main)

    def klass(self):
        line = self.line()
        self.take("class")
        name = self.ident()
        base = None
        if self.at(":"):
            self.take(":")
            base = self.ident()
        self.take("{")
        fields, methods, destructor = [], {}, None
        while not self.at("}"):
            if self.at("destructor"):
                self.take("destructor")
                self.take("(")
                self.take(")")
                if destructor is not None:
                    raise ParseError("zweiter Destruktor", self.line())
                destructor = self.block()
                continue
            mline = self.line()
            t = self.type()
            n = self.ident()
            if self.at(";"):
                self.take(";")
                fields.append((t, n))
                continue
            self.take("(")
            params = []
            if not self.at(")"):
                params.append((self.type(), self.ident()))
                while self.at(","):
                    self.take(",")
                    params.append((self.type(), self.ident()))
            self.take(")")
            if self.at("ro"):
                mode = "ro"
            elif self.at("mut"):
                mode = "mut"
            else:
                raise ParseError("Modus 'ro' oder 'mut' erwartet", self.line())
            self.i += 1
            self.take("{")
            body = []
            while not self.at("return"):
                body.append(self.stmt())
            self.take("return")
            ret = self.arg(allow_borrow=True)   # &ro p / &mut p bei Borrow-Rückgabe
            self.take(";")
            self.take("}")
            if n in methods:
                raise ParseError(f"Methode {n} doppelt", mline)
            methods[n] = A.Method(n, t, params, mode, body, ret, mline)
        self.take("}")
        return A.Class(name, base, fields, methods, destructor, line)

    def type(self):
        if self.at("int"):
            self.i += 1
            return A.INT
        for k in ("own", "shared", "weak", "ref", "mut"):
            if self.at(k):
                self.i += 1
                return A.Type(k, self.ident())
        raise ParseError(f"Typ erwartet, gefunden {self.peek()[1]!r}", self.line())

    def is_type_at(self, k):
        if self.at("int", k):
            return True
        return any(self.at(w, k) for w in ("own", "shared", "weak")) and self.peek(k + 1)[0] == "id" and self.peek(k + 2)[0] == "id"

    def is_type_start(self):
        if self.at("int"):
            return True
        # 'mut C x' ist ein Typ, aber nur, wenn zwei Namen folgen
        return any(self.at(k) for k in ("own", "shared", "weak", "ref", "mut")) \
            and self.peek(1)[0] == "id" and self.peek(2)[0] == "id"

    # ------------------------------------------------------------ Anweisungen
    def block(self):
        self.take("{")
        stmts = []
        while not self.at("}"):
            stmts.append(self.stmt())
        self.take("}")
        return stmts

    def place(self):
        # 'this' ist im Kalkül ein gewöhnlicher Variablenname.
        p = [self.ident()]
        while self.at(".") and self.peek(1)[0] == "id":
            self.take(".")
            p.append(self.ident())
        return tuple(p)

    def stmt(self):
        line = self.line()
        if self.is_type_start():
            t = self.type()
            if t.kind not in A.STORABLE:
                raise ParseError("lokale Variablen haben einen Speichertyp", line)
            x = self.ident()
            self.take("=")
            e = self.expr()
            self.take(";")
            return A.Decl(t, x, e, line)
        if self.at("(") and self.is_type_at(1):           # (S x, S y) = move z;  (R-5.25)
            self.i += 1
            vars_ = []
            while True:
                t = self.type()
                if t.kind not in A.STORABLE:
                    raise ParseError("Zerlegen bindet an Variablen mit Speichertyp", line)
                vars_.append((t, self.ident()))
                if self.at(","):
                    self.i += 1
                    continue
                break
            self.take(")")
            self.take("=")
            self.take("move")
            y = self.ident()
            self.take(";")
            return A.Unpack(vars_, y, line)
        if self.at("if"):
            self.i += 1
            self.take("(")
            c = self.place()
            self.take(")")
            then = self.block()
            els = []
            if self.at("else"):
                self.i += 1
                els = self.block()
            return A.If(c, then, els, line)
        if self.at("while"):
            self.i += 1
            self.take("(")
            c = self.place()
            self.take(")")
            return A.While(c, self.block(), line)
        if self.at("borrow"):
            self.i += 1
            x = self.ident()
            self.take("=")
            if not self.at("&"):                 # borrow x = p.m(args) { … }
                call = self.expr()
                if not isinstance(call, A.Call):
                    raise ParseError("borrow verlangt &ro/&mut oder einen Methodenaufruf", line)
                return A.BorrowCall(x, call, self.block(), line)
            self.take("&")
            if self.at("ro") or self.at("mut"):
                kind = self.peek()[1]
                self.i += 1
            else:
                raise ParseError("'ro' oder 'mut' erwartet", self.line())
            p = self.place()
            return A.Borrow(x, kind, p, self.block(), line)
        if self.at("upgrade"):
            self.i += 1
            x = self.ident()
            self.take("=")
            p = self.place()
            then = self.block()
            els = []
            if self.at("else"):
                self.i += 1
                els = self.block()
            return A.Upgrade(x, p, then, els, line)
        if self.at("print"):
            self.i += 1
            self.take("(")
            e = self.expr()
            self.take(")")
            self.take(";")
            return A.Print(e, line)
        e = self.expr(allow_place=True)
        if isinstance(e, tuple):          # bloßer Ort: nur als Zuweisungsziel
            self.take("=")
            rhs = self.expr()
            self.take(";")
            if len(e) == 1:
                return A.AssignVar(e[0], rhs, line)
            return A.AssignField(e, rhs, line)
        self.take(";")
        return A.ExprStmt(e, line)

    # -------------------------------------------------------------- Ausdrücke
    def expr(self, allow_place=False):
        left = self.term(allow_place)
        if isinstance(left, tuple):
            if any(self.at(op) for op in "+-<"):
                raise ParseError("Ort ohne copy oder move im Ausdruck", self.line())
            return left
        while any(self.at(op) for op in "+-<"):
            op = self.peek()[1]
            line = self.line()
            self.i += 1
            right = self.term(False)
            left = A.BinOp(op, left, right, line)
        return left

    def term(self, allow_place=False):
        line = self.line()
        t = self.peek()
        if t[0] == "num":
            self.i += 1
            return A.Num(t[1], line)
        if self.at("copy"):
            self.i += 1
            return A.Copy(self.place(), line)
        if self.at("move"):
            self.i += 1
            return A.Move(self.ident(), line)
        if self.at("new"):
            self.i += 1
            c = self.ident()
            return A.New(c, self.args(allow_borrow=False), line)
        if self.at("share"):
            self.i += 1
            return A.Share(self.term(), line)
        if self.at("demote"):
            self.i += 1
            return A.Demote(self.term(), line)
        if self.at("replace"):
            self.i += 1
            self.take("(")
            p = self.place()
            self.take(",")
            e = self.expr()
            self.take(")")
            if len(p) < 2:
                raise ParseError("replace verlangt ein Feld", line)
            return A.Replace(p, e, line)
        if self.at("("):
            self.i += 1
            e = self.expr()
            self.take(")")
            return e
        if t[0] == "id":
            p = self.place()
            if self.at("("):
                if len(p) < 2:
                    raise ParseError("Aufruf ohne Empfänger", line)
                return A.Call(p[:-1], p[-1], self.args(allow_borrow=True), line)
            if not allow_place:
                raise ParseError("Ort ohne copy oder move im Ausdruck", line)
            return p
        raise ParseError(f"Ausdruck erwartet, gefunden {t[1]!r}", line)

    def args(self, allow_borrow):
        self.take("(")
        out = []
        if not self.at(")"):
            out.append(self.arg(allow_borrow))
            while self.at(","):
                self.take(",")
                out.append(self.arg(allow_borrow))
        self.take(")")
        return out

    def arg(self, allow_borrow):
        if self.at("&"):
            line = self.line()
            if not allow_borrow:
                raise ParseError("Borrow nur als Argument eines Methodenaufrufs", line)
            self.i += 1
            if self.at("ro") or self.at("mut"):
                kind = self.peek()[1]
                self.i += 1
            else:
                raise ParseError("'ro' oder 'mut' erwartet", self.line())
            return A.BorrowArg(kind, self.place(), line)
        return self.expr()


def parse(src):
    return Parser(src).program()
