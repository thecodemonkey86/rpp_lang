"""Abstrakte Syntax von FR++ (Kapitel 5, Abschnitt 3.1) und Rückübersetzung in Text."""

from dataclasses import dataclass, field
from typing import Optional


# ------------------------------------------------------------------ Typen

@dataclass(frozen=True)
class Type:
    kind: str                 # int | own | shared | weak | ref | mut
    cls: Optional[str] = None

    def __str__(self):
        return "int" if self.kind == "int" else f"{self.kind} {self.cls}"


INT = Type("int")
STORABLE = ("int", "own", "shared", "weak")      # Speichertypen S
BORROW = ("ref", "mut")                          # nur an Parametern und Borrow-Variablen


def brw(kind, cls):
    """brw(ro, C) = ref C, brw(mut, C) = mut C"""
    return Type("ref" if kind == "ro" else "mut", cls)


# Ein Ort ist ein Tupel (x, f1, f2, ...).

def place_str(p):
    return ".".join(p)


# ------------------------------------------------------------ Ausdrücke

@dataclass
class Num:
    value: int
    line: int = 0


@dataclass
class Copy:
    place: tuple
    line: int = 0


@dataclass
class Move:
    var: str
    line: int = 0


@dataclass
class New:
    cls: str
    args: list
    line: int = 0


@dataclass
class Share:
    expr: object
    line: int = 0


@dataclass
class Demote:
    expr: object
    line: int = 0


@dataclass
class Replace:
    place: tuple
    expr: object
    line: int = 0


@dataclass
class Call:
    place: tuple              # Empfänger
    method: str
    args: list                # Ausdrücke oder BorrowArg
    line: int = 0


@dataclass
class BinOp:                  # Erweiterung für Testprogramme: + - <
    op: str
    left: object
    right: object
    line: int = 0


@dataclass
class BorrowArg:
    kind: str                 # ro | mut
    place: tuple
    line: int = 0


# ----------------------------------------------------------- Anweisungen

@dataclass
class Decl:
    type: Type
    var: str
    expr: object
    line: int = 0


@dataclass
class AssignVar:
    var: str
    expr: object
    line: int = 0


@dataclass
class AssignField:
    place: tuple
    expr: object
    line: int = 0


@dataclass
class ExprStmt:
    expr: object
    line: int = 0


@dataclass
class If:
    cond: tuple
    then: list
    els: list
    line: int = 0


@dataclass
class While:
    cond: tuple
    body: list
    line: int = 0


@dataclass
class Borrow:
    var: str
    kind: str
    place: tuple
    body: list
    line: int = 0


@dataclass
class BorrowCall:             # borrow x = p.m(args) { … }  (R-5.24)
    var: str
    call: object
    body: list
    line: int = 0


@dataclass
class Upgrade:
    var: str
    place: tuple
    then: list
    els: list
    line: int = 0


@dataclass
class Unpack:                 # (S₁ x₁, …, Sₙ xₙ) = move y;  (R-5.25)
    vars: list                # [(Type, Name)]
    var: str
    line: int = 0


@dataclass
class Print:                  # Erweiterung für Testprogramme
    expr: object
    line: int = 0


# --------------------------------------------------------- Deklarationen

@dataclass
class Method:
    name: str
    ret: Type
    params: list              # [(Type, Name)]
    mode: str                 # ro | mut
    body: list
    ret_expr: object
    line: int = 0


@dataclass
class Class:
    name: str
    base: Optional[str]
    fields: list              # [(Type, Name)], nur eigene Felder
    methods: dict             # Name -> Method
    destructor: Optional[list] = None
    line: int = 0


@dataclass
class Program:
    classes: dict             # Name -> Class
    main: list = field(default_factory=list)


# ------------------------------------------------------ Rückübersetzung

