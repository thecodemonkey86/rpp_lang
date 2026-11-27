"""Interpreter für FR++ nach Kapitel 5, Abschnitt 5 (Big-Step-Semantik).

Werte:  ("int", n) | ("own", l) | ("sh", l) | ("wk", l) | ("ref", l) | BOT
Heap:   l -> Obj  oder  DEAD (freigegeben, †)

Der Interpreter prüft jeden Schritt defensiv. Jede Situation, für die das
Kalkül keine Regel hat (Zugriff auf †, doppelte Freigabe, Lesen von ⊥,
falsche Wertart), löst `Stuck` aus: Das ist ein Fehlerzustand im Sinne von
Satz 5.1. Ein Sperrkonflikt löst `BorrowError` aus (⚡).

Mit check_invariants=True werden nach jeder Anweisung die Invarianten
I-1, I-2, I-4 und I-5 aus Abschnitt 6.2 geprüft.
"""

from collections import Counter

BOT = ("bot",)
DEAD = "†"


class Stuck(Exception):
    """Fehlerzustand: Das wohlgetypte Programm hätte hier nicht ankommen dürfen."""


class BorrowError(Exception):
    """⚡: Sperrkonflikt bei einem shared-Objekt (zulässiges Ergebnis)."""


class OutOfFuel(Exception):
    """Schrittbudget erschöpft (wird wie Nichtterminierung behandelt)."""


class InvariantViolation(Exception):
    pass


class Obj:
    __slots__ = ("cls", "fields", "k")

    def __init__(self, cls, fields):
        self.cls = cls
        self.fields = fields      # dict Name -> Wert, geerbte Felder zuerst
        self.k = None             # None bei eindeutigem Besitz, sonst [rc, beta]
                                  # beta: 0 frei, n > 0 lesend(n), -1 schreibend


