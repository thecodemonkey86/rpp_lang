"""Übersetzung von FR++ in die Oberflächensprache R++ (Kapitel 5, Abschnitt 7, rückwärts).

Gebraucht für den Vergleich des Modells mit der ersten Implementierung `rppc`
(Meilenstein M3): Ein FR++-Programm und seine Übersetzung sollen gleich beurteilt
werden – beide angenommen oder beide abgelehnt.

Manche FR++-Konstrukte haben an der Oberfläche keine Entsprechung, weil die
Oberfläche Moves und Kopien nicht ausdrücklich schreibt. `move x` für eine int-,
weak- oder shared-Variable ist an der Oberfläche eine Kopie (V-9); die Übersetzung
ersetzt es deshalb auch im FR++-Programm durch `copy x` (Normalform), und der
Vergleich prüft das veränderte Programm. Ohne Entsprechung bleiben `share e` mit
einem e, das nicht own ist, `demote e` mit einem e, das nicht shared ist, eine
Zuweisung an einen Parameter ohne own (R-8.19) und Pfade durch shared-Felder (V-2,
an der Oberfläche erlaubt). Dafür wird `Untranslatable` geworfen; solche Programme
zählt der Vergleich gesondert.

Abbildung (Abschnitt 7):
    int / own C / shared C / weak C      i32 / C / shared C / weak C
    Parameter ref C / mut C              C / mut C
    copy p                               p (int, shared, weak); sonst copy(p) – eine
                                         Kopie gibt es nur mit copy-Konstruktor, den
                                         die Übersetzung nicht erzeugt (R-5.2 ↔ R-7.11)
    move x, share e, demote e            x, e, e (das Ziel bestimmt Move und Besitzform;
                                         move eines int-, weak- oder shared-Werts → copy)
    replace(p.f, e)                      replace(mut p.f, e)
    p.m(&ro q, &mut r, e)                p.m(q, mut r, e)
    a < b                                (a < b ? 1 : 0)
    e;                                   discard e; bei Aufrufen, sonst T tmpN = e;
    if (p) …  while (p) …                if (p != 0) …  while (p != 0) …
    borrow x = &ro p { … }               { ref C x = p; … }
    borrow x = &mut p { … }              { ref mut C x = mut p; … }
    borrow x = p.m(…) { … }              { ref C x = p.m(…); … } bzw. ref mut C x = mut p.m(…)
    (S₁ x₁, …, Sₙ xₙ) = move y;          (T₁ x₁, …, Tₙ xₙ) = y;  (R-5.25)
    upgrade x = p { … } else { … }       { shared C? x = p.lock(); if (x != null) { … } else { … } }
    demote (share e)                     FrHilfe::weak_C(e): shared-Variable, dann weak (V-6, V-13)
    new C(e₁, …, eₙ)                     new C(e₁, …, eₙ) mit einem Konstruktor, der alle
                                         Felder setzt (geerbte über base(…))
    Methode mit Modus mut                Methode mit mut nach der Parameterliste
    brw(ro, C) m() / brw(mut, C) m()     ref C m() / ref mut C m() mut, return q; bzw. return mut q;
"""

from . import ast as A


class Untranslatable(Exception):
    pass


def rtype(t):
    if t.kind == "int":
        return "i32"
    if t.kind == "own":
        return t.cls
    if t.kind in ("shared", "weak"):
        return f"{t.kind} {t.cls}"
    if t.kind == "ref":
        return t.cls
    if t.kind == "mut":
        return f"mut {t.cls}"
    raise Untranslatable(f"Typ {t}")


def param_type(t):
    if t.kind == "own":
        return f"own {t.cls}"
    return rtype(t)