def expr_str(e):
    if isinstance(e, Num):
        return str(e.value)
    if isinstance(e, Copy):
        return "copy " + place_str(e.place)
    if isinstance(e, Move):
        return "move " + e.var
    if isinstance(e, New):
        return f"new {e.cls}({', '.join(expr_str(a) for a in e.args)})"
    if isinstance(e, Share):
        return f"share ({expr_str(e.expr)})"
    if isinstance(e, Demote):
        return f"demote ({expr_str(e.expr)})"
    if isinstance(e, Replace):
        return f"replace({place_str(e.place)}, {expr_str(e.expr)})"
    if isinstance(e, Call):
        return f"{place_str(e.place)}.{e.method}({', '.join(expr_str(a) for a in e.args)})"
    if isinstance(e, BinOp):
        return f"({expr_str(e.left)} {e.op} {expr_str(e.right)})"
    if isinstance(e, BorrowArg):
        return f"&{e.kind} {place_str(e.place)}"
    raise TypeError(e)


def stmts_str(stmts, ind):
    out = []
    pad = "    " * ind
    for s in stmts:
        if isinstance(s, Decl):
            out.append(f"{pad}{s.type} {s.var} = {expr_str(s.expr)};")
        elif isinstance(s, AssignVar):
            out.append(f"{pad}{s.var} = {expr_str(s.expr)};")
        elif isinstance(s, AssignField):
            out.append(f"{pad}{place_str(s.place)} = {expr_str(s.expr)};")
        elif isinstance(s, ExprStmt):
            out.append(f"{pad}{expr_str(s.expr)};")
        elif isinstance(s, Print):
            out.append(f"{pad}print({expr_str(s.expr)});")
        elif isinstance(s, Unpack):
            vs = ", ".join(f"{t} {x}" for t, x in s.vars)
            out.append(f"{pad}({vs}) = move {s.var};")
        elif isinstance(s, If):
            out.append(f"{pad}if ({place_str(s.cond)}) {{")
            out += stmts_str(s.then, ind + 1)
            out.append(f"{pad}}} else {{")
            out += stmts_str(s.els, ind + 1)
            out.append(f"{pad}}}")
        elif isinstance(s, While):
            out.append(f"{pad}while ({place_str(s.cond)}) {{")
            out += stmts_str(s.body, ind + 1)
            out.append(f"{pad}}}")
        elif isinstance(s, Borrow):
            out.append(f"{pad}borrow {s.var} = &{s.kind} {place_str(s.place)} {{")
            out += stmts_str(s.body, ind + 1)
            out.append(f"{pad}}}")
        elif isinstance(s, BorrowCall):
            out.append(f"{pad}borrow {s.var} = {expr_str(s.call)} {{")
            out += stmts_str(s.body, ind + 1)
            out.append(f"{pad}}}")
        elif isinstance(s, Upgrade):
            out.append(f"{pad}upgrade {s.var} = {place_str(s.place)} {{")
            out += stmts_str(s.then, ind + 1)
            out.append(f"{pad}}} else {{")
            out += stmts_str(s.els, ind + 1)
            out.append(f"{pad}}}")
        else:
            raise TypeError(s)
    return out


def program_str(p):
    out = []
    for c in p.classes.values():
        base = f" : {c.base}" if c.base else ""
        out.append(f"class {c.name}{base} {{")
        for t, f in c.fields:
            out.append(f"    {t} {f};")
        for m in c.methods.values():
            params = ", ".join(f"{t} {x}" for t, x in m.params)
            out.append(f"    {m.ret} {m.name}({params}) {m.mode} {{")
            out += stmts_str(m.body, 2)
            out.append(f"        return {expr_str(m.ret_expr)};")
            out.append("    }")
        if c.destructor is not None:
            out.append("    destructor() {")
            out += stmts_str(c.destructor, 2)
            out.append("    }")
        out.append("}")
    out.append("main {")
    out += stmts_str(p.main, 1)
    out.append("}")
    return "\n".join(out) + "\n"