class Interp:
    def __init__(self, prog, fuel=20000, check_invariants=False, checker=None):
        from .checker import Checker
        self.prog = prog
        self.ct = checker or Checker(prog)
        self.heap = {}
        self.next = 0
        self.frames = []
        self.temps = []           # Werte, die gerade zwischen zwei Orten unterwegs sind
        self.dropping = set()     # Objekte, deren Freigabe gerade läuft
        self.out = []
        self.fuel = fuel
        self.inv = check_invariants
        self.shared_used = False
        self.stats = Counter()

    # ------------------------------------------------------------ Hilfen
    def tick(self):
        self.fuel -= 1
        if self.fuel < 0:
            raise OutOfFuel()

    def live(self, l):
        o = self.heap.get(l)
        if o is None:
            raise Stuck(f"Adresse {l} existiert nicht")
        if o is DEAD:
            raise Stuck(f"Zugriff auf freigegebenes Objekt {l}")
        return o

    def lookup(self, cls, name):
        m = self.ct.method(cls, name)
        if m is None:
            raise Stuck(f"Methode {name} fehlt in {cls}")
        return m

    def var(self, frame, x):
        if x not in frame:
            raise Stuck(f"unbekannte Variable {x}")
        return frame[x]

    # --------------------------------------------------------- Orte (5.2)
    def ziel(self, frame, p):
        v = self.var(frame, p[0])
        if v[0] not in ("own", "sh", "ref"):
            raise Stuck(f"{p[0]} verweist auf kein Objekt ({v[0]})")
        l = v[1]
        self.live(l)
        for f in p[1:]:
            o = self.live(l)
            if f not in o.fields:
                raise Stuck(f"Feld {f} fehlt")
            fv = o.fields[f]
            if fv[0] != "own":
                raise Stuck(f"Feld {f} ist kein own-Feld ({fv[0]})")
            l = fv[1]
            self.live(l)
        return l

    def wert(self, frame, p):
        if len(p) == 1:
            v = self.var(frame, p[0])
        else:
            o = self.live(self.ziel(frame, p[:-1]))
            if p[-1] not in o.fields:
                raise Stuck(f"Feld {p[-1]} fehlt")
            v = o.fields[p[-1]]
        if v is BOT:
            raise Stuck(f"{'.'.join(p)} hat den Wert ⊥")
        return v

    def shared_root(self, frame, p):
        v = self.var(frame, p[0])
        if v[0] == "sh":
            return self.live(v[1])
        return None

    def check_access(self, frame, p, kind):
        o = self.shared_root(frame, p)
        if o is None:
            return
        beta = o.k[1]
        if (kind == "ro" and beta == -1) or (kind == "mut" and beta != 0):
            self.stats["borrow_error"] += 1
            raise BorrowError(f"{'.'.join(p)}: Objekt ist {'schreibend' if beta == -1 else 'lesend'} gesperrt")

    def sperre(self, frame, p, kind):
        self.check_access(frame, p, kind)
        o = self.shared_root(frame, p)
        if o is None:
            return None
        o.k[1] = -1 if kind == "mut" else o.k[1] + 1
        self.stats["lock"] += 1
        return (frame[p[0]][1], kind)

    def freigabe(self, tok):
        if tok is None:
            return
        l, kind = tok
        o = self.live(l)
        if kind == "mut":
            if o.k[1] != -1:
                raise Stuck("Sperrzustand inkonsistent")
            o.k[1] = 0
        else:
            if o.k[1] <= 0:
                raise Stuck("Sperrzustand inkonsistent")
            o.k[1] -= 1

    # ---------------------------------------------------------- Freigabe (5.3)
    def drop(self, v):
        kind = v[0]
        if kind in ("int", "ref", "wk", "bot"):
            return
        l = v[1]
        o = self.heap.get(l)
        if o is DEAD:
            raise Stuck(f"doppelte Freigabe von {l}")
        o = self.live(l)
        if kind == "sh":
            if o.k is None or o.k[0] <= 0:
                raise Stuck(f"Zähler von {l} inkonsistent")
            o.k[0] -= 1
            if o.k[0] > 0:
                return
        elif kind == "own":
            if o.k is not None:
                raise Stuck(f"own-Verweis auf shared-Objekt {l}")
        else:
            raise Stuck(f"unbekannte Wertart {kind}")
        if l in self.dropping:
            raise Stuck(f"doppelte Freigabe von {l} (während der Freigabe)")
        self.dropping.add(l)
        self.stats["free"] += 1
        for c in self.ct.chain(o.cls):                       # V-7: Basisklasse zuletzt
            d = self.prog.classes[c].destructor
            if d is not None:
                self.invoke(d, {"this": ("ref", l)}, None, [])
        for f in reversed(list(o.fields)):
            fv = o.fields[f]
            o.fields[f] = BOT
            self.drop(fv)
        self.dropping.discard(l)
        self.heap[l] = DEAD

    # -------------------------------------------------------------- Ausdrücke
    def eval(self, frame, e):
        from . import ast as A
        if isinstance(e, A.Num):
            return ("int", e.value)
        if isinstance(e, A.BinOp):
            a = self.eval(frame, e.left)
            self.temps.append(a)
            b = self.eval(frame, e.right)
            self.temps.pop()
            if a[0] != "int" or b[0] != "int":
                raise Stuck("Operator auf Nicht-int")
            if e.op == "+":
                return ("int", a[1] + b[1])
            if e.op == "-":
                return ("int", a[1] - b[1])
            return ("int", 1 if a[1] < b[1] else 0)
        if isinstance(e, A.Copy):
            self.check_access(frame, e.place, "ro")
            v = self.wert(frame, e.place)
            if v[0] == "sh":
                self.live(v[1]).k[0] += 1
            elif v[0] not in ("int", "wk"):
                raise Stuck(f"copy eines {v[0]}-Werts")
            return v
        if isinstance(e, A.Move):
            v = self.var(frame, e.var)
            if v is BOT:
                raise Stuck(f"{e.var} ist schon verschoben (⊥)")
            if v[0] not in ("own", "sh"):
                raise Stuck(f"move eines {v[0]}-Werts")
            frame[e.var] = BOT
            return v
        if isinstance(e, A.New):
            vals = []
            for a in e.args:
                v = self.eval(frame, a)
                self.temps.append(v)
                vals.append(v)
            del self.temps[len(self.temps) - len(vals):]
            names = [n for _, n in self.ct.fields(e.cls)]
            if len(names) != len(vals):
                raise Stuck("falsche Feldzahl")
            l = self.next
            self.next += 1
            self.heap[l] = Obj(e.cls, dict(zip(names, vals)))
            self.stats["new"] += 1
            return ("own", l)
        if isinstance(e, A.Share):
            v = self.eval(frame, e.expr)
            if v[0] != "own":
                raise Stuck("share auf Nicht-own")
            o = self.live(v[1])
            if o.k is not None:
                raise Stuck("share auf bereits geteiltes Objekt")
            o.k = [1, 0]
            self.shared_used = True
            return ("sh", v[1])
        if isinstance(e, A.Demote):
            v = self.eval(frame, e.expr)
            if v[0] != "sh":
                raise Stuck("demote auf Nicht-shared")
            self.drop(v)
            return ("wk", v[1])
        if isinstance(e, A.Replace):
            v = self.eval(frame, e.expr)
            self.temps.append(v)
            self.check_access(frame, e.place, "mut")
            o = self.live(self.ziel(frame, e.place[:-1]))
            f = e.place[-1]
            if f not in o.fields:
                raise Stuck(f"Feld {f} fehlt")
            old = o.fields[f]
            o.fields[f] = v
            self.temps.pop()
            return old
        if isinstance(e, A.Call):
            rv, recv = self.call(frame, e)
            self.freigabe(recv)
            return rv
        if isinstance(e, A.BorrowArg):          # return &κ p in einer Borrow-Methode
            return ("ref", self.ziel(frame, e.place))
        raise Stuck(f"unerwarteter Ausdruck {type(e).__name__}")

    def call(self, frame, e):
        from . import ast as A
        self.tick()
        l = self.ziel(frame, e.place)
        m = self.lookup(self.live(l).cls, e.method)
        locks = [self.sperre(frame, e.place, m.mode)]
        if len(m.params) != len(e.args):
            raise Stuck("falsche Argumentzahl")
        vals = []
        for (pt, pn), a in zip(m.params, e.args):
            if isinstance(a, A.BorrowArg):
                locks.append(self.sperre(frame, a.place, a.kind))
                v = ("ref", self.ziel(frame, a.place))
            else:
                v = self.eval(frame, a)
            self.temps.append(v)
            vals.append(v)
        del self.temps[len(self.temps) - len(vals):]
        new = {"this": ("ref", l)}
        for (pt, pn), v in zip(m.params, vals):
            new[pn] = v
        rv = self.invoke(m.body, new, m.ret_expr, [pn for _, pn in m.params])
        for tok in reversed(locks[1:]):
            self.freigabe(tok)
        return rv, locks[0]      # die Sperre des Empfängers gibt der Aufrufer frei

    def invoke(self, body, frame, ret_expr, params):
        self.frames.append(frame)
        declared = list(params)
        for s in body:
            self.stmt(frame, s, declared)
        rv = self.eval(frame, ret_expr) if ret_expr is not None else ("int", 0)
        self.temps.append(rv)
        for x in reversed(declared):
            self.drop(frame.pop(x))
        self.temps.pop()
        self.frames.pop()
        return rv

    # ------------------------------------------------------------ Anweisungen
    def block(self, frame, stmts, pre=()):
        declared = list(pre)
        for s in stmts:
            self.stmt(frame, s, declared)
        for x in reversed(declared):
            self.drop(frame.pop(x))

    def stmt(self, frame, s, declared):
        from . import ast as A
        self.tick()
        if isinstance(s, A.Decl):
            v = self.eval(frame, s.expr)
            if s.var in frame:
                raise Stuck(f"{s.var} doppelt im Rahmen")
            frame[s.var] = v
            declared.append(s.var)
        elif isinstance(s, A.Unpack):                 # R-5.25: ohne Destruktor
            v = self.var(frame, s.var)
            if v is BOT or v[0] != "own":
                raise Stuck(f"zerlegen von {s.var} ohne own-Wert")
            o = self.live(v[1])
            chain, c = [], o.cls
            while c is not None:
                chain.append(c)
                c = self.prog.classes[c].base
            names = [f for k in reversed(chain) for _, f in self.prog.classes[k].fields]
            vals = [o.fields[f] for f in names]
            self.heap[v[1]] = DEAD
            frame[s.var] = BOT
            for (_, x), val in zip(s.vars, vals):
                if x in frame:
                    raise Stuck(f"{x} doppelt im Rahmen")
                frame[x] = val
                declared.append(x)
        elif isinstance(s, A.AssignVar):
            v = self.eval(frame, s.expr)
            old = self.var(frame, s.var)
            if old[0] == "ref":
                raise Stuck("Zuweisung an Borrow-Variable")
            frame[s.var] = v
            self.drop(old)
        elif isinstance(s, A.AssignField):
            old = self.eval(frame, A.Replace(s.place, s.expr, s.line))
            self.drop(old)
        elif isinstance(s, A.ExprStmt):
            v = self.eval(frame, s.expr)
            self.drop(v)
        elif isinstance(s, A.Print):
            v = self.eval(frame, s.expr)
            if v[0] != "int":
                raise Stuck("print auf Nicht-int")
            self.out.append(v[1])
        elif isinstance(s, A.If):
            if self.cond(frame, s.cond):
                self.block(frame, s.then)
            else:
                self.block(frame, s.els)
        elif isinstance(s, A.While):
            while self.cond(frame, s.cond):
                self.tick()
                self.block(frame, s.body)
        elif isinstance(s, A.Borrow):
            tok = self.sperre(frame, s.place, s.kind)
            frame[s.var] = ("ref", self.ziel(frame, s.place))
            self.block(frame, s.body)
            del frame[s.var]
            self.freigabe(tok)
        elif isinstance(s, A.BorrowCall):
            rv, recv = self.call(frame, s.call)
            self.stats["borrow_call"] += 1
            if rv[0] != "ref":
                raise Stuck("borrow-Aufruf liefert keinen Borrow")
            self.live(rv[1])
            frame[s.var] = rv
            self.block(frame, s.body)
            del frame[s.var]
            self.freigabe(recv)
        elif isinstance(s, A.Upgrade):
            self.check_access(frame, s.place, "ro")
            v = self.wert(frame, s.place)
            if v[0] != "wk":
                raise Stuck("upgrade auf Nicht-weak")
            o = self.heap.get(v[1])
            if o is not None and o is not DEAD and o.k[0] > 0:
                o.k[0] += 1
                frame[s.var] = ("sh", v[1])
                self.stats["upgrade_ok"] += 1
                self.block(frame, s.then, pre=[s.var])
            else:
                self.stats["upgrade_fail"] += 1
                self.block(frame, s.els)
        else:
            raise Stuck(f"unerwartete Anweisung {type(s).__name__}")
        if self.inv:
            self.check_invariants()

    def cond(self, frame, p):
        self.check_access(frame, p, "ro")
        v = self.wert(frame, p)
        if v[0] != "int":
            raise Stuck("Bedingung ist kein int")
        return v[1] != 0

    # ---------------------------------------------------- Invarianten (6.2)
    def check_invariants(self):
        own, sh = Counter(), Counter()

        def visit(v, where):
            if v is BOT or v[0] in ("int", "wk"):
                return
            l = v[1]
            if self.heap.get(l) is DEAD:
                rule = "I-4" if v[0] == "ref" else "I-2"
                raise InvariantViolation(f"{rule}: {where} verweist auf freigegebenes Objekt {l}")
            if v[0] == "own":
                own[l] += 1
            elif v[0] == "sh":
                sh[l] += 1

        for i, fr in enumerate(self.frames):
            for x, v in fr.items():
                visit(v, f"Variable {x} in Rahmen {i}")
        for v in self.temps:
            visit(v, "Zwischenwert")
        for l, o in self.heap.items():
            if o is not DEAD:
                for f, v in o.fields.items():
                    visit(v, f"Feld {f} von Objekt {l}")
        for l, o in self.heap.items():
            if o is DEAD or l in self.dropping:
                continue
            if o.k is None:
                if own[l] != 1 or sh[l] != 0:
                    raise InvariantViolation(f"I-1: Objekt {l} hat {own[l]} own- und {sh[l]} shared-Verweise")
            else:
                if own[l] != 0 or sh[l] != o.k[0]:
                    raise InvariantViolation(f"I-1: shared-Objekt {l}: rc={o.k[0]}, "
                                             f"{sh[l]} shared- und {own[l]} own-Verweise")
                if o.k[1] != 0 and o.k[0] < 1:
                    raise InvariantViolation(f"I-5: gesperrtes Objekt {l} mit rc={o.k[0]}")

    # ------------------------------------------------------------------ Start
    def run(self):
        frame = {}
        self.frames.append(frame)
        self.block(frame, self.prog.main)
        self.frames.pop()
        return self.out

    def live_objects(self):
        return [l for l, o in self.heap.items() if o is not DEAD]
