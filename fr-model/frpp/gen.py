"""Zufallsgenerator für FR++-Programme.

Die Klassentabelle ist fest (siehe TEMPLATE); die Rümpfe der mit GEN
markierten Methoden, der Destruktor von N und das Hauptprogramm werden
zufällig erzeugt.

Der Generator führt grob Buch, welche Variablen verschoben, geliehen oder nur
lesbar sind, damit genug Programme die Typprüfung bestehen. Mit der
Wahrscheinlichkeit `frech` ignoriert er diese Buchführung absichtlich. So
entstehen auch Programme, die der Typprüfer ablehnen muss. Entscheidend ist
allein der Typprüfer; der Generator muss nicht korrekt sein.

Umgebungseinträge sind Listen [Name, Typ, schreibbar, verschiebbar].
"""

import random

from . import ast as A
from .checker import Checker
from .parser import parse

TEMPLATE = """
class A {
    int v;
    int get() ro { return copy this.v; }
    int inc() mut { this.v = copy this.v + 1; return 0; }
    destructor() { print(copy this.v); }
}
class A2 : A {
    own A inner;
    int get() ro { return copy this.inner.v; }
}
class N {
    int v;
    own A a;
    shared A s;
    weak A w;
    int f1(mut A x, ref N y) mut { return 0; }
    own A f2(own A x, shared A y) ro { return new A(0); }
    int f3(ref A x, mut N y) ro { return 0; }
    shared A f4() mut { return share new A(0); }
    int f5(mut A x) mut { this.a = new A(9); return x.get(); }
    ref A first() ro { return &ro this.a; }
    mut A firstMut() mut { return &mut this.a; }
    int f6(ref A x) mut { this.a = new A(8); return x.get(); }
    int touch() ro { shared A t = copy this.s; t.inc(); return 0; }
    int look() ro { shared A t = copy this.s; return t.get(); }
    destructor() { }
}
class S {
    shared N n;
    weak N back;
    int g(mut N x) ro { return 0; }
    int h() mut { return 0; }
    int g2(mut N x) ro { return this.poke(); }
    int g3(ref N x) ro { return this.peek(); }
    int poke() ro { shared N t = copy this.n; t.v = copy t.v + 1; return 0; }
    int peek() ro { shared N t = copy this.n; return copy t.v; }
    int drop() mut { this.n = share new N(0, new A(0), share new A(0), demote share new A(0)); return 0; }
}
main { }
"""

GENERATED = [("N", "f1"), ("N", "f2"), ("N", "f3"), ("N", "f4"), ("S", "g"), ("S", "h")]

DECL_TYPES = [A.INT, A.INT, A.Type("own", "A"), A.Type("own", "A2"), A.Type("own", "N"),
              A.Type("own", "S"), A.Type("shared", "A"), A.Type("shared", "N"),
              A.Type("shared", "S"), A.Type("weak", "A"), A.Type("weak", "N")]


class Retry(Exception):
    """Der Generator findet keinen passenden Aufruf und weicht aus."""


def entry(x, t):
    w = t.kind != "ref"
    m = t.kind in ("own", "shared")
    return [x, t, w, m]


