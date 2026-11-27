"""Typprüfer für FR++ nach Kapitel 5, Abschnitt 3.2 und 4 (Regeln R-5.1 bis R-5.22).

Umgebung  Γ: dict Name -> (Typ, verfügbar)
Loans     L: tuple von (Art, Ort), Art in {ro, mut}

Jede Prüffunktion arbeitet funktional: Sie liefert die neue Umgebung zurück und
verändert die alte nicht. Fehler werden als CheckError mit Regelnummer gemeldet.
"""

from . import ast as A


class CheckError(Exception):
    def __init__(self, rule, msg, line):
        super().__init__(f"Zeile {line}: {rule}: {msg}")
        self.rule = rule
        self.msg = msg
        self.line = line


class Checker:
    def __init__(self, prog):
        self.prog = prog
        self.classes = prog.classes

    # ------------------------------------------------------- Klassentabelle
    def chain(self, c):
        """Klasse und ihre Basisklassen, die Klasse selbst zuerst."""
        out = []
        while c is not None:
            out.append(c)
            c = self.classes[c].base
        return out

    def fields(self, c):
        """felder(C): geerbte Felder zuerst, in Deklarationsreihenfolge."""
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

    def subclass(self, c, d):
        return d in self.chain(c)

    def subtype(self, t1, t2):
        if t1.kind != t2.kind:
            return False
        return t1.kind == "int" or self.subclass(t1.cls, t2.cls)

    def check_type(self, t, line, storable):
        if storable and t.kind not in A.STORABLE:
            raise CheckError("R-5.22", f"{t} ist kein Speichertyp", line)
        if t.kind != "int" and t.cls not in self.classes:
            raise CheckError("R-5.22", f"unbekannte Klasse {t.cls}", line)

    # --------------------------------------------------------- Orte (3.2)
    @staticmethod
    def obj_class(t, is_var):
        """Klasse eines Objektorts oder None (V-2: shared nur an der Wurzel)."""
        if t.kind in ("own", "ref", "mut"):
            return t.cls
        if t.kind == "shared" and is_var:
            return t.cls
        return None

    def place_type(self, env, p, line):
        root = p[0]
        if root not in env:
            raise CheckError("O-Var", f"unbekannte Variable {root}", line)
        t, avail = env[root]
        if not avail:
            raise CheckError("O-Var", f"{root} ist verschoben", line)
        for i, f in enumerate(p[1:]):
            c = self.obj_class(t, i == 0)
            if c is None:
                raise CheckError("O-Feld", f"{A.place_str(p[:i + 1])} ist kein Objektort "
                                 f"(Typ {t}); Zugriff durch ein shared- oder weak-Feld "
                                 "nur über eine lokale Variable (V-2)", line)
            ft = self.field_type(c, f)
            if ft is None:
                raise CheckError("O-Feld", f"Klasse {c} hat kein Feld {f}", line)
            t = ft
        return t

    def obj(self, env, p, line):
        t = self.place_type(env, p, line)
        return self.obj_class(t, len(p) == 1)

    @staticmethod
    def writable(env, p):
        return env[p[0]][0].kind != "ref"

    @staticmethod
    def overlap(env, p, q):
        if p[0] != q[0]:
            return False
        if p[:len(q)] == q or q[:len(p)] == p:
            return True
        return env[p[0]][0].kind == "shared"

    def require_allowed(self, env, loans, kind, p, rule, line, what):
        for k2, q in loans:
            if self.overlap(env, p, q) and not (kind == "ro" and k2 == "ro"):
                art = "veränderbar" if k2 == "mut" else "lesend"
                raise CheckError(rule, f"{what} {A.place_str(p)} nicht erlaubt: "
                                 f"{A.place_str(q)} ist {art} geliehen", line)

    # -------------------------------------------------------- Ausdrücke (4.1)
    def expr(self, env, loans, e):
        """Γ; L ⊢ e : S ⊣ Γ'  ->  (S, Γ')"""
        line = e.line
        if isinstance(e, A.Num):                                        # R-5.1
            return A.INT, env
        if isinstance(e, A.BinOp):
            t1, env = self.expr(env, loans, e.left)
            t2, env = self.expr(env, loans, e.right)
            if t1 != A.INT or t2 != A.INT:
                raise CheckError("R-5.1", f"Operator {e.op} nur für int", line)
            return A.INT, env
        if isinstance(e, A.Copy):                                       # R-5.2
            t = self.place_type(env, e.place, line)
            if t.kind not in ("int", "shared", "weak"):
                raise CheckError("R-5.2", f"{A.place_str(e.place)} hat Typ {t}; kopierbar "
                                 "sind nur int, shared und weak", line)
            self.require_allowed(env, loans, "ro", e.place, "R-5.2", line, "Lesen von")
            return t, env
        if isinstance(e, A.Move):                                       # R-5.3
            if e.var not in env:
                raise CheckError("O-Var", f"unbekannte Variable {e.var}", line)
            t, avail = env[e.var]
            if not avail:
                raise CheckError("R-5.3", f"{e.var} ist bereits verschoben", line)
            if t.kind not in ("own", "shared"):
                raise CheckError("R-5.3", f"{e.var} hat Typ {t} und kann nicht "
                                 "verschoben werden", line)
            self.require_allowed(env, loans, "mut", (e.var,), "R-5.3", line, "Move von")
            env = dict(env)
            env[e.var] = (t, False)
            return t, env
        if isinstance(e, A.New):                                        # R-5.4
            if e.cls not in self.classes:
                raise CheckError("R-5.4", f"unbekannte Klasse {e.cls}", line)
            fs = self.fields(e.cls)
            if len(fs) != len(e.args):
                raise CheckError("R-5.4", f"new {e.cls} verlangt {len(fs)} Werte", line)
            for (ft, fn), a in zip(fs, e.args):
                if isinstance(a, A.BorrowArg):
                    raise CheckError("R-5.4", "Borrows können nicht in Felder", line)
                t, env = self.expr(env, loans, a)
                if not self.subtype(t, ft):
                    raise CheckError("R-5.4", f"Feld {fn} verlangt {ft}, nicht {t}", line)
            return A.Type("own", e.cls), env
        if isinstance(e, A.Share):                                      # R-5.5
            t, env = self.expr(env, loans, e.expr)
            if t.kind != "own":
                raise CheckError("R-5.5", f"share verlangt own, nicht {t}", line)
            return A.Type("shared", t.cls), env
        if isinstance(e, A.Demote):                                     # R-5.6
            t, env = self.expr(env, loans, e.expr)
            if t.kind != "shared":
                raise CheckError("R-5.6", f"demote verlangt shared, nicht {t}", line)
            return A.Type("weak", t.cls), env
        if isinstance(e, A.Replace):                                    # R-5.7
            t, env = self.expr(env, loans, e.expr)
            ft = self.place_type(env, e.place, line)
            if not self.subtype(t, ft):
                raise CheckError("R-5.7", f"{A.place_str(e.place)} hat Typ {ft}, nicht {t}", line)
            if not self.writable(env, e.place):
                raise CheckError("R-5.7", f"{e.place[0]} ist nur lesend geliehen", line)
            self.require_allowed(env, loans, "mut", e.place, "R-5.7", line, "Schreiben von")
            return ft, env
        if isinstance(e, A.Call):                                       # R-5.10
            t, env, _ = self.call(env, loans, e)
            if t.kind in A.BORROW:
                raise CheckError("R-5.24", f"{e.method} liefert einen Borrow; das Ergebnis "
                                 "muss mit borrow x = … { … } gebunden werden", e.line)
            return t, env
        if isinstance(e, A.BorrowArg):
            raise CheckError("R-5.9", "Borrow nur als Argument", line)
        raise TypeError(e)

    # ------------------------------------------- Aufrufe und Argumente (4.2)
    def call(self, env, loans, e):
        line = e.line
        c = self.obj(env, e.place, line)
        if c is None:
            raise CheckError("R-5.10", f"{A.place_str(e.place)} ist kein Objektort", line)
        m = self.method(c, e.method)
        if m is None:
            raise CheckError("R-5.10", f"Klasse {c} hat keine Methode {e.method}", line)
        if m.mode == "mut" and not self.writable(env, e.place):
            raise CheckError("R-5.10", f"{e.method} verändert den Empfänger, "
                             f"{e.place[0]} ist aber nur lesend geliehen", line)
        self.require_allowed(env, loans, m.mode, e.place, "R-5.10", line,
                             "Aufruf auf" if m.mode == "ro" else "veränderbarer Aufruf auf")
        loans = loans + ((m.mode, e.place),)
        if len(m.params) != len(e.args):
            raise CheckError("R-5.10", f"{e.method} verlangt {len(m.params)} Argumente", line)
        for (pt, pn), a in zip(m.params, e.args):
            if pt.kind in A.BORROW:                                     # R-5.9
                want = "ro" if pt.kind == "ref" else "mut"
                if not isinstance(a, A.BorrowArg) or a.kind != want:
                    raise CheckError("R-5.10", f"Parameter {pn} verlangt &{want}", line)
                ac = self.obj(env, a.place, line)
                if ac is None:
                    raise CheckError("R-5.9", f"{A.place_str(a.place)} ist kein Objektort", line)
                if not self.subclass(ac, pt.cls):
                    raise CheckError("R-5.9", f"Parameter {pn} verlangt {pt.cls}, nicht {ac}", line)
                if want == "mut" and not self.writable(env, a.place):
                    raise CheckError("R-5.9", f"{a.place[0]} ist nur lesend geliehen", line)
                self.require_allowed(env, loans, want, a.place, "R-5.9", line, "Borrow von")
                loans = loans + ((want, a.place),)
            else:                                                       # R-5.8
                if isinstance(a, A.BorrowArg):
                    raise CheckError("R-5.10", f"Parameter {pn} verlangt einen Wert vom Typ {pt}", line)
                t, env = self.expr(env, loans, a)
                if not self.subtype(t, pt):
                    raise CheckError("R-5.10", f"Parameter {pn} verlangt {pt}, nicht {t}", line)
        return m.ret, env, m

    # ------------------------------------------------------- Anweisungen (4.3)
    def block(self, env, loans, stmts):
        out = self.stmts(env, loans, stmts)
        return {k: out[k] for k in env}

    @staticmethod
    def join(e1, e2):
        return {k: (e1[k][0], e1[k][1] and e2[k][1]) for k in e1}

    def stmts(self, env, loans, stmts):                                 # R-5.19
        for s in stmts:
            env = self.stmt(env, loans, s)
        return env

    def cond(self, env, loans, p, rule, line):
        t = self.place_type(env, p, line)
        if t != A.INT:
            raise CheckError(rule, "Bedingung muss int sein", line)
        self.require_allowed(env, loans, "ro", p, rule, line, "Lesen von")

    def stmt(self, env, loans, s):
        line = s.line
        if isinstance(s, A.Decl):                                       # R-5.11
            self.check_type(s.type, line, storable=True)
            if s.var in env:
                raise CheckError("R-5.11", f"{s.var} ist schon deklariert", line)
            t, env = self.expr(env, loans, s.expr)
            if not self.subtype(t, s.type):
                raise CheckError("R-5.11", f"{s.var} hat Typ {s.type}, nicht {t}", line)
            env = dict(env)
            env[s.var] = (s.type, True)
            return env
        if isinstance(s, A.AssignVar):                                  # R-5.12
            if s.var not in env:
                raise CheckError("O-Var", f"unbekannte Variable {s.var}", line)
            vt = env[s.var][0]
            if vt.kind not in A.STORABLE:
                raise CheckError("R-5.12", f"{s.var} ist eine Borrow-Variable und kann "
                                 "nicht neu belegt werden", line)
            t, env = self.expr(env, loans, s.expr)
            if not self.subtype(t, vt):
                raise CheckError("R-5.12", f"{s.var} hat Typ {vt}, nicht {t}", line)
            self.require_allowed(env, loans, "mut", (s.var,), "R-5.12", line, "Zuweisung an")
            env = dict(env)
            env[s.var] = (vt, True)
            return env
        if isinstance(s, A.AssignField):                                # R-5.13
            _, env = self.expr(env, loans, A.Replace(s.place, s.expr, line))
            return env
        if isinstance(s, A.ExprStmt):                                   # R-5.14
            _, env = self.expr(env, loans, s.expr)
            return env
        if isinstance(s, A.Unpack):                                     # R-5.25
            if s.var not in env:
                raise CheckError("O-Var", f"unbekannte Variable {s.var}", line)
            t, avail = env[s.var]
            if not avail:
                raise CheckError("R-5.25", f"{s.var} ist verschoben", line)
            if t.kind != "own":
                raise CheckError("R-5.25", f"zerlegen lässt sich nur own, nicht {t}", line)
            if any(self.classes[k].destructor is not None for k in self.chain(t.cls)):
                raise CheckError("R-5.25", f"{t.cls} hat einen Destruktor", line)
            fs = self.fields(t.cls)
            if len(fs) != len(s.vars):
                raise CheckError("R-5.25", f"{t.cls} hat {len(fs)} Felder", line)
            self.require_allowed(env, loans, "mut", (s.var,), "R-5.25", line, "Zerlegen von")
            env = dict(env)
            for (ft, fn), (vt, x) in zip(fs, s.vars):
                self.check_type(vt, line, storable=True)
                if not self.subtype(ft, vt):
                    raise CheckError("R-5.25", f"Feld {fn} hat Typ {ft}, nicht {vt}", line)
                if x in env:
                    raise CheckError("R-5.25", f"{x} ist schon deklariert", line)
                env[x] = (vt, True)
            env[s.var] = (t, False)
            return env
        if isinstance(s, A.Print):
            t, env = self.expr(env, loans, s.expr)
            if t != A.INT:
                raise CheckError("R-5.14", "print verlangt int", line)
            return env
        if isinstance(s, A.If):                                         # R-5.15
            self.cond(env, loans, s.cond, "R-5.15", line)
            return self.join(self.block(env, loans, s.then), self.block(env, loans, s.els))
        if isinstance(s, A.While):                                      # R-5.16
            self.cond(env, loans, s.cond, "R-5.16", line)
            after = self.block(env, loans, s.body)
            for x, (_, avail) in env.items():
                if avail and not after[x][1]:
                    raise CheckError("R-5.16", f"{x} wird in der Schleife verschoben "
                                     "und nicht neu belegt", line)
            return env
        if isinstance(s, A.Borrow):                                     # R-5.17
            c = self.obj(env, s.place, line)
            if c is None:
                raise CheckError("R-5.17", f"{A.place_str(s.place)} ist kein Objektort", line)
            if s.kind == "mut" and not self.writable(env, s.place):
                raise CheckError("R-5.17", f"{s.place[0]} ist nur lesend geliehen", line)
            self.require_allowed(env, loans, s.kind, s.place, "R-5.17", line, "Borrow von")
            if s.var in env:
                raise CheckError("R-5.17", f"{s.var} ist schon deklariert", line)
            inner = dict(env)
            inner[s.var] = (A.brw(s.kind, c), True)
            out = self.stmts(inner, loans + ((s.kind, s.place),), s.body)
            return {k: out[k] for k in env}
        if isinstance(s, A.BorrowCall):                                 # R-5.24
            t, env2, m = self.call(env, loans, s.call)
            if t.kind not in A.BORROW:
                raise CheckError("R-5.24", f"{s.call.method} liefert keinen Borrow", line)
            if s.var in env2:
                raise CheckError("R-5.24", f"{s.var} ist schon deklariert", line)
            # Der Empfänger bleibt für den ganzen Block mit dem Modus der Methode geliehen.
            inner = dict(env2)
            inner[s.var] = (t, True)
            out = self.stmts(inner, loans + ((m.mode, s.call.place),), s.body)
            return {k: out[k] for k in env2}
        if isinstance(s, A.Upgrade):                                    # R-5.18
            t = self.place_type(env, s.place, line)
            if t.kind != "weak":
                raise CheckError("R-5.18", f"upgrade verlangt weak, nicht {t}", line)
            self.require_allowed(env, loans, "ro", s.place, "R-5.18", line, "Lesen von")
            if s.var in env:
                raise CheckError("R-5.18", f"{s.var} ist schon deklariert", line)
            inner = dict(env)
            inner[s.var] = (A.Type("shared", t.cls), True)
            then = self.stmts(inner, loans, s.then)
            return self.join({k: then[k] for k in env}, self.block(env, loans, s.els))
        raise TypeError(s)

    # ------------------------------------------------ Methoden, Klassen (4.4)
    def check_classes(self):
        for c in self.classes.values():
            if c.base is not None and c.base not in self.classes:
                raise CheckError("R-5.22", f"unbekannte Basisklasse {c.base}", c.line)
        for c in self.classes.values():                 # keine zyklische Vererbung
            seen, k = set(), c.name
            while k is not None:
                if k in seen:
                    raise CheckError("R-5.22", f"zyklische Vererbung bei {c.name}", c.line)
                seen.add(k)
                k = self.classes[k].base
        for c in self.classes.values():
            names = [n for _, n in self.fields(c.name)]
            if len(names) != len(set(names)):
                raise CheckError("R-5.22", f"doppeltes Feld in {c.name}", c.line)
            for t, _ in c.fields:
                self.check_type(t, c.line, storable=True)
            for m in c.methods.values():
                self.check_type(m.ret, m.line, storable=m.ret.kind not in A.BORROW)
                for t, _ in m.params:
                    self.check_type(t, m.line, storable=False)
                if c.base is not None:                                  # R-5.22
                    sup = self.method(c.base, m.name)
                    if sup is not None and ([t for t, _ in sup.params] != [t for t, _ in m.params]
                                            or sup.ret != m.ret or sup.mode != m.mode):
                        raise CheckError("R-5.22", f"{c.name}.{m.name} überschreibt mit "
                                         "anderer Signatur", m.line)

    def check_method(self, c, m):                                       # R-5.20
        env = {"this": (A.brw(m.mode, c.name), True)}
        for t, x in m.params:
            if x in env:
                raise CheckError("R-5.20", f"Parameter {x} doppelt", m.line)
            env[x] = (t, True)
        env = self.stmts(env, (), m.body)
        if m.ret.kind in A.BORROW:                                      # R-5.23
            self.check_borrow_return(env, c, m)
            return
        if isinstance(m.ret_expr, A.BorrowArg):
            raise CheckError("R-5.20", f"{m.name} liefert einen Wert, keinen Borrow", m.line)
        t, _ = self.expr(env, (), m.ret_expr)
        if not self.subtype(t, m.ret):
            raise CheckError("R-5.20", f"{m.name} liefert {t} statt {m.ret}", m.ret_expr.line or m.line)

    def check_borrow_return(self, env, c, m):                          # R-5.23
        e = m.ret_expr
        want = "ro" if m.ret.kind == "ref" else "mut"
        if not isinstance(e, A.BorrowArg) or e.kind != want:
            raise CheckError("R-5.23", f"{m.name} muss mit return &{want} … enden", m.line)
        if e.place[0] != "this":
            raise CheckError("R-5.23", "ein zurückgegebener Borrow muss von this ausgehen, "
                             f"nicht von {e.place[0]}", e.line or m.line)
        if want == "mut" and m.mode != "mut":
            raise CheckError("R-5.23", "ein veränderbarer Borrow verlangt eine &-Methode", m.line)
        oc = self.obj(env, e.place, e.line or m.line)
        if oc is None or not self.subclass(oc, m.ret.cls):
            raise CheckError("R-5.23", f"{A.place_str(e.place)} ist kein Objektort vom Typ {m.ret.cls}",
                             e.line or m.line)

    def check(self):
        self.check_classes()
        for c in self.classes.values():
            for m in c.methods.values():
                self.check_method(c, m)
            if c.destructor is not None:                                # R-5.21
                self.stmts({"this": (A.Type("mut", c.name), True)}, (), c.destructor)
        self.stmts({}, (), self.prog.main)


def check(prog):
    Checker(prog).check()