class Translator:
    def __init__(self, prog):
        self.prog = prog
        self.classes = prog.classes
        self.tmp = 0
        self.params = {}
        self.normalized = 0

    # ------------------------------------------------------- Klassentabelle
    def chain(self, c):
        out = []
        while c is not None and c in self.classes:
            out.append(c)
            c = self.classes[c].base
        return out

    def fields(self, c):
        out = []
        for k in reversed(self.chain(c)):
            out += self.classes[k].fields
        return out

    def field_type(self, c, f):
        for t, n in self.fields(c):
            if n == f:
                return t
        return None

    def method(self, c, m):
        for k in self.chain(c):
            if m in self.classes[k].methods:
                return self.classes[k].methods[m]
        return None

    # ------------------------------------------------------------- Typen
    def place_type(self, env, p):
        t = env.get(p[0])
        for f in p[1:]:
            if t is None or t.kind == "int":
                return None
            t = self.field_type(t.cls, f)
        return t

    def expr_type(self, env, e):
        if isinstance(e, (A.Num, A.BinOp)):
            return A.INT
        if isinstance(e, A.Copy):
            return self.place_type(env, e.place)
        if isinstance(e, A.Move):
            return env.get(e.var)
        if isinstance(e, A.New):
            return A.Type("own", e.cls)
        if isinstance(e, A.Share):
            t = self.expr_type(env, e.expr)
            return t and A.Type("shared", t.cls)
        if isinstance(e, A.Demote):
            t = self.expr_type(env, e.expr)
            return t and A.Type("weak", t.cls)
        if isinstance(e, A.Replace):
            return self.place_type(env, e.place)
        if isinstance(e, A.Call):
            t = self.place_type(env, e.place)
            m = t and t.kind != "int" and self.method(t.cls, e.method)
            return m.ret if m else None
        return None

    # --------------------------------------------------------- Ausdrücke
    def place(self, p, env=None):
        if env is not None:
            t = env.get(p[0])
            for i, f in enumerate(p[1:]):
                if t is None or t.kind == "int":
                    break
                if t.kind == "shared" and i > 0:
                    # V-2: an der Oberfläche erlaubt (Hilfsvariable des Compilers), in FR++ nicht
                    raise Untranslatable("Pfad durch ein shared-Feld (V-2)")
                t = self.field_type(t.cls, f)
        return ".".join(p)

    def expr(self, env, e):
        if isinstance(e, A.Num):
            return str(e.value)
        if isinstance(e, A.BinOp):
            l, r = self.expr(env, e.left), self.expr(env, e.right)
            if e.op == "<":
                return f"({l} < {r} ? 1 : 0)"
            return f"({l} {e.op} {r})"
        if isinstance(e, A.Copy):
            t = self.place_type(env, e.place)
            if t is not None and t.kind in ("int", "shared", "weak"):
                return self.place(e.place, env)
            return f"copy({self.place(e.place, env)})"
        if isinstance(e, A.Move):
            t = env.get(e.var)
            if t is not None and t.kind in ("int", "weak", "shared"):
                # An der Oberfläche werden int-, weak- und shared-Werte immer kopiert (V-9).
                # Normalform: Das FR++-Programm selbst bekommt copy statt move.
                e.__class__ = A.Copy
                e.place = (e.var,)
                self.normalized += 1
            return e.var
        if isinstance(e, A.New):
            return f"new {e.cls}({', '.join(self.arg(env, a) for a in e.args)})"
        if isinstance(e, A.Share):
            t = self.expr_type(env, e.expr)
            if t is None or t.kind != "own":
                raise Untranslatable(f"share mit Typ {t}")
            return self.expr(env, e.expr)
        if isinstance(e, A.Demote):
            t = self.expr_type(env, e.expr)
            if t is None or t.kind != "shared":
                raise Untranslatable(f"demote mit Typ {t}")
            if isinstance(e.expr, A.Share):
                # V-13: weak entsteht nur aus shared; an der Oberfläche über eine shared-Variable.
                return f"FrHilfe::weak_{t.cls}({self.expr(env, e.expr.expr)})"
            return self.expr(env, e.expr)
        if isinstance(e, A.Replace):
            return f"replace(mut {self.place(e.place, env)}, {self.expr(env, e.expr)})"
        if isinstance(e, A.Call):
            return f"{self.place(e.place, env)}.{e.method}({', '.join(self.arg(env, a) for a in e.args)})"
        if isinstance(e, A.BorrowArg):
            raise Untranslatable("Borrow als Ausdruck")
        raise Untranslatable(type(e).__name__)

    def arg(self, env, a):
        if isinstance(a, A.BorrowArg):
            return ("mut " if a.kind == "mut" else "") + self.place(a.place, env)
        return self.expr(env, a)

    # ------------------------------------------------------- Anweisungen
    def block(self, env, stmts, ind):
        env = dict(env)
        out = []
        for s in stmts:
            out += self.stmt(env, s, ind)
        return out

    def stmt(self, env, s, ind):
        pad = "    " * ind
        if isinstance(s, A.Decl):
            env[s.var] = s.type
            return [f"{pad}{rtype(s.type)} {s.var} = {self.expr(env, s.expr)};"]
        if isinstance(s, A.AssignVar):
            if s.var in self.params and self.params[s.var].kind in ("int", "shared", "weak"):
                # R-8.19: an der Oberfläche sind Parameter ohne own nur lesbar
                raise Untranslatable(f"Zuweisung an den Parameter {s.var}")
            return [f"{pad}{s.var} = {self.expr(env, s.expr)};"]
        if isinstance(s, A.AssignField):
            return [f"{pad}{self.place(s.place, env)} = {self.expr(env, s.expr)};"]
        if isinstance(s, A.ExprStmt):
            e = s.expr
            t = self.expr_type(env, e)
            if isinstance(e, A.Call):
                if t is not None and t.kind in A.BORROW:
                    raise Untranslatable("ungebundene Borrow-Rückgabe als Anweisung")
                return [f"{pad}discard {self.expr(env, e)};"]
            if t is None:
                raise Untranslatable("Ausdrucksanweisung ohne Typ")
            self.tmp += 1
            return [f"{pad}{rtype(t)} tmp{self.tmp} = {self.expr(env, e)};"]
        if isinstance(s, A.Print):
            return [f"{pad}print(({self.expr(env, s.expr)}).toString());"]
        if isinstance(s, A.Unpack):
            for t, x in s.vars:
                env[x] = t
            vs = ", ".join(f"{rtype(t)} {x}" for t, x in s.vars)
            return [f"{pad}({vs}) = {s.var};"]
        if isinstance(s, A.If):
            return ([f"{pad}if ({self.place(s.cond, env)} != 0) {{"] + self.block(env, s.then, ind + 1)
                    + [f"{pad}}} else {{"] + self.block(env, s.els, ind + 1) + [f"{pad}}}"])
        if isinstance(s, A.While):
            return ([f"{pad}while ({self.place(s.cond, env)} != 0) {{"] + self.block(env, s.body, ind + 1)
                    + [f"{pad}}}"])
        if isinstance(s, A.Borrow):
            t = self.place_type(env, s.place)
            if t is None or t.kind == "int" or (t.kind in ("shared", "weak") and len(s.place) > 1):
                raise Untranslatable(f"borrow auf {self.place(s.place, env)} ohne Objekttyp")
            inner = dict(env)
            inner[s.var] = A.brw(s.kind, t.cls)
            head = (f"ref {t.cls} {s.var} = {self.place(s.place, env)};" if s.kind == "ro"
                    else f"ref mut {t.cls} {s.var} = mut {self.place(s.place, env)};")
            return [f"{pad}{{", f"{pad}    {head}"] + self.block(inner, s.body, ind + 1) + [f"{pad}}}"]
        if isinstance(s, A.BorrowCall):
            t = self.expr_type(env, s.call)
            if t is None or t.kind not in A.BORROW:
                raise Untranslatable("borrow eines Aufrufs ohne Borrow-Rückgabe")
            inner = dict(env)
            inner[s.var] = t
            call = self.expr(env, s.call)
            head = f"ref {t.cls} {s.var} = {call};" if t.kind == "ref" else f"ref mut {t.cls} {s.var} = mut {call};"
            return [f"{pad}{{", f"{pad}    {head}"] + self.block(inner, s.body, ind + 1) + [f"{pad}}}"]
        if isinstance(s, A.Upgrade):
            t = self.place_type(env, s.place)
            if t is None or t.kind != "weak":
                raise Untranslatable("upgrade ohne weak")
            inner = dict(env)
            inner[s.var] = A.Type("shared", t.cls)
            return ([f"{pad}{{", f"{pad}    shared {t.cls}? {s.var} = {self.place(s.place, env)}.lock();",
                     f"{pad}    if ({s.var} != null) {{"] + self.block(inner, s.then, ind + 2)
                    + [f"{pad}    }} else {{"] + self.block(env, s.els, ind + 2) + [f"{pad}    }}", f"{pad}}}"])
        raise Untranslatable(type(s).__name__)

    # ------------------------------------------------------ Deklarationen
    def overridden(self, cname, mname):
        """Wird die Methode in einer Unterklasse überschrieben?"""
        for c in self.classes.values():
            if c.name != cname and cname in self.chain(c.name)[1:] and mname in c.methods:
                return True
        return False

    def method_text(self, c, m):
        params = ", ".join(f"{param_type(t)} {x}" for t, x in m.params)
        mut = " mut" if m.mode == "mut" else ""
        inherited = c.base is not None and self.method(c.base, m.name) is not None
        mod = "override " if inherited else ("virtual " if self.overridden(c.name, m.name) else "")
        env = {"this": A.brw(m.mode, c.name)}
        env.update({x: t for t, x in m.params})
        self.params = {x: t for t, x in m.params}
        body = self.block(env, m.body, 2)
        # Variablen aus dem Rumpf sind für return sichtbar
        for s in m.body:
            if isinstance(s, A.Decl):
                env[s.var] = s.type
        if m.ret.kind in A.BORROW:
            e = m.ret_expr
            if not isinstance(e, A.BorrowArg):
                raise Untranslatable("Borrow-Methode ohne &-Rückgabe")
            ret = ("ref mut " if m.ret.kind == "mut" else "ref ") + m.ret.cls
            body.append(f"        return {'mut ' if e.kind == 'mut' else ''}{self.place(e.place, env)};")
        else:
            if isinstance(m.ret_expr, A.BorrowArg):
                raise Untranslatable("Wert-Methode mit &-Rückgabe")
            ret = rtype(m.ret)
            body.append(f"        return {self.expr(env, m.ret_expr)};")
        return [f"    {mod}{ret} {m.name}({params}){mut} {{"] + body + ["    }"]

    def class_text(self, c):
        base = f" : {c.base}" if c.base else ""
        out = [f"class {c.name}{base} {{", "public:"]
        for t, f in c.fields:
            out.append(f"    {rtype(t)} {f};")
        allf = self.fields(c.name)
        inherited = len(allf) - len(c.fields)
        params = ", ".join(f"{param_type(t)} p_{f}" for t, f in allf)
        base_args = f" : base({', '.join('p_' + f for _, f in allf[:inherited])})" if c.base else ""
        sets = " ".join(f"this.{f} = p_{f};" for _, f in c.fields)
        out.append(f"    constructor({params}){base_args} {{ {sets} }}")
        for m in c.methods.values():
            out += self.method_text(c, m)
        if c.destructor is not None:
            self.params = {}
            out.append("    destructor() {")
            out += self.block({"this": A.Type("mut", c.name)}, c.destructor, 2)
            out.append("    }")
        out.append("}")
        return out

    def program(self):
        out = []
        for c in self.classes.values():
            out += self.class_text(c) + [""]
        out.append("class FrHilfe {")
        out.append("public:")
        for c in self.classes:
            out.append(f"    static weak {c} weak_{c}(own {c} x) {{ shared {c} s = x; return s; }}")
        out += ["}", ""]
        self.params = {}
        out.append("i32 main(string[] args) {")
        out += self.block({}, self.prog.main, 1)
        out.append("    return 0;")
        out.append("}")
        return "\n".join(out) + "\n"


def translate(prog):
    """R++-Quelltext zu einem FR++-Programm; wirft Untranslatable.

    Ersetzt im Programm `move x` für int-, weak- und shared-Variablen durch
    `copy x` (Normalform der Oberfläche) und liefert (Text, Anzahl der Ersetzungen).
    Das veränderte Programm ist das, mit dem rppc zu vergleichen ist."""
    tr = Translator(prog)
    text = tr.program()
    return text, tr.normalized