class Gen:
    def __init__(self, seed, size=1.0, frech=0.1):
        self.r = random.Random(seed)
        self.prog = parse(TEMPLATE)
        self.ct = Checker(self.prog)
        self.n = 0
        self.size = size
        self.frech = frech

    # ------------------------------------------------------------ Hilfen
    def fresh(self):
        self.n += 1
        return f"v{self.n}"

    def chance(self, p):
        return self.r.random() < p

    def rude(self):
        return self.chance(self.frech)

    def subclasses(self, c):
        return [k for k in self.prog.classes if self.ct.subclass(k, c)]

    def places(self, env, depth=2):
        """[(Ort, Typ, schreibbar)]"""
        out = []
        for x, t, w, _ in env:
            out.append(((x,), t, w))
            self._extend((x,), t, w, True, depth, out)
        return out

    def _extend(self, p, t, w, is_var, depth, out):
        if depth == 0:
            return
        c = Checker.obj_class(t, is_var)
        if c is None:
            return
        for ft, f in self.ct.fields(c):
            q = p + (f,)
            out.append((q, ft, w))
            self._extend(q, ft, w, False, depth - 1, out)

    def obj_places(self, env, cls=None, need_write=False):
        res = []
        for p, t, w in self.places(env):
            c = Checker.obj_class(t, len(p) == 1)
            if c is None or (cls is not None and not self.ct.subclass(c, cls)):
                continue
            if need_write and not w and not self.rude():
                continue
            res.append((p, c))
        return res

    def methods(self, cls):
        out = []
        for k in self.ct.chain(cls):
            for m in self.prog.classes[k].methods.values():
                if all(m.name != o.name for o in out):
                    out.append(m)
        return out

    @staticmethod
    def restricted(env, root, how):
        """Kopie der Umgebung, in der `root` nur lesbar (ro) oder ganz unsichtbar (mut) ist."""
        out = []
        for e in env:
            if e[0] == root:
                if how == "mut":
                    continue
                e = [e[0], e[1], False, False]
            else:
                e = list(e)
            out.append(e)
        return out

    def take_move(self, env, x):
        for e in env:
            if e[0] == x and not self.rude():
                e[3] = False
                env.remove(e)
                return

    # ---------------------------------------------------------- Ausdrücke
    def expr(self, env, t, depth):
        r = self.r
        opts = []
        pl = self.places(env)
        if t.kind == "int":
            opts.append((3, lambda: A.Num(r.randint(0, 3))))
            ints = [p for p, pt, _ in pl if pt == A.INT]
            if ints:
                opts.append((3, lambda: A.Copy(r.choice(ints))))
            if depth > 0:
                opts.append((1, lambda: A.BinOp(r.choice("+-<"), self.expr(env, A.INT, depth - 1),
                                                self.expr(env, A.INT, depth - 1))))
        elif t.kind == "own":
            opts.append((3, lambda: self.new(env, r.choice(self.subclasses(t.cls)), depth - 1)))
            movable = [x for x, xt, _, m in env
                       if xt.kind == "own" and self.ct.subclass(xt.cls, t.cls) and (m or self.rude())]
            if movable:
                def mk_move():
                    x = r.choice(movable)
                    self.take_move(env, x)
                    return A.Move(x)
                opts.append((3, mk_move))
            fields = [(p, pt) for p, pt, w in pl
                      if len(p) > 1 and pt.kind == "own" and self.ct.subclass(pt.cls, t.cls)
                      and (w or self.rude())]
            if fields and depth > 0:
                def mk_replace():
                    p, pt = r.choice(fields)
                    return A.Replace(p, self.expr(env, pt, depth - 1))
                opts.append((1, mk_replace))
        elif t.kind == "shared":
            opts.append((2, lambda: A.Share(self.expr(env, A.Type("own", t.cls), depth - 1))))
            sh = [p for p, pt, _ in pl if pt.kind == "shared" and self.ct.subclass(pt.cls, t.cls)]
            if sh:
                opts.append((5, lambda: A.Copy(r.choice(sh))))
            mv = [x for x, xt, _, m in env
                  if xt.kind == "shared" and self.ct.subclass(xt.cls, t.cls) and (m or self.rude())]
            if mv:
                def mk_move_sh():
                    x = r.choice(mv)
                    self.take_move(env, x)
                    return A.Move(x)
                opts.append((1, mk_move_sh))
        elif t.kind == "weak":
            opts.append((2, lambda: A.Demote(self.expr(env, A.Type("shared", t.cls), depth - 1))))
            wk = [p for p, pt, _ in pl if pt.kind == "weak" and self.ct.subclass(pt.cls, t.cls)]
            if wk:
                opts.append((5, lambda: A.Copy(r.choice(wk))))
        if depth > 0:
            calls = self.call_candidates(env, t)
            if calls:
                def mk_call():
                    try:
                        return self.call(env, r.choice(calls), depth - 1)
                    except Retry:
                        return self.simple(t)
                opts.append((2, mk_call))
        total = sum(w for w, _ in opts)
        x = r.uniform(0, total)
        for w, f in opts:
            x -= w
            if x <= 0:
                return f()
        return opts[-1][1]()

    def new(self, env, cls, depth):
        return A.New(cls, [self.expr(env, ft, depth) if depth > 0 else self.simple(ft)
                           for ft, _ in self.ct.fields(cls)])

    def simple(self, t):
        if t.kind == "int":
            return A.Num(self.r.randint(0, 3))
        if t.kind == "own":
            return A.New(t.cls, [self.simple(ft) for ft, _ in self.ct.fields(t.cls)])
        if t.kind == "shared":
            return A.Share(self.simple(A.Type("own", t.cls)))
        return A.Demote(self.simple(A.Type("shared", t.cls)))

    def call_candidates(self, env, want):
        out = []
        for p, c in self.obj_places(env):
            w = any(pp == p and ww for pp, _, ww in self.places(env))
            for m in self.methods(c):
                if m.ret.kind in A.BORROW:          # Borrow-Methoden nur über borrow x = …
                    continue
                if want is not None and not self.ct.subtype(m.ret, want):
                    continue
                if m.mode == "mut" and not w and not self.rude():
                    continue
                out.append((p, m))
        return out

    def call(self, env, cand, depth):
        p, m = cand
        busy = {p[0]: m.mode}
        args = []
        for pt, _ in m.params:
            if pt.kind in A.BORROW:
                kind = "ro" if pt.kind == "ref" else "mut"
                if self.chance(0.03):
                    kind = "mut" if kind == "ro" else "ro"
                cands = [(q, c) for q, c in self.obj_places(env, pt.cls, need_write=kind == "mut")
                         if self.rude() or q[0] not in busy or (kind == "ro" and busy[q[0]] == "ro")]
                if not cands:
                    raise Retry()
                q = self.r.choice(cands)[0]
                busy[q[0]] = "mut" if kind == "mut" or busy.get(q[0]) == "mut" else "ro"
                args.append(A.BorrowArg(kind, q))
            else:
                sub = env
                for root, k in busy.items():
                    if not self.rude():
                        sub = self.restricted(sub, root, k)
                args.append(self.expr(sub, pt, depth))
                moved = {e[0] for e in env} - {e[0] for e in sub} - set(busy)
                for e in list(env):
                    if e[0] in moved:
                        env.remove(e)
        return A.Call(p, m.name, args)

    # -------------------------------------------------------- Anweisungen
    def block(self, env, depth, count, nomove=False):
        env = [list(e) for e in env]
        if nomove:
            for e in env:
                e[3] = False
        out = []
        for _ in range(count):
            out += self.stmt(env, depth)
        return out

    def stmt(self, env, depth):
        r = self.r
        kinds = ["decl"] * 6 + ["assign"] * 2 + ["field"] * 2 + ["call"] * 5 + ["print"]
        if depth > 0:
            kinds += ["if"] * 2 + ["while"] + ["borrow"] * 3 + ["upgrade"] * 2 + ["bcall"] * 3
        k = r.choice(kinds)
        ed = 2
        if k == "decl":
            t = r.choice(DECL_TYPES)
            x = self.fresh()
            s = A.Decl(t, x, self.expr(env, t, ed))
            env.append(entry(x, t))
            return [s]
        if k == "assign":
            vs = [(x, t) for x, t, w, _ in env if t.kind in A.STORABLE and (w or self.rude())]
            if vs:
                x, t = r.choice(vs)
                e = self.expr(env, t, ed)
                for en in env:
                    if en[0] == x:
                        en[3] = t.kind in ("own", "shared")
                if all(en[0] != x for en in env):
                    env.append(entry(x, t))
                return [A.AssignVar(x, e)]
        if k == "field":
            fs = [(p, t) for p, t, w in self.places(env) if len(p) > 1 and (w or self.rude())]
            if fs:
                p, t = r.choice(fs)
                return [A.AssignField(p, self.expr(env, t, ed))]
        if k == "call":
            cands = self.call_candidates(env, None)
            if cands:
                try:
                    return [A.ExprStmt(self.call(env, r.choice(cands), ed))]
                except Retry:
                    pass
        if k == "print":
            return [A.Print(self.expr(env, A.INT, ed))]
        if k == "if":
            c = self.fresh()
            env.append(entry(c, A.INT))
            nomove = self.chance(0.5)
            return [A.Decl(A.INT, c, A.Num(r.randint(0, 1))),
                    A.If((c,), self.block(env, depth - 1, r.randint(0, 3), nomove),
                         self.block(env, depth - 1, r.randint(0, 2), nomove))]
        if k == "while":
            c = self.fresh()
            env.append(entry(c, A.INT))
            body = self.block(env, depth - 1, r.randint(0, 3), nomove=not self.rude())
            body.append(A.AssignVar(c, A.BinOp("-", A.Copy((c,)), A.Num(1))))
            return [A.Decl(A.INT, c, A.Num(r.randint(0, 3))), A.While((c,), body)]
        if k == "borrow":
            kind = r.choice(["ro", "mut"])
            cands = self.obj_places(env, need_write=kind == "mut")
            if cands:
                p, c = r.choice(cands)
                x = self.fresh()
                inner = env if self.rude() else self.restricted(env, p[0], kind)
                inner = inner + [entry(x, A.brw(kind, c))]
                return [A.Borrow(x, kind, p, self.block(inner, depth - 1, r.randint(1, 3)))]
        if k == "bcall":
            cands = []
            for p, c in self.obj_places(env):
                w = any(pp == p and ww for pp, _, ww in self.places(env))
                for m in self.methods(c):
                    if m.ret.kind in A.BORROW and (m.mode == "ro" or w or self.rude()):
                        cands.append((p, m))
            if cands:
                p, m = r.choice(cands)
                x = self.fresh()
                try:
                    call = self.call(env, (p, m), ed)
                except Retry:
                    call = None
                if call is not None:
                    inner = env if self.rude() else self.restricted(env, p[0], m.mode)
                    inner = inner + [entry(x, m.ret)]
                    return [A.BorrowCall(x, call, self.block(inner, depth - 1, r.randint(1, 3)))]
        if k == "upgrade":
            wk = [(p, t) for p, t, _ in self.places(env) if t.kind == "weak"]
            if wk:
                p, t = r.choice(wk)
                x = self.fresh()
                inner = env + [entry(x, A.Type("shared", t.cls))]
                return [A.Upgrade(x, p, self.block(inner, depth - 1, r.randint(1, 3)),
                                  self.block(env, depth - 1, r.randint(0, 1)))]
        return [A.Print(self.expr(env, A.INT, 1))]

    # ------------------------------------------------------------ Programm
    def program(self):
        for cname, mname in GENERATED:
            m = self.prog.classes[cname].methods[mname]
            env = [entry("this", A.brw(m.mode, cname))] + [entry(x, t) for t, x in m.params]
            body_env = [list(e) for e in env]
            m.body = []
            for _ in range(self.r.randint(0, int(3 * self.size))):
                m.body += self.stmt(body_env, 1)
            m.ret_expr = self.expr(body_env, m.ret, 1)
        n = self.prog.classes["N"]
        n.destructor = self.block([entry("this", A.Type("mut", "N"))], 1, self.r.randint(0, 2))
        # Fester Anfang: mehrere Verweise auf dieselben shared-Objekte, damit die
        # Laufzeitprüfung (V-1) und upgrade oft genug vorkommen.
        prelude = parse("""main {
            shared N sn = share new N(1, new A(1), share new A(2), demote share new A(3));
            own S so = new S(copy sn, demote copy sn);
            shared A sa = copy sn.s;
            weak A wa = demote copy sa;
        }""").main if self.chance(0.7) else []
        main_env = [entry(s.var, s.type) for s in prelude]
        self.prog.main = list(prelude)
        for _ in range(self.r.randint(3, int(10 * self.size))):
            self.prog.main += self.stmt(main_env, 2)
        return self.prog


def generate(seed, size=1.0, frech=0.1):
    return Gen(seed, size, frech).program()
