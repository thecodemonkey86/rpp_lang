/* Laufzeit für den C-Codegenerator von rpp-compiler (Meilenstein M6), siehe rpp_rt.h.
 * Jede Funktion folgt der gleichnamigen Stelle im Debug-Interpreter von rppc
 * (rppc/crates/rpp-interp/src/lib.rs und stdlib.rs).
 */
#define _CRT_SECURE_NO_WARNINGS
#include "rpp_rt.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include <math.h>
#include <float.h>
#include <errno.h>

#ifdef _WIN32
#include <windows.h>
#include <timeapi.h>
#pragma comment(lib, "winmm.lib")
#pragma comment(lib, "shell32.lib")
#include <shellapi.h>
#include <io.h>
#include <fcntl.h>
#include <direct.h>
#include <sys/stat.h>
#else
#include <pthread.h>
#include <fcntl.h>
#include <dirent.h>
#include <sys/stat.h>
#include <unistd.h>
#endif

/* ================================================================ Zustand */

int rt_release = 0;
rv rt_exc;
RTrace rt_exc_trace;
int rt_fatals = 0;
static int rt_nofree = 0;

static const char* fatal_name = NULL;
static char* fatal_msg = NULL;
static RTrace fatal_trace;
static int fatal_pending = 0;

const char* rt_frames[RT_MAX_DEPTH + 16];
int rt_fdepth = 0;

/* ---------------------------------------------------------------- Speicher (M7)
 * Kleine Blöcke (bis 1024 Byte) kommen aus Freilisten je Größenklasse, die aus großen Stücken
 * gespeist werden; größere direkt von malloc. Vor jedem Block steht ein Kopf mit der Klasse
 * (16 Byte, damit die Ausrichtung erhalten bleibt). Der Speicher kleiner Blöcke geht nicht an
 * das Betriebssystem zurück. Die Laufzeit führt R++-Code in einem einzigen Thread aus. */
#define MEM_HDR 16
#define MEM_SMALL 1024
#define MEM_CLASSES 64              /* Klassen in Schritten von 16 Byte */
#define MEM_LARGE 0xFFFF
#define MEM_CHUNK ((size_t)1 << 20)

typedef struct MemFree { struct MemFree* next; } MemFree;
static MemFree* mem_free_list[MEM_CLASSES + 1];
static char* mem_chunk_cur = NULL;
static size_t mem_chunk_left = 0;

static void mem_oom(void) {
    fputs("interner Fehler: kein Speicher\n", stderr);
    exit(3);
}

static void* mem_carve(size_t total) {
    if (mem_chunk_left < total) {
        mem_chunk_cur = (char*)malloc(MEM_CHUNK);
        if (!mem_chunk_cur) mem_oom();
        mem_chunk_left = MEM_CHUNK;
    }
    void* p = mem_chunk_cur;
    mem_chunk_cur += total;
    mem_chunk_left -= total;
    return p;
}

static void* xmalloc(size_t n) {
    if (n == 0) n = 1;
    if (n <= MEM_SMALL) {
        size_t k = (n + 15) >> 4;   /* 1..64 */
        MemFree* f = mem_free_list[k];
        char* h;
        if (f) {
            mem_free_list[k] = f->next;
            h = (char*)f;
        } else {
            h = (char*)mem_carve(MEM_HDR + (k << 4));
        }
        *(uint32_t*)h = (uint32_t)k;
        return h + MEM_HDR;
    }
    char* h = (char*)malloc(MEM_HDR + n);
    if (!h) mem_oom();
    *(uint32_t*)h = MEM_LARGE;
    return h + MEM_HDR;
}

static void xfree(void* p) {
    if (!p) return;
    char* h = (char*)p - MEM_HDR;
    uint32_t k = *(uint32_t*)h;
    if (k == MEM_LARGE) {
        free(h);
        return;
    }
    MemFree* f = (MemFree*)h;
    f->next = mem_free_list[k];
    mem_free_list[k] = f;
}

static void* xrealloc(void* p, size_t n) {
    if (!p) return xmalloc(n);
    if (n == 0) n = 1;
    char* h = (char*)p - MEM_HDR;
    uint32_t k = *(uint32_t*)h;
    if (k == MEM_LARGE) {
        if (n > MEM_SMALL) {
            char* q = (char*)realloc(h, MEM_HDR + n);
            if (!q) mem_oom();
            return q + MEM_HDR;
        }
        void* q = xmalloc(n);
        memcpy(q, p, n);   /* der alte Block ist größer als MEM_SMALL >= n */
        free(h);
        return q;
    }
    size_t have = (size_t)k << 4;
    if (n <= have) return p;
    void* q = xmalloc(n);
    memcpy(q, p, have);
    xfree(p);
    return q;
}

/* ---------------------------------------------------------------- Text-Puffer */

typedef struct Buf {
    char* p;
    int64_t n, cap;
} Buf;

static void buf_add(Buf* b, const char* s, int64_t n) {
    if (b->n + n + 1 > b->cap) {
        int64_t c = b->cap ? b->cap * 2 : 64;
        while (c < b->n + n + 1) c *= 2;
        b->p = (char*)xrealloc(b->p, (size_t)c);
        b->cap = c;
    }
    memcpy(b->p + b->n, s, (size_t)n);
    b->n += n;
    b->p[b->n] = 0;
}

static void buf_str(Buf* b, const char* s) { buf_add(b, s, (int64_t)strlen(s)); }

static void buf_fmt(Buf* b, const char* fmt, ...) {
    char tmp[512];
    va_list ap;
    va_start(ap, fmt);
    int n = vsnprintf(tmp, sizeof tmp, fmt, ap);
    va_end(ap);
    if (n < 0) return;
    if (n < (int)sizeof tmp) {
        buf_add(b, tmp, n);
        return;
    }
    char* big = (char*)xmalloc((size_t)n + 1);
    va_start(ap, fmt);
    vsnprintf(big, (size_t)n + 1, fmt, ap);
    va_end(ap);
    buf_add(b, big, n);
    xfree(big);
}

/* ================================================================ Zahlen */

int rt_is_int_tag(uint8_t t) { return t >= RT_I8 && t <= RT_U64; }
int rt_is_num_tag(uint8_t t) { return t >= RT_I8 && t <= RT_F64; }
static int is_unsigned(uint8_t t) { return t >= RT_U8 && t <= RT_U64; }
static int is_float_tag(uint8_t t) { return t == RT_F32 || t == RT_F64; }


/* Ganzzahl mit 128 Bit (für exakte Ergebnisse wie i128 im Interpreter) */
typedef struct W {
    uint64_t lo;
    int64_t hi;
} W;

static W w_from_i(int64_t v) { W w; w.lo = (uint64_t)v; w.hi = v < 0 ? -1 : 0; return w; }
static W w_from_u(uint64_t v) { W w; w.lo = v; w.hi = 0; return w; }
static int w_neg_p(W a) { return a.hi < 0; }
static W w_add(W a, W b) { W r; r.lo = a.lo + b.lo; r.hi = (int64_t)((uint64_t)a.hi + (uint64_t)b.hi + (r.lo < a.lo ? 1u : 0u)); return r; }
static W w_negate(W a) { W r; r.lo = ~a.lo + 1; r.hi = (int64_t)(~(uint64_t)a.hi + (r.lo == 0 ? 1u : 0u)); return r; }
static W w_sub(W a, W b) { return w_add(a, w_negate(b)); }

static void mul64(uint64_t a, uint64_t b, uint64_t* hi, uint64_t* lo) {
    uint64_t a0 = a & 0xffffffffu, a1 = a >> 32, b0 = b & 0xffffffffu, b1 = b >> 32;
    uint64_t p00 = a0 * b0, p01 = a0 * b1, p10 = a1 * b0, p11 = a1 * b1;
    uint64_t mid = (p00 >> 32) + (p01 & 0xffffffffu) + (p10 & 0xffffffffu);
    *lo = (p00 & 0xffffffffu) | (mid << 32);
    *hi = p11 + (p01 >> 32) + (p10 >> 32) + (mid >> 32);
}

static W w_mul(W a, W b) {
    int na = w_neg_p(a), nb = w_neg_p(b);
    W ua = na ? w_negate(a) : a, ub = nb ? w_negate(b) : b;
    uint64_t hi, lo;
    mul64(ua.lo, ub.lo, &hi, &lo);
    hi += ua.lo * (uint64_t)ub.hi + (uint64_t)ua.hi * ub.lo;
    W r; r.lo = lo; r.hi = (int64_t)hi;
    return (na != nb) ? w_negate(r) : r;
}

/* Division und Rest mit Abschneiden gegen null; Operanden passen in 65 Bit */
static void w_divrem(W a, W b, W* q, W* r) {
    int na = w_neg_p(a), nb = w_neg_p(b);
    W ua = na ? w_negate(a) : a, ub = nb ? w_negate(b) : b;
    uint64_t qa = ua.lo / ub.lo, ra = ua.lo % ub.lo;
    W wq = w_from_u(qa), wr = w_from_u(ra);
    *q = (na != nb) ? w_negate(wq) : wq;
    *r = na ? w_negate(wr) : wr;
}

static int w_cmp(W a, W b) {
    if (a.hi != b.hi) return a.hi < b.hi ? -1 : 1;
    if (a.lo != b.lo) return a.lo < b.lo ? -1 : 1;
    return 0;
}

static W w_of(rv v) {
    switch (v.tag) {
    case RT_U8: case RT_U16: case RT_U32: case RT_U64: case RT_BYTE: return w_from_u(v.v.u);
    case RT_CHAR: return w_from_u(v.v.u);
    default: return w_from_i(v.v.i);
    }
}

static int w_fits(W v, uint8_t t) {
    switch (t) {
    case RT_I8: return w_cmp(v, w_from_i(-128)) >= 0 && w_cmp(v, w_from_i(127)) <= 0;
    case RT_I16: return w_cmp(v, w_from_i(-32768)) >= 0 && w_cmp(v, w_from_i(32767)) <= 0;
    case RT_I32: return w_cmp(v, w_from_i(INT32_MIN)) >= 0 && w_cmp(v, w_from_i(INT32_MAX)) <= 0;
    case RT_I64: return (v.hi == 0 && v.lo <= (uint64_t)INT64_MAX) || (v.hi == -1 && v.lo >= (uint64_t)INT64_MIN);
    case RT_U8: return v.hi == 0 && v.lo <= 255;
    case RT_U16: return v.hi == 0 && v.lo <= 65535;
    case RT_U32: return v.hi == 0 && v.lo <= 0xffffffffu;
    case RT_U64: return v.hi == 0;
    case RT_F32: case RT_F64: return 1;
    default: return 0;
    }
}

static void w_text(W v, char* out) {
    char tmp[64];
    int n = 0, neg = w_neg_p(v);
    W u = neg ? w_negate(v) : v;
    uint64_t hi = (uint64_t)u.hi, lo = u.lo;
    if (hi == 0 && lo == 0) tmp[n++] = '0';
    while (hi != 0 || lo != 0) {
        /* (hi:lo) / 10 */
        uint64_t rh = hi % 10, qh = hi / 10;
        uint64_t q = 0, r = rh;
        for (int i = 63; i >= 0; i--) {
            r = (r << 1) | ((lo >> i) & 1);
            q <<= 1;
            if (r >= 10) { r -= 10; q |= 1; }
        }
        tmp[n++] = (char)('0' + r);
        hi = qh;
        lo = q;
    }
    int k = 0;
    if (neg) out[k++] = '-';
    while (n > 0) out[k++] = tmp[--n];
    out[k] = 0;
}

static double w_to_f64(W v) {
    if (w_neg_p(v)) return -w_to_f64(w_negate(v));
    return (double)(uint64_t)v.hi * 18446744073709551616.0 + (double)v.lo;
}

static rv make_int_w(W v, uint8_t t) {
    rv r = rv_make(t);
    switch (t) {
    case RT_I8: r.v.i = (int8_t)v.lo; break;
    case RT_I16: r.v.i = (int16_t)v.lo; break;
    case RT_I32: r.v.i = (int32_t)v.lo; break;
    case RT_I64: r.v.i = (int64_t)v.lo; break;
    case RT_U8: case RT_BYTE: r.v.u = (uint8_t)v.lo; break;
    case RT_U16: r.v.u = (uint16_t)v.lo; break;
    case RT_U32: r.v.u = (uint32_t)v.lo; break;
    case RT_U64: r.v.u = v.lo; break;
    case RT_F32: r.v.i = 0; r.v.f32 = (float)w_to_f64(v); break;
    case RT_F64: r.v.f = w_to_f64(v); break;
    case RT_CHAR: {
        uint32_t c = (uint32_t)v.lo;
        r.v.u = (c > 0x10ffff || (c >= 0xd800 && c <= 0xdfff)) ? 0 : c;
        break;
    }
    default: r.tag = RT_I32; r.v.i = (int32_t)v.lo; break;
    }
    return r;
}

rv rv_int_slow(int64_t v, uint8_t tag) { return make_int_w(w_from_i(v), tag); }
rv rv_uint_slow(uint64_t v, uint8_t tag) { return make_int_w(w_from_u(v), tag); }

static const char* prim_name(uint8_t t) {
    switch (t) {
    case RT_BOOL: return "bool";
    case RT_CHAR: return "char";
    case RT_BYTE: return "byte";
    case RT_STR: return "string";
    case RT_I8: return "i8";
    case RT_I16: return "i16";
    case RT_I32: return "i32";
    case RT_I64: return "i64";
    case RT_U8: return "ui8";
    case RT_U16: return "ui16";
    case RT_U32: return "ui32";
    case RT_U64: return "ui64";
    case RT_F32: return "f32";
    case RT_F64: return "f64";
    default: return "?";
    }
}

/* R-4.6: automatische Umwandlungen zwischen Zahltypen */
static int auto_converts(uint8_t from, uint8_t to) {
    switch (from) {
    case RT_I8: return to == RT_I16 || to == RT_I32 || to == RT_I64 || to == RT_F32 || to == RT_F64;
    case RT_I16: return to == RT_I32 || to == RT_I64 || to == RT_F32 || to == RT_F64;
    case RT_I32: return to == RT_I64 || to == RT_F64;
    case RT_U8: return to == RT_U16 || to == RT_U32 || to == RT_U64 || to == RT_I16 || to == RT_I32 || to == RT_I64 || to == RT_F32 || to == RT_F64;
    case RT_U16: return to == RT_U32 || to == RT_U64 || to == RT_I32 || to == RT_I64 || to == RT_F32 || to == RT_F64;
    case RT_U32: return to == RT_U64 || to == RT_I64 || to == RT_F64;
    case RT_F32: return to == RT_F64;
    default: return 0;
    }
}

static int conv_or_same(uint8_t a, uint8_t b) { return a == b || auto_converts(a, b); }

static const uint8_t NUMERIC[10] = { RT_I8, RT_I16, RT_I32, RT_I64, RT_U8, RT_U16, RT_U32, RT_U64, RT_F32, RT_F64 };

/* R-4.12: gemeinsamer Zahltyp; 0 wenn keiner */
static uint8_t common_numeric_calc(uint8_t a, uint8_t b) {
    int both_int = rt_is_int_tag(a) && rt_is_int_tag(b);
    uint8_t cands[10];
    int n = 0;
    for (int i = 0; i < 10; i++) {
        uint8_t t = NUMERIC[i];
        if (conv_or_same(a, t) && conv_or_same(b, t) && (!both_int || rt_is_int_tag(t))) cands[n++] = t;
    }
    for (int i = 0; i < n; i++) {
        int all = 1;
        for (int j = 0; j < n; j++) if (!conv_or_same(cands[i], cands[j])) all = 0;
        if (all) return cands[i];
    }
    return 0;
}

static uint8_t cn_table[32][32];
static int cn_ready = 0;

static uint8_t common_numeric(uint8_t a, uint8_t b) {
    if (!cn_ready) {
        for (int x = 0; x < 32; x++)
            for (int y = 0; y < 32; y++)
                cn_table[x][y] = (x >= RT_I8 && x <= RT_F64 && y >= RT_I8 && y <= RT_F64) ? common_numeric_calc((uint8_t)x, (uint8_t)y) : 0;
        cn_ready = 1;
    }
    return cn_table[a & 31][b & 31];
}

static double as_f64(rv v) {
    switch (v.tag) {
    case RT_F32: return (double)v.v.f32;
    case RT_F64: return v.v.f;
    default: return w_to_f64(w_of(v));
    }
}

static int has_f64(rv v) { return rt_is_num_tag(v.tag); }

rv rt_num_to(rv v, uint8_t p) {
    if (p == RT_F32) {
        if (v.tag == RT_F32) return v;
        return rv_f32((float)as_f64(v));
    }
    if (p == RT_F64) return rv_f64(as_f64(v));
    if (!rt_is_int_tag(v.tag) && v.tag != RT_BYTE && v.tag != RT_CHAR) return make_int_w(w_from_i(0), p);
    return make_int_w(w_of(v), p);
}

/* ================================================================ Fehler */

RTrace rt_trace(void) {
    RTrace t;
    t.n = rt_fdepth;
    t.labels = (const char**)xmalloc(sizeof(char*) * (size_t)(rt_fdepth + 1));
    for (int i = 0; i < rt_fdepth; i++) t.labels[i] = rt_frames[rt_fdepth - 1 - i];
    return t;
}

static char* vformat(const char* fmt, va_list ap) {
    va_list ap2;
    va_copy(ap2, ap);
    int n = vsnprintf(NULL, 0, fmt, ap2);
    va_end(ap2);
    char* s = (char*)xmalloc((size_t)n + 1);
    vsnprintf(s, (size_t)n + 1, fmt, ap);
    return s;
}

static int fatal_v(const char* name, char* msg) {
    rt_fatals++;
    if (fatal_pending) {
        /* combine (R-9.14): Der erste Programmfehler bleibt die Ursache. */
        Buf b = { 0 };
        buf_str(&b, fatal_msg);
        buf_fmt(&b, "; beim Aufräumen außerdem %s: %s", name, msg);
        xfree(fatal_msg);
        xfree(msg);
        fatal_msg = b.p;
        return ST_FATAL;
    }
    fatal_pending = 1;
    fatal_name = name;
    fatal_msg = msg;
    fatal_trace = rt_trace();
    return ST_FATAL;
}

int rt_fatal(const char* name, const char* fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    char* msg = vformat(fmt, ap);
    va_end(ap);
    return fatal_v(name, msg);
}

int rt_fatal_at(int st, int line) {
    if (st == ST_FATAL && fatal_pending && fatal_msg) {
        Buf b = { 0 };
        buf_fmt(&b, "%s (Zeile %d)", fatal_msg, line);
        xfree(fatal_msg);
        fatal_msg = b.p;
    }
    return st;
}

int rt_internal(const char* msg) { return rt_fatal("InternalError", "%s", msg); }

/* Native Grundwerte (M7): langsame Fälle über die rv-Pfade, damit Meldungen, Überlauf und
 * Releasemodus genau wie im Interpreter sind */
int rn_arith_slow(int64_t* out, int op, int64_t a, int64_t b, uint8_t tag) {
    rv v;
    int st = rt_arith(&v, op, rn_box(a, tag), rn_box(b, tag));
    if (st == ST_OK) {
        if (v.tag != tag) st = rt_convert_exact(&v, tag);
        *out = v.v.i;
    }
    return st;
}

int rn_neg_slow(int64_t* out, int64_t a, uint8_t tag) {
    rv v;
    int st = rt_neg(&v, rn_box(a, tag));
    if (st == ST_OK) *out = v.v.i;
    return st;
}

int rn_incdec_slow(int64_t* out, int64_t a, int d, uint8_t tag) {
    rv v;
    int st = rt_incdec(&v, rn_box(a, tag), d);
    if (st == ST_OK) *out = v.v.i;
    return st;
}

int rn_index_slow(int64_t* out, rv obj, int64_t i) {
    rv v;
    int st = rt_index_slow(&v, obj, rv_i64(i));
    if (st == ST_OK) *out = v.v.i;
    return st;
}

int rn_len_slow(int64_t* out, rv obj, int bytes) {
    rv v;
    int st = rt_member_len_slow(&v, obj, bytes);
    if (st == ST_OK) *out = v.v.i;
    return st;
}

/* RPP_NATCHECK: native Lesezugriffe prüfen die Art des Werts (Prüfung des Generators) */
int64_t rn_check(const rv* x, uint8_t tag) {
    if (x->tag != tag) {
        fprintf(stderr, "RPP_NATCHECK: Art %d statt %d\n", (int)x->tag, (int)tag);
        fflush(stderr);
        abort();
    }
    return x->v.i;
}

int rt_combine(int st, int d) {
    if (d == ST_FATAL) return ST_FATAL;
    if (st != ST_OK) return st;
    return d;
}

int rt_throw(rv ex) {
    rt_exc = ex;
    rt_exc_trace = rt_trace();
    return ST_THROW;
}

rv rt_take_exc(void) {
    rv e = rt_exc;
    rt_exc = rv_null();
    return e;
}

int rt_throw_std(const char* full, const char* msg) {
    rv a[1];
    a[0] = rt_str_c(msg);
    rv ex;
    int st = rt_std_new(&ex, full, 1, a);
    if (st) return st;
    return rt_throw(ex);
}

static int io_error(const char* msg, const char* path) {
    rv a[2];
    a[0] = rt_str_c(msg);
    a[1] = rt_str_c(path);
    rv ex;
    int st = rt_std_new(&ex, "rpp.core.IOException", 2, a);
    if (st) return st;
    return rt_throw(ex);
}

/* ================================================================ Rahmen */

int rt_enter_overflow(void) {
    return rt_fatal("StackOverflowError", "mehr als %d verschachtelte Aufrufe", RT_MAX_DEPTH);
}

int rt_depth(void) { return rt_fdepth; }

/* ================================================================ Strings */

static RStr* str_alloc(int64_t len) {
    RStr* s = (RStr*)xmalloc(sizeof(RStr) + (size_t)len + 1);
    s->rc = 1;
    s->len = len;
    s->chars = -1;
    s->data[len] = 0;
    return s;
}

static rv rv_str(RStr* s, uint8_t form) {
    rv r = rv_make(RT_STR);
    r.v.p = s;
    r.form = form;
    return r;
}

rv rt_str_from(const char* p, int64_t len) {
    RStr* s = str_alloc(len);
    memcpy(s->data, p, (size_t)len);
    return rv_str(s, F_OWN);
}

rv rt_str_c(const char* p) { return rt_str_from(p, (int64_t)strlen(p)); }

rv rt_lit(RStr** cache, const char* p, int64_t len) {
    if (!*cache) {
        RStr* s = str_alloc(len);
        memcpy(s->data, p, (size_t)len);
        s->rc = -1;
        *cache = s;
    }
    return rv_str(*cache, F_OWN);
}

RStr* rt_s(rv v) {
    static RStr* empty = NULL;
    if (v.tag == RT_STR) return (RStr*)v.v.p;
    if (!empty) {
        empty = str_alloc(0);
        empty->rc = -1;
    }
    return empty;
}

static rv buf_take(Buf* b) {
    rv r = rt_str_from(b->p ? b->p : "", b->n);
    xfree(b->p);
    return r;
}

/* UTF-8 */
static int utf8_dec(const char* s, int64_t n, int64_t i, uint32_t* c) {
    unsigned char b = (unsigned char)s[i];
    if (b < 0x80) { *c = b; return 1; }
    if ((b & 0xe0) == 0xc0 && i + 1 < n) { *c = ((uint32_t)(b & 0x1f) << 6) | (s[i + 1] & 0x3f); return 2; }
    if ((b & 0xf0) == 0xe0 && i + 2 < n) { *c = ((uint32_t)(b & 0x0f) << 12) | ((uint32_t)(s[i + 1] & 0x3f) << 6) | (s[i + 2] & 0x3f); return 3; }
    if (i + 3 < n) { *c = ((uint32_t)(b & 0x07) << 18) | ((uint32_t)(s[i + 1] & 0x3f) << 12) | ((uint32_t)(s[i + 2] & 0x3f) << 6) | (s[i + 3] & 0x3f); return 4; }
    *c = 0xfffd;
    return 1;
}

static int utf8_enc(uint32_t c, char* out) {
    if (c < 0x80) { out[0] = (char)c; return 1; }
    if (c < 0x800) { out[0] = (char)(0xc0 | (c >> 6)); out[1] = (char)(0x80 | (c & 0x3f)); return 2; }
    if (c < 0x10000) { out[0] = (char)(0xe0 | (c >> 12)); out[1] = (char)(0x80 | ((c >> 6) & 0x3f)); out[2] = (char)(0x80 | (c & 0x3f)); return 3; }
    out[0] = (char)(0xf0 | (c >> 18)); out[1] = (char)(0x80 | ((c >> 12) & 0x3f)); out[2] = (char)(0x80 | ((c >> 6) & 0x3f)); out[3] = (char)(0x80 | (c & 0x3f));
    return 4;
}

static int utf8_valid(const unsigned char* s, int64_t n) {
    int64_t i = 0;
    while (i < n) {
        unsigned char b = s[i];
        if (b < 0x80) { i++; continue; }
        int k;
        uint32_t c, min;
        if ((b & 0xe0) == 0xc0) { k = 1; c = b & 0x1f; min = 0x80; }
        else if ((b & 0xf0) == 0xe0) { k = 2; c = b & 0x0f; min = 0x800; }
        else if ((b & 0xf8) == 0xf0) { k = 3; c = b & 0x07; min = 0x10000; }
        else return 0;
        for (int j = 1; j <= k; j++) {
            if (i + j >= n || (s[i + j] & 0xc0) != 0x80) return 0;
            c = (c << 6) | (s[i + j] & 0x3f);
        }
        if (c < min || c > 0x10ffff || (c >= 0xd800 && c <= 0xdfff)) return 0;
        i += k + 1;
    }
    return 1;
}

int64_t rt_str_chars(RStr* s) {
    if (s->chars < 0) {
        int64_t n = 0;
        for (int64_t i = 0; i < s->len; i++)
            if (((unsigned char)s->data[i] & 0xc0) != 0x80) n++;
        s->chars = n;
    }
    return s->chars;
}

/* Byteposition des Zeichens k */
static int64_t char_pos(RStr* s, int64_t k) {
    if (rt_str_chars(s) == s->len) return k;
    int64_t i = 0;
    while (k > 0 && i < s->len) {
        i++;
        while (i < s->len && ((unsigned char)s->data[i] & 0xc0) == 0x80) i++;
        k--;
    }
    return i;
}

/* ---------------------------------------------------------------- Unicode */
/* Tabellen aus der Standardbibliothek von Rust (gen_unicode.rs), damit char und Strings sich wie im
 * Interpreter verhalten */
#include "rpp_unicode.h"

static int u_in(const uint32_t (*r)[2], int n, uint32_t c) {
    int lo = 0, hi = n - 1;
    while (lo <= hi) {
        int mid = (lo + hi) / 2;
        if (c < r[mid][0]) hi = mid - 1;
        else if (c > r[mid][1]) lo = mid + 1;
        else return 1;
    }
    return 0;
}

/* Abbildung (bis zu drei Zeichen); NULL, wenn das Zeichen bleibt */
static const uint32_t* u_map(const uint32_t (*m)[4], int n, uint32_t c) {
    int lo = 0, hi = n - 1;
    while (lo <= hi) {
        int mid = (lo + hi) / 2;
        if (c < m[mid][0]) hi = mid - 1;
        else if (c > m[mid][0]) lo = mid + 1;
        else return &m[mid][1];
    }
    return NULL;
}

static int u_is_whitespace(uint32_t c) { return u_in(u_space, U_SPACE_N, c); }
static int u_is_alpha(uint32_t c) { return u_in(u_alpha, U_ALPHA_N, c); }
static int u_is_upper(uint32_t c) { return u_in(u_upper, U_UPPER_N, c); }
static int u_is_lower(uint32_t c) { return u_in(u_lower, U_LOWER_N, c); }
static int u_is_cased(uint32_t c) { return u_in(u_cased, U_CASED_N, c); }
static int u_is_ignorable(uint32_t c) { return u_in(u_ignorable, U_IGNORABLE_N, c); }

/* erstes Zeichen von to_uppercase / to_lowercase (char.toUpper, char.toLower) */
static uint32_t u_to_upper(uint32_t c) {
    const uint32_t* m = u_map(u_upmap, U_UPMAP_N, c);
    return m ? m[0] : c;
}

static uint32_t u_to_lower(uint32_t c) {
    const uint32_t* m = u_map(u_lowmap, U_LOWMAP_N, c);
    return m ? m[0] : c;
}

/* ---------------------------------------------------------------- Zahlen als Text */

/* kürzeste Ziffernfolge, die den Wert wiederherstellt (wie Display in Rust) */
static void shortest(double x, int is32, char* digits, int* exp10) {
    char tmp[64];
    for (int p = 1; p <= 17; p++) {
        snprintf(tmp, sizeof tmp, "%.*e", p - 1, x);
        int ok;
        if (is32) ok = (float)strtod(tmp, NULL) == (float)x;
        else ok = strtod(tmp, NULL) == x;
        if (ok || p == 17) break;
    }
    /* tmp: d.ddde[+-]xx */
    int k = 0;
    char* e = strchr(tmp, 'e');
    for (char* q = tmp; q < e; q++)
        if (*q >= '0' && *q <= '9') digits[k++] = *q;
    while (k > 1 && digits[k - 1] == '0') k--;
    digits[k] = 0;
    *exp10 = atoi(e + 1);
}

static void float_text(Buf* b, double x, int is32) {
    if (x != x) { buf_str(b, "NaN"); return; }
    if (x == HUGE_VAL || x == -HUGE_VAL) { buf_str(b, x > 0 ? "Infinity" : "-Infinity"); return; }
    if (x == 0) { buf_str(b, signbit(x) ? "-0" : "0"); return; }
    if (x < 0) { buf_str(b, "-"); x = -x; }
    char d[32];
    int e;
    shortest(x, is32, d, &e);
    int n = (int)strlen(d);
    /* Wert = 0.d * 10^(e+1) */
    int point = e + 1;
    if (point <= 0) {
        buf_str(b, "0.");
        for (int i = 0; i < -point; i++) buf_str(b, "0");
        buf_str(b, d);
    } else if (point >= n) {
        buf_str(b, d);
        for (int i = 0; i < point - n; i++) buf_str(b, "0");
    } else {
        buf_add(b, d, point);
        buf_str(b, ".");
        buf_str(b, d + point);
    }
}

static void int_text(Buf* b, rv v) {
    char t[64];
    w_text(w_of(v), t);
    buf_str(b, t);
}

static void text_of(Buf* b, rv v) {
    switch (v.tag) {
    case RT_BOOL: buf_str(b, v.v.i ? "true" : "false"); break;
    case RT_CHAR: { char t[4]; int n = utf8_enc((uint32_t)v.v.u, t); buf_add(b, t, n); break; }
    case RT_BYTE: buf_fmt(b, "%u", (unsigned)v.v.u); break;
    case RT_F32: float_text(b, (double)v.v.f32, 1); break;
    case RT_F64: float_text(b, v.v.f, 0); break;
    case RT_STR: { RStr* s = rt_s(v); buf_add(b, s->data, s->len); break; }
    case RT_NULL: buf_str(b, "null"); break;
    default:
        if (rt_is_int_tag(v.tag)) int_text(b, v);
        break;
    }
}

rv rt_text(rv v) {
    if (v.tag == RT_STR) {
        RStr* s = rt_s(v);
        if (s->rc >= 0) s->rc++;
        return rv_str(s, F_OWN);
    }
    Buf b = { 0 };
    text_of(&b, v);
    return buf_take(&b);
}

/* ================================================================ Besitz */

static int destruct(RObj* o);

static void obj_free(RObj* o) {
    if (rt_nofree) return;
    xfree(o->slots);
    o->slots = NULL;
    o->len = o->cap = 0;
    if (o->native) {
        xfree(o->native);
        o->native = NULL;
    }
    if (o->weak == 0) xfree(o);
    else o->freed = 1;
}

static int drop_agg(RAgg* a, uint8_t tag) {
    (void)tag;
    if (--a->rc > 0) return ST_OK;
    int st = ST_OK;
    for (int i = a->n - 1; i >= 0; i--) {
        int d = rt_drop(a->items[i]);
        if (d) { st = d; break; }
    }
    if (!rt_nofree) xfree(a);
    return st;
}

int rt_drop_slow(rv v) {
    switch (v.tag) {
    case RT_OBJ: {
        RObj* o = (RObj*)v.v.p;
        if (v.form == F_OWN) return destruct(o);
        if (v.form == F_SHARED) {
            if (o->rc > 0) o->rc--;
            if (o->rc == 0) return destruct(o);
            return ST_OK;
        }
        if (v.form == F_WEAK) {
            if (o->weak > 0) o->weak--;
            if (o->weak == 0 && o->freed && !rt_nofree) xfree(o);
        }
        return ST_OK;
    }
    case RT_STR:
        if (v.form == F_OWN) {
            RStr* s = (RStr*)v.v.p;
            if (s->rc > 0 && --s->rc == 0 && !rt_nofree) xfree(s);
        }
        return ST_OK;
    case RT_TUP: case RT_ENUMD:
        if (v.form == F_OWN) return drop_agg((RAgg*)v.v.p, v.tag);
        return ST_OK;
    case RT_FUNC:
        if (v.form == F_OWN) {
            RFun* f = (RFun*)v.v.p;
            if (--f->rc > 0) return ST_OK;
            int st = ST_OK;
            if (f->own) {
                for (int i = f->n - 1; i >= 0; i--) {
                    rv x = f->vals[i];
                    f->vals[i] = rv_moved();
                    st = rt_drop(x);
                    if (st) break;
                }
            }
            if (!rt_nofree) {
                xfree(f->cells);
                xfree(f->vals);
                xfree(f);
            }
            return st;
        }
        return ST_OK;
    default:
        return ST_OK;
    }
}

/* ---------------------------------------------------------------- Zwischenwerte */

static rv* temps = NULL;
int64_t rt_ntemps = 0;
#define ntemps rt_ntemps
static int64_t captemps = 0;

rv rt_temp_push(rv v) {
    if (ntemps == captemps) {
        captemps = captemps ? captemps * 2 : 256;
        temps = (rv*)xrealloc(temps, sizeof(rv) * (size_t)captemps);
    }
    temps[ntemps++] = v;
    return rv_view(v);
}

int rt_trelease_slow(int64_t mark, int st) {
    int d = ST_OK;
    while (ntemps > mark) {
        rv t = temps[--ntemps];
        int x = rt_drop(t);
        if (x) d = x;
    }
    return rt_combine(st, d);
}

void rt_tdiscard(int64_t mark) {
    if (ntemps > mark) ntemps = mark;
}

int rt_scope_drop_slow(int st, rv* vars, int n) {
    for (int i = n - 1; i >= 0; i--) {
        rv x = vars[i];
        vars[i] = rv_moved();
        int d = rt_drop(x);
        st = rt_combine(st, d);
        if (st == ST_FATAL && d == ST_FATAL && rt_fatals >= 2) {
            /* weitere Destruktoren laufen nicht (R-9.14) */
        }
    }
    return st;
}

/* ---------------------------------------------------------------- Kopie und Übergabe */

static RAgg* agg_new(int n) {
    RAgg* a = (RAgg*)xmalloc(sizeof(RAgg) + sizeof(rv) * (size_t)n);
    a->rc = 1;
    a->def = 0;
    a->idx = 0;
    a->n = n;
    return a;
}

rv rt_promote_slow(rv v) {
    switch (v.tag) {
    case RT_OBJ: {
        RObj* o = (RObj*)v.v.p;
        if (v.form == F_BORROW && o->rc > 0) {
            o->rc++;
            v.form = F_SHARED;
        } else if (v.form == F_WEAKB) {
            o->weak++;
            v.form = F_WEAK;
        }
        return v;
    }
    case RT_STR:
        if (v.form == F_BORROW) {
            RStr* s = (RStr*)v.v.p;
            if (s->rc >= 0) s->rc++;
            v.form = F_OWN;
        }
        return v;
    case RT_TUP:
        if (v.form == F_BORROW) {
            RAgg* a = (RAgg*)v.v.p;
            RAgg* n = agg_new(a->n);
            for (int i = 0; i < a->n; i++) n->items[i] = rt_promote(rv_view(a->items[i]));
            rv r = rv_make(RT_TUP);
            r.v.p = n;
            return r;
        }
        return v;
    default:
        return v;
    }
}

rv rt_own_copy(rv v) {
    switch (v.tag) {
    case RT_OBJ: {
        RObj* o = (RObj*)v.v.p;
        if ((v.form == F_BORROW && o->rc > 0) || v.form == F_SHARED) {
            o->rc++;
            v.form = F_SHARED;
        } else if (v.form == F_WEAK || v.form == F_WEAKB) {
            o->weak++;
            v.form = F_WEAK;
        }
        return v;
    }
    case RT_STR:
        return rt_promote(v);
    case RT_TUP: {
        RAgg* a = (RAgg*)v.v.p;
        RAgg* n = agg_new(a->n);
        for (int i = 0; i < a->n; i++) n->items[i] = rt_own_copy(v.form == F_BORROW ? rv_view(a->items[i]) : a->items[i]);
        rv r = rv_make(RT_TUP);
        r.v.p = n;
        return r;
    }
    case RT_FUNC: {
        RFun* f = (RFun*)v.v.p;
        f->rc++;
        v.form = F_OWN;
        return v;
    }
    case RT_ENUMD: {
        RAgg* a = (RAgg*)v.v.p;
        a->rc++;
        v.form = F_OWN;
        return v;
    }
    default:
        return v;
    }
}

int rt_plain(rv v) {
    switch (v.tag) {
    case RT_OBJ: case RT_FUNC: case RT_MOVED: case RT_UNINIT: return 0;
    case RT_ENUM: return rt_enums[v.x]->plain;
    case RT_ENUMD: return rt_enums[((RAgg*)v.v.p)->def]->plain;
    case RT_TUP: {
        RAgg* a = (RAgg*)v.v.p;
        for (int i = 0; i < a->n; i++)
            if (!rt_plain(a->items[i])) return 0;
        return 1;
    }
    default: return 1;
    }
}

/* gemeinsam genutzte Kopie eines unveränderlichen Werts (Tupel, Enum mit Daten) */
static rv share(rv v) {
    if (v.tag == RT_TUP || v.tag == RT_ENUMD) {
        if (v.form == F_BORROW) return rt_own_copy(v);
        ((RAgg*)v.v.p)->rc++;
        v.form = F_OWN;
        return v;
    }
    if (v.tag == RT_STR) return rt_promote(rv_view(v));
    return v;
}

rv rt_transfer_var(rv* cell) {
    rv cur = *cell;
    switch (cur.tag) {
    case RT_OBJ: {
        RObj* o = (RObj*)cur.v.p;
        if ((cur.form == F_SHARED || cur.form == F_BORROW) && o->rc > 0) {
            o->rc++;
            cur.form = F_SHARED;
            return cur;
        }
        if (cur.form == F_OWN) {
            *cell = rv_moved();
            return cur;
        }
        if (cur.form == F_WEAK || cur.form == F_WEAKB) {
            o->weak++;
            cur.form = F_WEAK;
        }
        return cur;
    }
    case RT_TUP: case RT_ENUM: case RT_ENUMD:
        if (rt_plain(cur)) return share(cur);
        if (cur.form == F_BORROW) return cur;
        *cell = rv_moved();
        return cur;
    case RT_FUNC: {
        RFun* f = (RFun*)cur.v.p;
        if (f->own && cur.form == F_OWN) {
            *cell = rv_moved();
            return cur;
        }
        return rt_own_copy(cur);
    }
    case RT_STR:
        return share(cur);
    default:
        return cur;
    }
}

rv rt_to_shared(rv v) {
    if (v.tag == RT_OBJ && v.form == F_OWN) {
        ((RObj*)v.v.p)->rc++;
        v.form = F_SHARED;
    }
    return v;
}

int rt_to_weak(rv* v) {
    if (v->tag == RT_OBJ && (v->form == F_SHARED || v->form == F_BORROW)) {
        RObj* o = (RObj*)v->v.p;
        rv old = *v;
        o->weak++;
        v->form = F_WEAK;
        if (old.form == F_SHARED) return rt_drop(old);
    }
    return ST_OK;
}

static int int_result(rv* out, W v, uint8_t p) {
    if (w_fits(v, p) || rt_release) {
        *out = make_int_w(v, p);
        return ST_OK;
    }
    char t[64];
    w_text(v, t);
    return rt_fatal("OverflowError", "Überlauf: %s passt nicht in %s", t, prim_name(p));
}

int rt_int_result_i64(rv* out, int64_t v, uint8_t tag) { return int_result(out, w_from_i(v), tag); }

int rt_convert_exact(rv* v, uint8_t p) {
    if (rt_is_int_tag(p)) {
        if (rt_is_int_tag(v->tag)) return int_result(v, w_of(*v), p);
        return ST_OK;
    }
    if (p == RT_F32 || p == RT_F64) *v = rt_num_to(*v, p);
    return ST_OK;
}

/* ================================================================ Objekte */

RObj* rt_obj_new(const RClass* c, const RTy** targs, int32_t ntargs) {
    RObj* o = (RObj*)xmalloc(sizeof(RObj));
    memset(o, 0, sizeof *o);
    o->cls = c;
    o->alive = 1;
    o->len = o->cap = c->nfields;
    o->slots = (rv*)xmalloc(sizeof(rv) * (size_t)(c->nfields ? c->nfields : 1));
    for (int i = 0; i < c->nfields; i++) o->slots[i] = rv_uninit();
    o->targs = targs;
    o->ntargs = ntargs;
    return o;
}

void rt_obj_fail(RObj* o) {
    o->alive = 0;
}

RObj* rt_vec_new(int64_t cap) {
    RObj* o = (RObj*)xmalloc(sizeof(RObj));
    memset(o, 0, sizeof *o);
    o->alive = 1;
    o->cap = cap < 4 ? 4 : cap;
    o->slots = (rv*)xmalloc(sizeof(rv) * (size_t)o->cap);
    return o;
}

static void vec_reserve(RObj* v, int64_t n) {
    if (n <= v->cap) return;
    int64_t c = v->cap ? v->cap * 2 : 4;
    while (c < n) c *= 2;
    v->slots = (rv*)xrealloc(v->slots, sizeof(rv) * (size_t)c);
    v->cap = c;
}

void rt_vec_push(RObj* v, rv x) {
    vec_reserve(v, v->len + 1);
    v->slots[v->len++] = x;
}

rv rt_vec_from(int n, rv* items) {
    RObj* v = rt_vec_new(n);
    for (int i = 0; i < n; i++) rt_vec_push(v, items[i]);
    return rv_obj(v, F_OWN);
}

rv rt_tuple(int n, rv* items) {
    RAgg* a = agg_new(n);
    for (int i = 0; i < n; i++) a->items[i] = items[i];
    rv r = rv_make(RT_TUP);
    r.v.p = a;
    return r;
}

rv rt_enumd(int32_t def, int32_t idx, int n, rv* items) {
    RAgg* a = agg_new(n);
    a->def = def;
    a->idx = idx;
    for (int i = 0; i < n; i++) a->items[i] = items[i];
    rv r = rv_make(RT_ENUMD);
    r.v.p = a;
    return r;
}

int rt_is_subclass(const RClass* c, int32_t def) {
    if (!c) return 0;
    if (c->id == def) return 1;
    for (int i = 0; i < c->nsupers; i++)
        if (c->supers[i] == def) return 1;
    return 0;
}

/* Laufzeittypen werden geteilt; einfache Liste */
typedef struct TyNode {
    RTy t;
    struct TyNode* next;
} TyNode;
static TyNode* tys = NULL;

const RTy* rt_ty(int32_t kind, int32_t def, int32_t n, const RTy** args) {
    for (TyNode* p = tys; p; p = p->next) {
        if (p->t.kind != kind || p->t.def != def || p->t.n != n) continue;
        int same = 1;
        for (int i = 0; i < n; i++)
            if (p->t.args[i] != args[i]) same = 0;
        if (same) return &p->t;
    }
    TyNode* q = (TyNode*)xmalloc(sizeof(TyNode));
    q->t.kind = kind;
    q->t.def = def;
    q->t.n = n;
    q->t.args = (const RTy**)xmalloc(sizeof(RTy*) * (size_t)(n + 1));
    for (int i = 0; i < n; i++) q->t.args[i] = args[i];
    q->next = tys;
    tys = q;
    return &q->t;
}

int rt_check_lock(RObj* o, int mutable, int line) {
    int b = o->lock;
    if ((!mutable && b == -1) || (mutable && b != 0)) {
        int st = rt_fatal("BorrowError", "das shared-Objekt ist %s gesperrt", b == -1 ? "schreibend" : "lesend");
        if (line > 0) rt_fatal_at(st, line);
        return st;
    }
    return ST_OK;
}

int rt_acquire(RObj* o, int mutable, int line) {
    int st = rt_check_lock(o, mutable, line);
    if (st) return st;
    o->lock = mutable ? -1 : o->lock + 1;
    return ST_OK;
}

void rt_unlock(RObj* o, int mutable) {
    if (mutable) o->lock = 0;
    else o->lock = o->lock - 1 > 0 ? o->lock - 1 : 0;
}

int rt_lock_weak(rv* out, rv w) {
    if (w.tag == RT_OBJ) {
        RObj* o = (RObj*)w.v.p;
        if (o->alive && o->rc > 0) {
            o->rc++;
            *out = rv_obj(o, F_SHARED);
            return ST_OK;
        }
    }
    *out = rv_null();
    return ST_OK;
}

int rt_drop_fields(RObj* o, int64_t upto) {
    for (int64_t k = upto - 1; k >= 0; k--) {
        if (k >= o->len) continue;
        rv x = o->slots[k];
        o->slots[k] = rv_moved();
        int st = rt_drop(x);
        if (st) return st;
    }
    return ST_OK;
}

static int run_dtor(RObj* o, const RClass* c) {
    if (!c->dtor) return ST_OK;
    int st = c->dtor(o);
    if (st == ST_THROW) {
        rv ex = rt_take_exc();
        rv msg;
        rt_describe(&msg, ex);
        RStr* s = rt_s(msg);
        return rt_fatal("DestructorError", "Exception verlässt einen Destruktor: %s", s->data);
    }
    return st;
}

int rt_run_dtors(RObj* o, const RClass* from) {
    for (const RClass* c = from; c; c = c->base) {
        int st = run_dtor(o, c);
        if (st) return st;
    }
    return ST_OK;
}

static int native_drop(RObj* o);

/* V-7: Destruktoren von der Klasse zur Basis, dann die Felder rückwärts */
static int destruct(RObj* o) {
    if (!o->alive || o->dying || rt_fatals >= 2) return ST_OK;
    o->dying = 1;
    int st = ST_OK;
    if (!o->cls) {
        int64_t n = o->len;
        rv* elems = o->slots;
        o->slots = NULL;
        o->len = o->cap = 0;
        for (int64_t i = 0; i < n; i++) {
            int d = rt_drop(elems[i]);
            if (d) st = d;
        }
        xfree(elems);
    } else {
        st = rt_run_dtors(o, o->cls);
        if (!st) st = rt_drop_fields(o, o->len);
        if (!st && o->native) st = native_drop(o);
    }
    o->alive = 0;
    obj_free(o);
    return st;
}

int rt_unpack(rv v, int n, rv* out) {
    if (v.tag == RT_TUP) {
        RAgg* a = (RAgg*)v.v.p;
        for (int i = 0; i < n && i < a->n; i++) out[i] = (v.form == F_BORROW) ? rv_view(a->items[i]) : (a->rc == 1 ? a->items[i] : share(a->items[i]));
        if (v.form == F_OWN) {
            if (a->rc == 1) {
                if (!rt_nofree) xfree(a);
            } else {
                a->rc--;
            }
        }
        return ST_OK;
    }
    if (v.tag == RT_OBJ && v.form == F_OWN && ((RObj*)v.v.p)->cls) {
        /* R-5.25: Zerlegen; das Objekt wird ohne Destruktor freigegeben */
        RObj* o = (RObj*)v.v.p;
        for (int i = 0; i < n; i++) {
            out[i] = o->slots[i];
            o->slots[i] = rv_moved();
        }
        o->alive = 0;
        return ST_OK;
    }
    return rt_internal("Tupel oder own-Objekt erwartet");
}

int rt_index_slot_slow(rv** out, rv ov, rv idx) {
    W w = w_of(idx);
    int64_t i = (int64_t)w.lo;
    int neg = w_neg_p(w) || !rt_is_int_tag(idx.tag);
    if (!rt_is_int_tag(idx.tag)) i = -1;
    if (ov.tag == RT_OBJ) {
        RObj* o = (RObj*)ov.v.p;
        if (neg || i >= o->len) return rt_fatal("IndexError", "Index %lld außerhalb von 0..%lld", (long long)i, (long long)o->len);
        *out = &o->slots[i];
        return ST_OK;
    }
    if (ov.tag == RT_SLICE) {
        RObj* o = (RObj*)ov.v.p;
        int64_t a = ov.x, b = ov.y;
        if (neg || a + i >= b) return rt_fatal("IndexError", "Index %lld außerhalb von 0..%lld", (long long)i, (long long)(b - a));
        *out = &o->slots[a + i];
        return ST_OK;
    }
    return rt_internal("Index auf diesem Wert");
}

int rt_index_slow(rv* out, rv ov, rv idx) {
    if (ov.tag == RT_STR) {
        RStr* s = rt_s(ov);
        int64_t n = rt_str_chars(s);
        int64_t i = rt_is_int_tag(idx.tag) && !w_neg_p(w_of(idx)) ? idx.v.i : -1;
        if (i < 0 || i >= n) return rt_fatal("IndexError", "Index %lld außerhalb von 0..%lld", (long long)i, (long long)n);
        int64_t p = char_pos(s, i);
        uint32_t c;
        utf8_dec(s->data, s->len, p, &c);
        *out = rv_char(c);
        return ST_OK;
    }
    rv* slot;
    int st = rt_index_slot(&slot, ov, idx);
    if (st) return st;
    *out = rv_view(*slot);
    return ST_OK;
}

int rt_slice(rv* out, rv ov, int has_from, rv from, int has_to, rv to) {
    RObj* o;
    int64_t base, len;
    if (ov.tag == RT_OBJ) {
        o = (RObj*)ov.v.p;
        base = 0;
        len = o->len;
    } else if (ov.tag == RT_SLICE) {
        o = (RObj*)ov.v.p;
        base = ov.x;
        len = ov.y - ov.x;
    } else {
        return rt_internal("Bereich auf diesem Wert");
    }
    int64_t a = has_from ? from.v.i : 0;
    int64_t b = has_to ? to.v.i : len;
    if (a < 0 || b < a || b > len) return rt_fatal("IndexError", "Bereich %lld..%lld außerhalb von 0..%lld", (long long)a, (long long)b, (long long)len);
    rv r = rv_make(RT_SLICE);
    r.v.p = o;
    r.x = (int32_t)(base + a);
    r.y = (int32_t)(base + b);
    r.form = F_BORROW;
    *out = r;
    return ST_OK;
}

int rt_member_len_slow(rv* out, rv ov, int bytes) {
    if (ov.tag == RT_STR) {
        RStr* s = rt_s(ov);
        *out = rv_i64(bytes ? s->len : rt_str_chars(s));
        return ST_OK;
    }
    if (ov.tag == RT_OBJ && !((RObj*)ov.v.p)->cls) {
        *out = rv_i64(((RObj*)ov.v.p)->len);
        return ST_OK;
    }
    if (ov.tag == RT_SLICE) {
        *out = rv_i64(ov.y - ov.x);
        return ST_OK;
    }
    return rt_internal("length auf diesem Wert");
}

/* copy (R-7.11): copy-Rümpfe von der Wurzel bis zur Klasse */
static int copy_levels(RObj* n, const RClass* c, rv src) {
    if (c->base) {
        int st = copy_levels(n, c->base, src);
        if (st) return st;
    }
    int st = c->init ? c->init(n) : ST_OK;
    if (st) return st;
    if (c->copy) return c->copy(n, src);
    RObj* o = (RObj*)src.v.p;
    for (int k = c->first; k < c->nfields; k++) {
        rv x = o->slots[k];
        if (x.tag == RT_OBJ && x.form == F_OWN) {
            rv y;
            st = rt_deep_copy(&y, x);
            if (st) return st;
            n->slots[k] = y;
        } else {
            n->slots[k] = rt_own_copy(x);
        }
    }
    return ST_OK;
}

int rt_deep_copy(rv* out, rv v) {
    if (v.tag == RT_OBJ) {
        RObj* o = (RObj*)v.v.p;
        if (!o->cls) {
            RObj* n = rt_vec_new(o->len);
            for (int64_t i = 0; i < o->len; i++) {
                rv x = o->slots[i];
                /* schneller Pfad (M7): Grundwerte, Enums ohne Daten, Bereiche und Strings
                 * (wie share, ohne Aufruf je Element; n hat schon Platz für o->len) */
                if (x.tag < RT_STR || x.tag == RT_ENUM || x.tag == RT_SLICE) {
                    n->slots[n->len++] = x;
                    continue;
                }
                if (x.tag == RT_STR) {
                    RStr* s = (RStr*)x.v.p;
                    if (s->rc >= 0) s->rc++;
                    x.form = F_OWN;
                    n->slots[n->len++] = x;
                    continue;
                }
                if (x.tag == RT_OBJ || x.tag == RT_TUP || x.tag == RT_ENUM || x.tag == RT_ENUMD) {
                    rv y;
                    int st = rt_deep_copy(&y, x);
                    if (st) return st;
                    rt_vec_push(n, y);
                } else {
                    rt_vec_push(n, share(x));
                }
            }
            *out = rv_obj(n, F_OWN);
            return ST_OK;
        }
        RObj* n = rt_obj_new(o->cls, o->targs, o->ntargs);
        int st = copy_levels(n, o->cls, rv_view(v));
        if (st) return st;
        *out = rv_obj(n, F_OWN);
        return ST_OK;
    }
    if (v.tag == RT_TUP) {
        RAgg* a = (RAgg*)v.v.p;
        RAgg* n = agg_new(a->n);
        for (int i = 0; i < a->n; i++) {
            int st = rt_deep_copy(&n->items[i], a->items[i]);
            if (st) return st;
        }
        rv r = rv_make(RT_TUP);
        r.v.p = n;
        *out = r;
        return ST_OK;
    }
    *out = share(v);
    return ST_OK;
}

int rt_describe(rv* out, rv ex) {
    *out = rt_str_c("");
    if (ex.tag != RT_OBJ) return ST_OK;
    RObj* o = (RObj*)ex.v.p;
    if (!o->cls || !o->cls->describe) return ST_OK;
    rv r;
    int st = o->cls->describe(&r, rv_obj(o, F_BORROW), NULL);
    if (st) {
        if (st == ST_THROW) rt_take_exc();
        return ST_OK;
    }
    if (r.tag == RT_STR) *out = r;
    return ST_OK;
}

int rt_catch_matches(int32_t def) {
    if (rt_exc.tag != RT_OBJ) return 0;
    RObj* o = (RObj*)rt_exc.v.p;
    return rt_is_subclass(o->cls, def);
}

/* ================================================================ Lambdas */

RFun* rt_fun_new(RFn fn, int own, int n) {
    RFun* f = (RFun*)xmalloc(sizeof(RFun));
    f->rc = 1;
    f->own = (uint8_t)own;
    f->fn = fn;
    f->self = rv_null();
    f->mt = NULL;
    f->n = n;
    f->cells = (rv**)xmalloc(sizeof(rv*) * (size_t)(n ? n : 1));
    f->vals = own ? (rv*)xmalloc(sizeof(rv) * (size_t)(n ? n : 1)) : NULL;
    if (own)
        for (int i = 0; i < n; i++) f->cells[i] = &f->vals[i];
    return f;
}

rv rv_fun(RFun* f, uint8_t form) {
    rv r = rv_make(RT_FUNC);
    r.v.p = f;
    r.form = form;
    return r;
}

int rt_call_value(rv* ret, rv f, rv* args) {
    if (f.tag != RT_FUNC) return rt_internal("Aufruf eines Werts, der keine Funktion ist");
    RFun* c = (RFun*)f.v.p;
    return c->fn(ret, c, args);
}

int rt_capture_own(rv* cell, rv* out) {
    rv cur = *cell;
    switch (cur.tag) {
    case RT_TUP: case RT_ENUM: case RT_ENUMD:
        if (rt_plain(cur)) { *out = share(cur); return ST_OK; }
        *cell = rv_moved();
        *out = cur;
        return ST_OK;
    case RT_OBJ:
        if (cur.form == F_OWN) {
            *cell = rv_moved();
            *out = cur;
            return ST_OK;
        }
        *out = rt_own_copy(cur);
        return ST_OK;
    case RT_FUNC:
        if (((RFun*)cur.v.p)->own) {
            *cell = rv_moved();
            *out = cur;
            return ST_OK;
        }
        *out = rt_own_copy(cur);
        return ST_OK;
    default:
        *out = rt_own_copy(cur);
        return ST_OK;
    }
}

/* ================================================================ Arithmetik */

int rt_arith_slow(rv* out, int op, rv a, rv b) {
    if (a.tag == b.tag && (a.tag == RT_I32 || a.tag == RT_I64) && op <= OP_REM) {
        int64_t x = a.v.i, y = b.v.i, r;
        if (a.tag == RT_I32) {
            switch (op) {
            case OP_ADD: r = x + y; break;
            case OP_SUB: r = x - y; break;
            case OP_MUL: r = x * y; break;
            default:
                if (y == 0) return rt_fatal("DivisionError", "Division durch null");
                r = op == OP_DIV ? x / y : x % y;
                break;
            }
            if (r >= INT32_MIN && r <= INT32_MAX) {
                *out = rv_i32((int32_t)r);
                return ST_OK;
            }
        } else if (op == OP_ADD && !((y > 0 && x > INT64_MAX - y) || (y < 0 && x < INT64_MIN - y))) {
            *out = rv_i64(x + y);
            return ST_OK;
        } else if (op == OP_SUB && !((y < 0 && x > INT64_MAX + y) || (y > 0 && x < INT64_MIN + y))) {
            *out = rv_i64(x - y);
            return ST_OK;
        } else if ((op == OP_DIV || op == OP_REM) && y != 0 && !(x == INT64_MIN && y == -1)) {
            *out = rv_i64(op == OP_DIV ? x / y : x % y);
            return ST_OK;
        }
    }
    if (a.tag == RT_STR && b.tag == RT_STR && op == OP_ADD) {
        RStr* x = rt_s(a);
        RStr* y = rt_s(b);
        RStr* s = str_alloc(x->len + y->len);
        memcpy(s->data, x->data, (size_t)x->len);
        memcpy(s->data + x->len, y->data, (size_t)y->len);
        if (x->chars >= 0 && y->chars >= 0) s->chars = x->chars + y->chars;
        *out = rv_str(s, F_OWN);
        return ST_OK;
    }
    if (a.tag == RT_BYTE && b.tag == RT_BYTE) {
        uint8_t r = op == OP_AND ? (uint8_t)(a.v.u & b.v.u) : op == OP_OR ? (uint8_t)(a.v.u | b.v.u) : (uint8_t)(a.v.u ^ b.v.u);
        *out = rv_byte(r);
        return ST_OK;
    }
    if (op == OP_SHL || op == OP_SHR) {
        uint8_t p = rt_is_int_tag(a.tag) ? a.tag : (a.tag == RT_BYTE ? RT_BYTE : RT_I32);
        int bits = (p == RT_I8 || p == RT_U8 || p == RT_BYTE) ? 8 : (p == RT_I16 || p == RT_U16) ? 16 : (p == RT_I32 || p == RT_U32) ? 32 : 64;
        W wn = w_of(b);
        int64_t n;
        if (w_neg_p(wn) || wn.hi != 0 || wn.lo >= (uint64_t)bits) {
            if (!rt_release) {
                char t[64];
                w_text(wn, t);
                return rt_fatal("OverflowError", "Schiebeweite %s außerhalb von 0..%d", t, bits);
            }
            /* rem_euclid */
            int64_t m = (int64_t)(wn.lo % (uint64_t)bits);
            if (w_neg_p(wn)) {
                W q, r;
                w_divrem(wn, w_from_i(bits), &q, &r);
                m = (int64_t)r.lo;
                if (m < 0) m += bits;
                m = (int64_t)r.lo == 0 ? 0 : m;
                if (r.hi < 0) m = bits + (int64_t)r.lo;
            }
            n = m;
        } else {
            n = (int64_t)wn.lo;
        }
        W x = w_of(a);
        W r;
        if (op == OP_SHL) {
            r = w_from_u(x.lo << n);
        } else if (is_unsigned(p) || p == RT_BYTE) {
            r = w_from_u(x.lo >> n);
        } else {
            r = w_from_i((int64_t)x.lo >> n);
        }
        *out = make_int_w(r, p);
        return ST_OK;
    }
    if (!rt_is_num_tag(a.tag) || !rt_is_num_tag(b.tag)) return rt_internal("Arithmetik auf diesen Werten");
    uint8_t p = common_numeric(a.tag, b.tag);
    if (!p) p = a.tag;
    if (is_float_tag(p)) {
        if (p == RT_F32) {
            float x = rt_num_to(a, RT_F32).v.f32, y = rt_num_to(b, RT_F32).v.f32, r;
            switch (op) {
            case OP_ADD: r = x + y; break;
            case OP_SUB: r = x - y; break;
            case OP_MUL: r = x * y; break;
            case OP_DIV: r = x / y; break;
            default: r = fmodf(x, y); break;
            }
            *out = rv_f32(r);
            return ST_OK;
        }
        double x = as_f64(a), y = as_f64(b), r;
        switch (op) {
        case OP_ADD: r = x + y; break;
        case OP_SUB: r = x - y; break;
        case OP_MUL: r = x * y; break;
        case OP_DIV: r = x / y; break;
        default: r = fmod(x, y); break;
        }
        *out = rv_f64(r);
        return ST_OK;
    }
    W x = w_of(a), y = w_of(b), r;
    switch (op) {
    case OP_ADD: r = w_add(x, y); break;
    case OP_SUB: r = w_sub(x, y); break;
    case OP_MUL: r = w_mul(x, y); break;
    case OP_DIV: case OP_REM: {
        if (y.lo == 0 && y.hi == 0) return rt_fatal("DivisionError", "Division durch null");
        W q, m;
        w_divrem(x, y, &q, &m);
        r = op == OP_DIV ? q : m;
        break;
    }
    case OP_AND: r.lo = x.lo & y.lo; r.hi = x.hi & y.hi; *out = make_int_w(r, p); return ST_OK;
    case OP_OR: r.lo = x.lo | y.lo; r.hi = x.hi | y.hi; *out = make_int_w(r, p); return ST_OK;
    case OP_XOR: r.lo = x.lo ^ y.lo; r.hi = x.hi ^ y.hi; *out = make_int_w(r, p); return ST_OK;
    default: return rt_internal("Operator");
    }
    return int_result(out, r, p);
}

int rt_neg(rv* out, rv a) {
    if (a.tag == RT_F32) { *out = rv_f32(-a.v.f32); return ST_OK; }
    if (a.tag == RT_F64) { *out = rv_f64(-a.v.f); return ST_OK; }
    uint8_t p = rt_is_int_tag(a.tag) ? a.tag : RT_I32;
    return int_result(out, w_negate(w_of(a)), p);
}

rv rt_bitnot(rv a) {
    uint8_t p = rt_is_int_tag(a.tag) ? a.tag : (a.tag == RT_BYTE ? RT_BYTE : RT_I32);
    W w = w_of(a);
    w.lo = ~w.lo;
    w.hi = ~w.hi;
    return make_int_w(w, p);
}

int rt_incdec_slow(rv* out, rv cur, int d) {
    if (cur.tag == RT_I32 && cur.v.i + d >= INT32_MIN && cur.v.i + d <= INT32_MAX) {
        *out = rv_i32((int32_t)(cur.v.i + d));
        return ST_OK;
    }
    if (cur.tag == RT_I64 && !(d > 0 && cur.v.i == INT64_MAX) && !(d < 0 && cur.v.i == INT64_MIN)) {
        *out = rv_i64(cur.v.i + d);
        return ST_OK;
    }
    uint8_t p = rt_is_int_tag(cur.tag) ? cur.tag : RT_I32;
    W w = rt_is_int_tag(cur.tag) ? w_of(cur) : w_from_i(0);
    return int_result(out, w_add(w, w_from_i(d)), p);
}

int rt_equal_slow(int* out, rv a, rv b) {
    if (a.tag == b.tag && ((a.tag >= RT_I8 && a.tag <= RT_I64) || a.tag == RT_CHAR || a.tag == RT_BOOL || a.tag == RT_BYTE || (a.tag >= RT_U8 && a.tag <= RT_U64))) {
        *out = a.v.i == b.v.i;
        return ST_OK;
    }
    if (a.tag == RT_ENUM && b.tag == RT_ENUM) {
        *out = a.x == b.x && a.v.i == b.v.i;
        return ST_OK;
    }
    if (a.tag == RT_NULL || b.tag == RT_NULL) { *out = a.tag == b.tag; return ST_OK; }
    if (a.tag == RT_BOOL && b.tag == RT_BOOL) { *out = a.v.i == b.v.i; return ST_OK; }
    if (a.tag == RT_CHAR && b.tag == RT_CHAR) { *out = a.v.u == b.v.u; return ST_OK; }
    if (a.tag == RT_BYTE && b.tag == RT_BYTE) { *out = a.v.u == b.v.u; return ST_OK; }
    if (a.tag == RT_STR && b.tag == RT_STR) {
        RStr* x = rt_s(a);
        RStr* y = rt_s(b);
        *out = x->len == y->len && memcmp(x->data, y->data, (size_t)x->len) == 0;
        return ST_OK;
    }
    if ((a.tag == RT_ENUM || a.tag == RT_ENUMD) && (b.tag == RT_ENUM || b.tag == RT_ENUMD)) {
        int32_t da = a.tag == RT_ENUM ? a.x : ((RAgg*)a.v.p)->def, db = b.tag == RT_ENUM ? b.x : ((RAgg*)b.v.p)->def;
        int64_t ia = a.tag == RT_ENUM ? a.v.i : ((RAgg*)a.v.p)->idx, ib = b.tag == RT_ENUM ? b.v.i : ((RAgg*)b.v.p)->idx;
        *out = da == db && ia == ib;
        return ST_OK;
    }
    if (a.tag == RT_OBJ && b.tag == RT_OBJ) {
        RObj* o = (RObj*)a.v.p;
        RObj* q = (RObj*)b.v.p;
        if (!o->cls || !o->cls->equals) { *out = o == q; return ST_OK; }
        rv args[1];
        args[0] = rv_view(b);
        rv r;
        int st = o->cls->equals(&r, rv_view(a), args);
        if (st) return st;
        *out = rv_truthy(r);
        return ST_OK;
    }
    if (has_f64(a) && has_f64(b)) {
        uint8_t p = common_numeric(a.tag, b.tag);
        if (!p) p = a.tag;
        if (is_float_tag(p)) *out = as_f64(a) == as_f64(b);
        else *out = w_cmp(w_of(a), w_of(b)) == 0;
        return ST_OK;
    }
    *out = 0;
    return ST_OK;
}

int rt_compare_slow(int* out, rv a, rv b) {
    if (a.tag == b.tag && a.tag >= RT_I8 && a.tag <= RT_I64) {
        *out = a.v.i < b.v.i ? -1 : a.v.i > b.v.i;
        return ST_OK;
    }
    if (a.tag == RT_CHAR && b.tag == RT_CHAR) { *out = a.v.u < b.v.u ? -1 : a.v.u > b.v.u; return ST_OK; }
    if (a.tag == RT_BYTE && b.tag == RT_BYTE) { *out = a.v.u < b.v.u ? -1 : a.v.u > b.v.u; return ST_OK; }
    if (a.tag == RT_STR && b.tag == RT_STR) {
        RStr* x = rt_s(a);
        RStr* y = rt_s(b);
        int64_t n = x->len < y->len ? x->len : y->len;
        int c = memcmp(x->data, y->data, (size_t)n);
        if (c == 0) c = x->len < y->len ? -1 : x->len > y->len;
        *out = c < 0 ? -1 : c > 0;
        return ST_OK;
    }
    if (a.tag == RT_OBJ) {
        RObj* o = (RObj*)a.v.p;
        if (!o->cls) return rt_internal("Vergleich von Vektoren");
        if (!o->cls->compare) return rt_internal("compareTo fehlt");
        rv args[1];
        args[0] = rv_view(b);
        rv r;
        int st = o->cls->compare(&r, rv_view(a), args);
        if (st) return st;
        W w = w_of(r);
        *out = w_neg_p(w) ? -1 : (w.lo != 0 || w.hi != 0);
        return ST_OK;
    }
    uint8_t pa = rt_is_num_tag(a.tag) ? a.tag : RT_I32, pb = rt_is_num_tag(b.tag) ? b.tag : RT_I32;
    uint8_t p = common_numeric(pa, pb);
    if (!p) p = pa;
    if (is_float_tag(p)) {
        double x = as_f64(a), y = as_f64(b);
        *out = x < y ? -1 : x > y ? 1 : 0;
    } else {
        *out = w_cmp(w_of(a), w_of(b));
    }
    return ST_OK;
}

static int int_range(uint8_t p, W* lo, W* hi) {
    switch (p) {
    case RT_I8: *lo = w_from_i(-128); *hi = w_from_i(127); return 1;
    case RT_I16: *lo = w_from_i(-32768); *hi = w_from_i(32767); return 1;
    case RT_I32: *lo = w_from_i(INT32_MIN); *hi = w_from_i(INT32_MAX); return 1;
    case RT_I64: *lo = w_from_i(INT64_MIN); *hi = w_from_i(INT64_MAX); return 1;
    case RT_U8: *lo = w_from_i(0); *hi = w_from_u(255); return 1;
    case RT_U16: *lo = w_from_i(0); *hi = w_from_u(65535); return 1;
    case RT_U32: *lo = w_from_i(0); *hi = w_from_u(0xffffffffu); return 1;
    case RT_U64: *lo = w_from_i(0); *hi = w_from_u(UINT64_MAX); return 1;
    default: return 0;
    }
}

rv rt_minmax(uint8_t tag, int max) {
    W lo, hi;
    int_range(tag, &lo, &hi);
    return make_int_w(max ? hi : lo, tag);
}

static int64_t variant_value(const REnumDef* d, int64_t i) { return d->values ? d->values[i] : i; }

int rt_cast_enum(rv* out, rv v, int32_t def) {
    const REnumDef* d = rt_enums[def];
    W n = rt_is_int_tag(v.tag) ? w_of(v) : w_from_i(-1);
    for (int i = 0; i < d->nvariants; i++) {
        if (w_cmp(w_from_i(variant_value(d, i)), n) == 0) {
            *out = rv_enum(def, i);
            return ST_OK;
        }
    }
    char t[64];
    w_text(n, t);
    return rt_fatal("CastError", "%s ist kein Wert von %s", t, d->name);
}

int rt_cast_prim(rv* out, rv v, uint8_t p) {
    if (v.tag == RT_ENUM || v.tag == RT_ENUMD) {
        int32_t def = v.tag == RT_ENUM ? v.x : ((RAgg*)v.v.p)->def;
        int64_t idx = v.tag == RT_ENUM ? v.v.i : ((RAgg*)v.v.p)->idx;
        return rt_cast_prim(out, rv_i64(variant_value(rt_enums[def], idx)), p);
    }
    if (v.tag == RT_CHAR) return rt_cast_prim(out, rv_uint(v.v.u, RT_U32), p);
    if (v.tag == RT_BYTE) { *out = make_int_w(w_from_u(v.v.u), p); return ST_OK; }
    if (p == RT_CHAR) {
        W n = rt_is_int_tag(v.tag) ? w_of(v) : w_from_i(-1);
        if (n.hi != 0 || n.lo > 0x10ffff || (n.lo >= 0xd800 && n.lo <= 0xdfff)) {
            char t[64];
            w_text(n, t);
            return rt_fatal("CastError", "%s ist kein Unicode-Zeichen", t);
        }
        *out = rv_char((uint32_t)n.lo);
        return ST_OK;
    }
    if (p == RT_BYTE) { *out = rv_byte((uint8_t)w_of(v).lo); return ST_OK; }
    if (p == RT_F32 || p == RT_F64) { *out = rt_num_to(v, p); return ST_OK; }
    W lo, hi;
    if (int_range(p, &lo, &hi)) {
        if (is_float_tag(v.tag)) {
            double x = as_f64(v);
            double t = trunc(x);
            if (x != x || t < w_to_f64(lo) || t > w_to_f64(hi) || (p == RT_I64 && t >= 9223372036854775808.0) || (p == RT_U64 && t >= 18446744073709551616.0)) {
                if (!rt_release) {
                    Buf b = { 0 };
                    float_text(&b, x, 0);
                    int st = rt_fatal("CastError", "%s passt nicht in %s", b.p, prim_name(p));
                    xfree(b.p);
                    return st;
                }
                *out = make_int_w(x != x ? w_from_i(0) : x < 0 ? lo : hi, p);
                return ST_OK;
            }
            W w;
            if (t < 0) w = w_from_i((int64_t)t);
            else w = w_from_u((uint64_t)t);
            *out = make_int_w(w, p);
            return ST_OK;
        }
        W n = w_of(v);
        if ((w_cmp(n, lo) < 0 || w_cmp(n, hi) > 0) && !rt_release) {
            char t[64];
            w_text(n, t);
            return rt_fatal("CastError", "%s passt nicht in %s", t, prim_name(p));
        }
        *out = make_int_w(n, p);
        return ST_OK;
    }
    *out = v;
    return ST_OK;
}

/* Literale */
rv rt_int_lit(const char* text, int negative, uint8_t tag) {
    W v = w_from_i(0);
    const char* s = text;
    int hex = s[0] == '0' && s[1] == 'x';
    if (hex) s += 2;
    for (; *s; s++) {
        if (*s == '_') continue;
        int d;
        if (*s >= '0' && *s <= '9') d = *s - '0';
        else if (*s >= 'a' && *s <= 'f') d = *s - 'a' + 10;
        else if (*s >= 'A' && *s <= 'F') d = *s - 'A' + 10;
        else break;
        v = w_add(w_mul(v, w_from_i(hex ? 16 : 10)), w_from_i(d));
    }
    if (negative) v = w_negate(v);
    if (!tag) tag = w_fits(v, RT_I32) ? RT_I32 : RT_I64;
    return make_int_w(v, tag);
}

rv rt_float_lit(const char* text, int negative, int f32) {
    char clean[128];
    int k = 0;
    for (const char* s = text; *s && k < 126; s++)
        if (*s != '_') clean[k++] = *s;
    clean[k] = 0;
    if (f32) {
        float x = strtof(clean, NULL);
        return rv_f32(negative ? -x : x);
    }
    double x = strtod(clean, NULL);
    return rv_f64(negative ? -x : x);
}

/* ================================================================ Ausgabe */

void rt_print(rv v) {
    Buf b = { 0 };
    text_of(&b, v);
    buf_str(&b, "\n");
    fwrite(b.p, 1, (size_t)b.n, stdout);
    xfree(b.p);
}

/* ================================================================ Eingebaute Methoden */

static int64_t argi(rv* a, int i) { return rt_is_int_tag(a[i].tag) ? (int64_t)w_of(a[i]).lo : 0; }

static int64_t str_find64(RStr* s, RStr* pat, int64_t from) {
    if (pat->len == 0) return from <= s->len ? from : -1;
    for (int64_t i = from; i + pat->len <= s->len; i++)
        if (memcmp(s->data + i, pat->data, (size_t)pat->len) == 0) return i;
    return -1;
}

static int64_t chars_before(RStr* s, int64_t byte) {
    int64_t n = 0;
    for (int64_t i = 0; i < byte; i++)
        if (((unsigned char)s->data[i] & 0xc0) != 0x80) n++;
    return n;
}

/* Schluss-Sigma (str::to_lowercase in Rust): Ist vor dem Sigma, über ignorierbare Zeichen
 * hinweg, ein Buchstabe mit Groß-/Kleinschreibung und danach keiner? */
static int sigma_final(RStr* s, int64_t at, int64_t after) {
    int64_t i = at;
    int before = 0;
    while (i > 0) {
        int64_t p = i - 1;
        while (p > 0 && ((unsigned char)s->data[p] & 0xc0) == 0x80) p--;
        uint32_t c;
        utf8_dec(s->data, s->len, p, &c);
        i = p;
        if (u_is_ignorable(c)) continue;
        before = u_is_cased(c);
        break;
    }
    if (!before) return 0;
    i = after;
    while (i < s->len) {
        uint32_t c;
        int k = utf8_dec(s->data, s->len, i, &c);
        i += k;
        if (u_is_ignorable(c)) continue;
        return !u_is_cased(c);
    }
    return 1;
}

static rv map_case(RStr* s, int upper) {
    Buf b = { 0 };
    buf_str(&b, "");
    int64_t i = 0;
    while (i < s->len) {
        uint32_t c;
        int64_t at = i;
        int k = utf8_dec(s->data, s->len, i, &c);
        i += k;
        char t[4];
        if (!upper && c == 0x3a3) {
            int n = utf8_enc(sigma_final(s, at, i) ? 0x3c2 : 0x3c3, t);
            buf_add(&b, t, n);
            continue;
        }
        const uint32_t* m = upper ? u_map(u_upmap, U_UPMAP_N, c) : u_map(u_lowmap, U_LOWMAP_N, c);
        if (!m) {
            buf_add(&b, s->data + at, k);
            continue;
        }
        for (int j = 0; j < 3 && m[j]; j++) {
            int n = utf8_enc(m[j], t);
            buf_add(&b, t, n);
        }
    }
    return buf_take(&b);
}

static int vec_method(rv* out, RObj* o, const char* name, int n, rv* args);

int rt_builtin_method(rv* out, rv recv, const char* name, int n, rv* args) {
    if (recv.tag == RT_STR) {
        RStr* st = rt_s(recv);
        if (!strcmp(name, "toString")) { *out = recv; return ST_OK; }
        if (!strcmp(name, "trim")) {
            int64_t a = 0, b = st->len;
            while (a < b) {
                uint32_t c;
                int k = utf8_dec(st->data, st->len, a, &c);
                if (!u_is_whitespace(c)) break;
                a += k;
            }
            while (b > a) {
                int64_t p = b - 1;
                while (p > a && ((unsigned char)st->data[p] & 0xc0) == 0x80) p--;
                uint32_t c;
                utf8_dec(st->data, st->len, p, &c);
                if (!u_is_whitespace(c)) break;
                b = p;
            }
            *out = rt_str_from(st->data + a, b - a);
            return ST_OK;
        }
        if (!strcmp(name, "toUpper")) { *out = map_case(st, 1); return ST_OK; }
        if (!strcmp(name, "toLower")) { *out = map_case(st, 0); return ST_OK; }
        RStr* a0 = n > 0 ? rt_s(args[0]) : NULL;
        if (!strcmp(name, "startsWith")) { *out = rv_bool(a0->len <= st->len && memcmp(st->data, a0->data, (size_t)a0->len) == 0); return ST_OK; }
        if (!strcmp(name, "endsWith")) { *out = rv_bool(a0->len <= st->len && memcmp(st->data + st->len - a0->len, a0->data, (size_t)a0->len) == 0); return ST_OK; }
        if (!strcmp(name, "contains")) { *out = rv_bool(str_find64(st, a0, 0) >= 0); return ST_OK; }
        if (!strcmp(name, "indexOf")) {
            int64_t p = str_find64(st, a0, 0);
            *out = p < 0 ? rv_null() : rv_i64(chars_before(st, p));
            return ST_OK;
        }
        if (!strcmp(name, "replace")) {
            RStr* to = rt_s(args[1]);
            Buf b = { 0 };
            if (a0->len == 0) {
                int64_t i = 0;
                buf_add(&b, to->data, to->len);
                while (i < st->len) {
                    uint32_t c;
                    int k = utf8_dec(st->data, st->len, i, &c);
                    buf_add(&b, st->data + i, k);
                    buf_add(&b, to->data, to->len);
                    i += k;
                }
            } else {
                int64_t i = 0;
                for (;;) {
                    int64_t p = str_find64(st, a0, i);
                    if (p < 0) break;
                    buf_add(&b, st->data + i, p - i);
                    buf_add(&b, to->data, to->len);
                    i = p + a0->len;
                }
                buf_add(&b, st->data + i, st->len - i);
            }
            *out = buf_take(&b);
            return ST_OK;
        }
        if (!strcmp(name, "substring")) {
            int64_t a = argi(args, 0), b = argi(args, 1), nc = rt_str_chars(st);
            if (w_neg_p(w_of(args[0]))) a = -1;
            if (a < 0 || b < a || b > nc) return rt_fatal("IndexError", "Bereich %lld..%lld außerhalb von 0..%lld", (long long)a, (long long)b, (long long)nc);
            int64_t pa = char_pos(st, a), pb = char_pos(st, b);
            *out = rt_str_from(st->data + pa, pb - pa);
            return ST_OK;
        }
        if (!strcmp(name, "split")) {
            RObj* v = rt_vec_new(4);
            if (a0->len == 0) {
                rt_vec_push(v, rt_str_c(""));
                int64_t i = 0;
                while (i < st->len) {
                    uint32_t c;
                    int k = utf8_dec(st->data, st->len, i, &c);
                    rt_vec_push(v, rt_str_from(st->data + i, k));
                    i += k;
                }
                rt_vec_push(v, rt_str_c(""));
            } else {
                int64_t i = 0;
                for (;;) {
                    int64_t p = str_find64(st, a0, i);
                    if (p < 0) break;
                    rt_vec_push(v, rt_str_from(st->data + i, p - i));
                    i = p + a0->len;
                }
                rt_vec_push(v, rt_str_from(st->data + i, st->len - i));
            }
            *out = rv_obj(v, F_OWN);
            return ST_OK;
        }
        if (!strcmp(name, "bytes")) {
            RObj* v = rt_vec_new(st->len);
            for (int64_t i = 0; i < st->len; i++) rt_vec_push(v, rv_byte((uint8_t)st->data[i]));
            *out = rv_obj(v, F_OWN);
            return ST_OK;
        }
        if (!strcmp(name, "byteAt")) {
            int64_t i = argi(args, 0);
            if (w_neg_p(w_of(args[0])) || i >= st->len) return rt_fatal("IndexError", "Index %lld außerhalb von 0..%lld", (long long)(w_neg_p(w_of(args[0])) ? (int64_t)w_of(args[0]).lo : i), (long long)st->len);
            *out = rv_byte((uint8_t)st->data[i]);
            return ST_OK;
        }
        return rt_fatal("InternalError", "string.%s", name);
    }
    if (recv.tag == RT_CHAR) {
        uint32_t c = (uint32_t)recv.v.u;
        if (!strcmp(name, "toString")) { *out = rt_text(recv); return ST_OK; }
        if (!strcmp(name, "isLetter")) { *out = rv_bool(u_is_alpha(c)); return ST_OK; }
        if (!strcmp(name, "isDigit")) { *out = rv_bool(c >= '0' && c <= '9'); return ST_OK; }
        if (!strcmp(name, "isWhitespace")) { *out = rv_bool(u_is_whitespace(c)); return ST_OK; }
        if (!strcmp(name, "isUpper")) { *out = rv_bool(u_is_upper(c)); return ST_OK; }
        if (!strcmp(name, "isLower")) { *out = rv_bool(u_is_lower(c)); return ST_OK; }
        if (!strcmp(name, "toUpper")) { *out = rv_char(u_to_upper(c)); return ST_OK; }
        if (!strcmp(name, "toLower")) { *out = rv_char(u_to_lower(c)); return ST_OK; }
        return rt_fatal("InternalError", "char.%s", name);
    }
    if (!strcmp(name, "partitionPoint") && (recv.tag == RT_OBJ || recv.tag == RT_SLICE)) {
        /* binäre Suche wie im Interpreter: erste Position, an der das Prädikat falsch ist */
        RObj* v = (RObj*)recv.v.p;
        int64_t start = recv.tag == RT_SLICE ? recv.x : 0, end = recv.tag == RT_SLICE ? recv.y : v->len;
        int64_t lo = start, hi = end;
        while (lo < hi) {
            int64_t mid = lo + (hi - lo) / 2;
            if (mid >= v->len) return rt_fatal("IndexError", "ungültiger Bereich bei partitionPoint");
            rv a1[1];
            a1[0] = rv_view(v->slots[mid]);
            rv r;
            int st = rt_call_value(&r, args[0], a1);
            if (st) return st;
            if (rv_truthy(r)) lo = mid + 1;
            else hi = mid;
        }
        *out = rv_i64(lo - start);
        return ST_OK;
    }
    if (recv.tag == RT_OBJ && !((RObj*)recv.v.p)->cls) return vec_method(out, (RObj*)recv.v.p, name, n, args);
    if (recv.tag == RT_SLICE) return rt_fatal("InternalError", "Methode %s auf einem Bereich", name);
    if (!strcmp(name, "toString")) { *out = rt_text(recv); return ST_OK; }
    if (!strcmp(name, "abs")) {
        if (recv.tag == RT_F32) { *out = rv_f32(fabsf(recv.v.f32)); return ST_OK; }
        if (recv.tag == RT_F64) { *out = rv_f64(fabs(recv.v.f)); return ST_OK; }
        uint8_t p = rt_is_int_tag(recv.tag) ? recv.tag : RT_I32;
        W w = w_of(recv);
        return int_result(out, w_neg_p(w) ? w_negate(w) : w, p);
    }
    if (!strcmp(name, "tryAdd") || !strcmp(name, "trySub") || !strcmp(name, "tryMul")) {
        W a = w_of(recv), b = w_of(args[0]);
        W result;
        if (!strcmp(name, "tryAdd")) result = w_add(a, b);
        else if (!strcmp(name, "trySub")) result = w_sub(a, b);
        else result = w_mul(a, b);
        *out = w_fits(result, recv.tag) ? make_int_w(result, recv.tag) : rv_null();
        return ST_OK;
    }
    if (!strcmp(name, "min") || !strcmp(name, "max")) {
        int c;
        int st = rt_compare(&c, recv, args[0]);
        if (st) return st;
        int take_recv = (!strcmp(name, "min")) == (c != 1);
        *out = take_recv ? recv : args[0];
        return ST_OK;
    }
    return rt_fatal("InternalError", "Methode %s", name);
}

static int vec_method(rv* out, RObj* o, const char* name, int n, rv* args) {
    (void)n;
    int64_t len = o->len;
    *out = rv_void();
    if (!strcmp(name, "push")) {
        rt_vec_push(o, args[0]);
        return ST_OK;
    }
    if (!strcmp(name, "insert")) {
        W w = w_of(args[0]);
        int64_t i = (int64_t)w.lo;
        if (w_neg_p(w) || i > len) return rt_fatal("IndexError", "Index %lld außerhalb von 0..%lld", (long long)i, (long long)(len + 1));
        vec_reserve(o, len + 1);
        memmove(o->slots + i + 1, o->slots + i, sizeof(rv) * (size_t)(len - i));
        o->slots[i] = args[1];
        o->len++;
        return ST_OK;
    }
    if (!strcmp(name, "pop")) {
        if (len == 0) return rt_fatal("IndexError", "pop auf einem leeren Vektor");
        *out = o->slots[--o->len];
        return ST_OK;
    }
    if (!strcmp(name, "remove")) {
        W w = w_of(args[0]);
        int64_t i = (int64_t)w.lo;
        if (w_neg_p(w) || i >= len) return rt_fatal("IndexError", "Index %lld außerhalb von 0..%lld", (long long)i, (long long)len);
        *out = o->slots[i];
        memmove(o->slots + i, o->slots + i + 1, sizeof(rv) * (size_t)(len - i - 1));
        o->len--;
        return ST_OK;
    }
    if (!strcmp(name, "clear")) {
        rv* elems = o->slots;
        int64_t m = o->len;
        o->slots = (rv*)xmalloc(sizeof(rv) * 4);
        o->len = 0;
        o->cap = 4;
        for (int64_t i = 0; i < m; i++) {
            int st = rt_drop(elems[i]);
            if (st) { xfree(elems); return st; }
        }
        xfree(elems);
        return ST_OK;
    }
    if (!strcmp(name, "first")) { *out = len ? rv_view(o->slots[0]) : rv_null(); return ST_OK; }
    if (!strcmp(name, "last")) { *out = len ? rv_view(o->slots[len - 1]) : rv_null(); return ST_OK; }
    if (!strcmp(name, "contains") || !strcmp(name, "indexOf")) {
        int c = !strcmp(name, "contains");
        for (int64_t i = 0; i < o->len; i++) {
            int eq;
            int st = rt_equal(&eq, rv_view(o->slots[i]), args[0]);
            if (st) return st;
            if (eq) { *out = c ? rv_bool(1) : rv_i64(i); return ST_OK; }
        }
        *out = c ? rv_bool(0) : rv_null();
        return ST_OK;
    }
    if (!strcmp(name, "sort") || !strcmp(name, "sortBy")) {
        /* stabiles Einfügesortieren wie im Interpreter (dieselben Vergleiche) */
        int by = !strcmp(name, "sortBy");
        for (int64_t i = 1; i < o->len; i++) {
            int64_t j = i;
            while (j > 0) {
                int ord;
                if (by) {
                    rv a2[2];
                    a2[0] = rv_view(o->slots[j - 1]);
                    a2[1] = rv_view(o->slots[j]);
                    rv r;
                    int st = rt_call_value(&r, args[0], a2);
                    if (st) return st;
                    W w = w_of(r);
                    ord = w_neg_p(w) ? -1 : (w.lo || w.hi) ? 1 : 0;
                } else {
                    int st = rt_compare(&ord, rv_view(o->slots[j - 1]), rv_view(o->slots[j]));
                    if (st) return st;
                }
                if (ord == 1) {
                    rv t = o->slots[j - 1];
                    o->slots[j - 1] = o->slots[j];
                    o->slots[j] = t;
                    j--;
                } else {
                    break;
                }
            }
        }
        return ST_OK;
    }
    return rt_fatal("InternalError", "Vektor.%s", name);
}

/* Zahl aus Text wie str::parse in Rust */
static int parse_int(const char* s, int64_t n, W* out) {
    int64_t i = 0;
    int neg = 0;
    if (i < n && (s[i] == '+' || s[i] == '-')) { neg = s[i] == '-'; i++; }
    if (i >= n) return 0;
    W v = w_from_i(0);
    for (; i < n; i++) {
        if (s[i] < '0' || s[i] > '9') return 0;
        v = w_add(w_mul(v, w_from_i(10)), w_from_i(s[i] - '0'));
        if (v.hi > 0) return 0;
    }
    *out = neg ? w_negate(v) : v;
    return 1;
}

static int parse_float(const char* s, int64_t n, double* out) {
    char tmp[512];
    if (n <= 0 || n >= (int64_t)sizeof tmp) return 0;
    memcpy(tmp, s, (size_t)n);
    tmp[n] = 0;
    const char* p = tmp;
    int neg = 0;
    if (*p == '+' || *p == '-') { neg = *p == '-'; p++; }
    if (!strcmp(p, "inf") || !strcmp(p, "infinity") || !strcmp(p, "Infinity") || !strcmp(p, "INF")) { *out = neg ? -HUGE_VAL : HUGE_VAL; return 1; }
    if (!strcmp(p, "NaN") || !strcmp(p, "nan")) { *out = NAN; return 1; }
    /* Ziffern [. Ziffern] [e [+-] Ziffern] */
    const char* q = p;
    int digits = 0;
    while (*q >= '0' && *q <= '9') { q++; digits++; }
    if (*q == '.') {
        q++;
        while (*q >= '0' && *q <= '9') { q++; digits++; }
    }
    if (!digits) return 0;
    if (*q == 'e' || *q == 'E') {
        q++;
        if (*q == '+' || *q == '-') q++;
        if (!(*q >= '0' && *q <= '9')) return 0;
        while (*q >= '0' && *q <= '9') q++;
    }
    if (*q) return 0;
    *out = strtod(tmp, NULL);
    return 1;
}

int rt_builtin_static(rv* out, uint8_t p, const char* name, int n, rv* args) {
    (void)n;
    if (!strcmp(name, "parse") || !strcmp(name, "tryParse")) {
        RStr* s = rt_s(args[0]);
        /* trim */
        int64_t a = 0, b = s->len;
        while (a < b) {
            uint32_t c;
            int k = utf8_dec(s->data, s->len, a, &c);
            if (!u_is_whitespace(c)) break;
            a += k;
        }
        while (b > a) {
            int64_t q = b - 1;
            while (q > a && ((unsigned char)s->data[q] & 0xc0) == 0x80) q--;
            uint32_t c;
            utf8_dec(s->data, s->len, q, &c);
            if (!u_is_whitespace(c)) break;
            b = q;
        }
        int ok;
        if (rt_is_int_tag(p)) {
            W w;
            ok = parse_int(s->data + a, b - a, &w) && w_fits(w, p);
            if (ok) *out = make_int_w(w, p);
        } else {
            double x;
            ok = parse_float(s->data + a, b - a, &x);
            if (ok) *out = rt_num_to(rv_f64(x), p);
        }
        if (ok) return ST_OK;
        if (!strcmp(name, "tryParse")) { *out = rv_null(); return ST_OK; }
        Buf m = { 0 };
        buf_str(&m, "keine Zahl: ");
        buf_add(&m, s->data, s->len);
        int st = rt_throw_std("rpp.core.FormatException", m.p);
        xfree(m.p);
        return st;
    }
    if (!strcmp(name, "join")) {
        RStr* sep = rt_s(args[0]);
        Buf b = { 0 };
        buf_str(&b, "");
        rv v = args[1];
        if (v.tag == RT_OBJ || v.tag == RT_SLICE) {
            RObj* o = (RObj*)v.v.p;
            int64_t from = v.tag == RT_SLICE ? v.x : 0, to = v.tag == RT_SLICE ? v.y : o->len;
            for (int64_t i = from; i < to; i++) {
                if (i > from) buf_add(&b, sep->data, sep->len);
                text_of(&b, o->slots[i]);
            }
        }
        *out = buf_take(&b);
        return ST_OK;
    }
    if (!strcmp(name, "fromUtf8")) {
        Buf b = { 0 };
        buf_str(&b, "");
        rv v = args[0];
        if (v.tag == RT_OBJ) {
            RObj* o = (RObj*)v.v.p;
            for (int64_t i = 0; i < o->len; i++) {
                char c = (char)(uint8_t)o->slots[i].v.u;
                buf_add(&b, &c, 1);
            }
        }
        if (!utf8_valid((const unsigned char*)b.p, b.n)) {
            xfree(b.p);
            return rt_throw_std("rpp.core.FormatException", "kein gültiges UTF-8");
        }
        *out = buf_take(&b);
        return ST_OK;
    }
    return rt_fatal("InternalError", "%s::%s", prim_name(p), name);
}

int rt_hash(int64_t* out, rv v) {
    if (rt_is_prim(v)) {
        switch (v.tag) {
        case RT_STR: {
            RStr* s = rt_s(v);
            uint64_t h = 0xcbf29ce484222325ull;
            for (int64_t i = 0; i < s->len; i++) {
                h ^= (uint8_t)s->data[i];
                h *= 0x100000001b3ull;
            }
            *out = (int64_t)h;
            return ST_OK;
        }
        case RT_BOOL: case RT_CHAR: case RT_BYTE: *out = (int64_t)v.v.u; return ST_OK;
        case RT_F32: { uint32_t bits; memcpy(&bits, &v.v.f32, 4); *out = (int64_t)bits; return ST_OK; }
        case RT_F64: { uint64_t bits; memcpy(&bits, &v.v.f, 8); *out = (int64_t)bits; return ST_OK; }
        default: *out = (int64_t)w_of(v).lo; return ST_OK;
        }
    }
    if (v.tag == RT_OBJ) {
        RObj* o = (RObj*)v.v.p;
        if (o->cls && o->cls->hash) {
            rv r;
            int st = o->cls->hash(&r, rv_view(v), NULL);
            if (st) return st;
            *out = (int64_t)w_of(r).lo;
            return ST_OK;
        }
    }
    *out = 0;
    return ST_OK;
}

int rt_prim_mixin(rv* out, const char* name, rv v, rv* args, int* handled) {
    *handled = 1;
    if (!strcmp(name, "hash")) {
        int64_t h;
        int st = rt_hash(&h, v);
        if (st) return st;
        *out = rv_i64(h);
        return ST_OK;
    }
    if (!strcmp(name, "equals")) {
        int eq;
        int st = rt_equal(&eq, v, args[0]);
        if (st) return st;
        *out = rv_bool(eq);
        return ST_OK;
    }
    if (!strcmp(name, "compareTo") || !strcmp(name, "less")) {
        int c;
        int st = rt_compare(&c, v, args[0]);
        if (st) return st;
        *out = !strcmp(name, "less") ? rv_bool(c < 0) : rv_i32(c);
        return ST_OK;
    }
    *handled = 0;
    return ST_OK;
}

/* ================================================================ Standardbibliothek */

typedef struct SB {
    Buf b;
    int64_t chars;
} SB;

typedef struct HIdx {
    int64_t nb;         /* Anzahl der Fächer */
    int64_t* head;      /* je Fach: erste Position oder -1 */
    int64_t* next;      /* je Position: nächste im Fach */
    int64_t* hashes;
    int64_t cap;
} HIdx;

static int native_drop(RObj* o) {
    (void)o;
    return ST_OK;
}

static HIdx* hidx(RObj* o) {
    if (!o->native) {
        HIdx* h = (HIdx*)xmalloc(sizeof(HIdx));
        memset(h, 0, sizeof *h);
        o->native = h;
    }
    return (HIdx*)o->native;
}

static rv key_at(RObj* o, int64_t i, int map) {
    rv it = rv_view(o->slots[i]);
    if (map) {
        RObj* e = (RObj*)it.v.p;
        return rv_view(e->slots[0]);
    }
    return it;
}

static void hidx_rebuild_buckets(HIdx* h, int64_t n) {
    int64_t nb = 16;
    while (nb < n * 2) nb *= 2;
    xfree(h->head);
    h->head = (int64_t*)xmalloc(sizeof(int64_t) * (size_t)nb);
    for (int64_t i = 0; i < nb; i++) h->head[i] = -1;
    h->nb = nb;
    for (int64_t i = 0; i < n; i++) {
        int64_t b = (int64_t)((uint64_t)h->hashes[i] & (uint64_t)(nb - 1));
        h->next[i] = h->head[b];
        h->head[b] = i;
    }
}

static void hidx_add(RObj* o, int64_t hash, int64_t pos) {
    HIdx* h = hidx(o);
    if (pos >= h->cap) {
        int64_t c = h->cap ? h->cap * 2 : 16;
        while (c <= pos) c *= 2;
        h->next = (int64_t*)xrealloc(h->next, sizeof(int64_t) * (size_t)c);
        h->hashes = (int64_t*)xrealloc(h->hashes, sizeof(int64_t) * (size_t)c);
        h->cap = c;
    }
    h->hashes[pos] = hash;
    if (h->nb == 0 || pos + 1 > h->nb / 2) {
        hidx_rebuild_buckets(h, pos + 1);
        return;
    }
    int64_t b = (int64_t)((uint64_t)hash & (uint64_t)(h->nb - 1));
    h->next[pos] = h->head[b];
    h->head[b] = pos;
}

static int reindex(RObj* o, int map) {
    HIdx* h = hidx(o);
    if (o->len > h->cap) {
        int64_t c = 16;
        while (c < o->len) c *= 2;
        h->next = (int64_t*)xrealloc(h->next, sizeof(int64_t) * (size_t)c);
        h->hashes = (int64_t*)xrealloc(h->hashes, sizeof(int64_t) * (size_t)c);
        h->cap = c;
    }
    for (int64_t i = 0; i < o->len; i++) {
        int64_t hv;
        int st = rt_hash(&hv, key_at(o, i, map));
        if (st) return st;
        h->hashes[i] = hv;
    }
    hidx_rebuild_buckets(h, o->len);
    return ST_OK;
}

/* Position des Schlüssels; Kandidaten in Einfügereihenfolge wie im Interpreter */
static int find_key(int64_t* pos, RObj* o, rv key, int map) {
    int64_t hv;
    int st = rt_hash(&hv, key);
    if (st) return st;
    *pos = -1;
    HIdx* h = hidx(o);
    if (h->nb == 0) return ST_OK;
    int64_t b = (int64_t)((uint64_t)hv & (uint64_t)(h->nb - 1));
    /* Kette ist absteigend nach Position; der Interpreter prüft aufsteigend */
    int64_t cands[64];
    int64_t nc = 0;
    int64_t* many = NULL;
    for (int64_t i = h->head[b]; i >= 0; i = h->next[i]) {
        if (h->hashes[i] != hv) continue;
        if (nc < 64) cands[nc] = i;
        else {
            if (nc == 64) {
                many = (int64_t*)xmalloc(sizeof(int64_t) * 1024);
                memcpy(many, cands, sizeof cands);
            }
            if ((nc & 1023) == 0 && nc >= 1024) many = (int64_t*)xrealloc(many, sizeof(int64_t) * (size_t)(nc + 1024));
            many[nc] = i;
        }
        nc++;
    }
    int64_t* list = many ? many : cands;
    for (int64_t k = nc - 1; k >= 0; k--) {
        int eq;
        st = rt_equal(&eq, key_at(o, list[k], map), key);
        if (st) { xfree(many); return st; }
        if (eq) { *pos = list[k]; break; }
    }
    xfree(many);
    return ST_OK;
}

static const char* slash_dup(const char* p) {
    char* s = (char*)xmalloc(strlen(p) + 1);
    strcpy(s, p);
    for (char* q = s; *q; q++)
        if (*q == '\\') *q = '/';
    return s;
}

/* Pfade sind UTF-8; Windows braucht für Nicht-ASCII-Namen die Fassungen mit wchar_t */
#ifdef _WIN32
static wchar_t* u8w(const char* s) {
    int n = MultiByteToWideChar(CP_UTF8, 0, s, -1, NULL, 0);
    wchar_t* w = (wchar_t*)xmalloc(sizeof(wchar_t) * (size_t)(n > 0 ? n : 1));
    if (n <= 0 || !MultiByteToWideChar(CP_UTF8, 0, s, -1, w, n)) w[0] = 0;
    return w;
}

static char* w2u8(const wchar_t* w) {
    int n = WideCharToMultiByte(CP_UTF8, 0, w, -1, NULL, 0, NULL, NULL);
    char* s = (char*)xmalloc((size_t)(n > 0 ? n : 1));
    if (n <= 0 || !WideCharToMultiByte(CP_UTF8, 0, w, -1, s, n, NULL, NULL)) s[0] = 0;
    return s;
}

static FILE* u_fopen(const char* path, const char* mode) {
    wchar_t* wp = u8w(path);
    wchar_t* wm = u8w(mode);
    FILE* f = _wfopen(wp, wm);
    xfree(wp);
    xfree(wm);
    return f;
}

static int u_stat64(const char* path, struct _stat64* st) {
    wchar_t* wp = u8w(path);
    int r = _wstat64(wp, st);
    xfree(wp);
    return r;
}

static int u_rename(const char* from, const char* to) {
    wchar_t* a = u8w(from);
    wchar_t* b = u8w(to);
    int r = _wrename(a, b);
    xfree(a);
    xfree(b);
    return r;
}

static int u_remove(const char* path) {
    wchar_t* wp = u8w(path);
    int r = _wremove(wp);
    xfree(wp);
    return r;
}

static int u_mkdir(const char* path) {
    wchar_t* wp = u8w(path);
    int r = _wmkdir(wp);
    xfree(wp);
    return r;
}
#else
#define u_fopen fopen
#define u_rename rename
#define u_remove remove
#endif

static rv sv(const char* s) { return rt_str_c(s); }

static int file_mode(const char* p) {
#ifdef _WIN32
    struct _stat64 st;
    return u_stat64(p, &st) == 0 ? (int)(st.st_mode & S_IFMT) : -1;
#else
    struct stat st;
    return stat(p, &st) == 0 ? (int)(st.st_mode & S_IFMT) : -1;
#endif
}

static int is_file(const char* p) { return file_mode(p) == S_IFREG; }

static int is_dir(const char* p) { return file_mode(p) == S_IFDIR; }

static const char* sys_err(void) {
    static char buf[256];
    snprintf(buf, sizeof buf, "%s (os error %d)", strerror(errno), errno);
    return buf;
}

static int cmp_str(const void* a, const void* b) {
    return strcmp(*(const char* const*)a, *(const char* const*)b);
}

static int make_dirs(const char* path) {
    char tmp[4096];
    snprintf(tmp, sizeof tmp, "%s", path);
    size_t n = strlen(tmp);
    for (size_t i = 1; i <= n; i++) {
        if (tmp[i] == '/' || tmp[i] == '\\' || tmp[i] == 0) {
            char c = tmp[i];
            tmp[i] = 0;
            if (!is_dir(tmp) && strlen(tmp) > 0 && !(strlen(tmp) == 2 && tmp[1] == ':')) {
#ifdef _WIN32
                if (u_mkdir(tmp) != 0 && errno != EEXIST) return -1;
#else
                if (mkdir(tmp, 0777) != 0 && errno != EEXIST) return -1;
#endif
            }
            tmp[i] = c;
        }
    }
    return 0;
}

static char* in_buf = NULL;
static int64_t in_len = -1, in_pos = 0;

static void read_input(void) {
    if (in_len >= 0) return;
    Buf b = { 0 };
    char tmp[4096];
    size_t n;
    while ((n = fread(tmp, 1, sizeof tmp, stdin)) > 0) buf_add(&b, tmp, (int64_t)n);
    in_buf = b.p ? b.p : (char*)"";
    in_len = b.n;
}

static void normalize_path(const char* in, Buf* out) {
    char abs[4096];
#ifdef _WIN32
    {
        wchar_t* win = u8w(in);
        wchar_t wabs[4096];
        if (_wfullpath(wabs, win, 4096)) {
            char* u = w2u8(wabs);
            snprintf(abs, sizeof abs, "%s", u);
            xfree(u);
        } else {
            snprintf(abs, sizeof abs, "%s", in);
        }
        xfree(win);
    }
#else
    if (in[0] == '/') snprintf(abs, sizeof abs, "%s", in);
    else {
        char cwd[4096];
        if (!getcwd(cwd, sizeof cwd)) cwd[0] = 0;
        snprintf(abs, sizeof abs, "%s/%s", cwd, in);
    }
#endif
    for (char* q = abs; *q; q++)
        if (*q == '\\') *q = '/';
    /* Bestandteile */
    const char* parts[512];
    int lens[512];
    int np = 0;
    const char* p = abs;
    Buf prefix = { 0 };
    if (p[0] && p[1] == ':') { buf_add(&prefix, p, 2); p += 2; }
    int rooted = *p == '/';
    while (*p) {
        while (*p == '/') p++;
        if (!*p) break;
        const char* s = p;
        while (*p && *p != '/') p++;
        int l = (int)(p - s);
        if (l == 1 && s[0] == '.') continue;
        if (l == 2 && s[0] == '.' && s[1] == '.') {
            if (np > 0) np--;
            continue;
        }
        if (np < 512) { parts[np] = s; lens[np] = l; np++; }
    }
    buf_str(out, "");
    if (prefix.p) buf_add(out, prefix.p, prefix.n);
    if (rooted) buf_str(out, "/");
    for (int i = 0; i < np; i++) {
        if (i > 0) buf_str(out, "/");
        buf_add(out, parts[i], lens[i]);
    }
    xfree(prefix.p);
}



/* ---------------------------------------------------------------- Ausbau der Bibliothek (wie stdlib.rs) */

/* Pfad rein lexikalisch zerlegt (lexical_path in project.rs): Präfix (Laufwerk), Wurzel, Teile */
typedef struct {
    char prefix[8];
    int rooted;
    int n;
    char* parts[512];
} LexPath;

static void lex_path(const char* in, LexPath* lp) {
    memset(lp, 0, sizeof *lp);
    const char* p = in;
#ifdef _WIN32
    if (((p[0] >= 'A' && p[0] <= 'Z') || (p[0] >= 'a' && p[0] <= 'z')) && p[1] == ':') {
        lp->prefix[0] = p[0];
        lp->prefix[1] = ':';
        p += 2;
    }
#endif
    if (*p == '/' || *p == '\\') lp->rooted = 1;
    while (*p) {
        while (*p == '/' || *p == '\\') p++;
        if (!*p) break;
        const char* s = p;
        while (*p && *p != '/' && *p != '\\') p++;
        int64_t l = p - s;
        if (l == 1 && s[0] == '.') continue;
        if (l == 2 && s[0] == '.' && s[1] == '.') {
            if (lp->n > 0 && strcmp(lp->parts[lp->n - 1], "..") != 0) {
                xfree(lp->parts[--lp->n]);
                continue;
            }
            if (lp->rooted) continue;
        }
        if (lp->n < 512) {
            char* c = (char*)xmalloc((size_t)l + 1);
            memcpy(c, s, (size_t)l);
            c[l] = 0;
            lp->parts[lp->n++] = c;
        }
    }
}

static void lex_free(LexPath* lp) {
    for (int i = 0; i < lp->n; i++) xfree(lp->parts[i]);
    lp->n = 0;
}

/* Text ab Teil from; mit Präfix und Wurzel nur für from == 0 (sonst ein Rest innerhalb der Basis) */
static rv lex_text(LexPath* lp, int from) {
    Buf b = { 0 };
    buf_str(&b, "");
    if (from == 0) {
        buf_str(&b, lp->prefix);
        if (lp->rooted) buf_str(&b, "/");
    }
    for (int i = from; i < lp->n; i++) {
        if (i > from) buf_str(&b, "/");
        buf_str(&b, lp->parts[i]);
    }
    if (b.n == 0) buf_str(&b, ".");
    return buf_take(&b);
}

static int path_io_error(const char* path) {
    Buf m = { 0 };
    buf_fmt(&m, "%s: %s", path, sys_err());
    int st = io_error(m.p, path);
    xfree(m.p);
    return st;
}

/* 0 File, 1 Directory, 2 Symlink, 3 Other (Reihenfolge wie enum FileType) */
static int file_kind(const char* path) {
#ifdef _WIN32
    wchar_t* wp = u8w(path);
    DWORD a = GetFileAttributesW(wp);
    if (a == INVALID_FILE_ATTRIBUTES) { xfree(wp); return 3; }
    if (a & FILE_ATTRIBUTE_REPARSE_POINT) {
        WIN32_FIND_DATAW fd;
        HANDLE h = FindFirstFileW(wp, &fd);
        if (h != INVALID_HANDLE_VALUE) {
            FindClose(h);
            if (fd.dwReserved0 == IO_REPARSE_TAG_SYMLINK || fd.dwReserved0 == IO_REPARSE_TAG_MOUNT_POINT) { xfree(wp); return 2; }
        }
    }
    xfree(wp);
    return (a & FILE_ATTRIBUTE_DIRECTORY) ? 1 : 0;
#else
    struct stat st;
    if (lstat(path, &st) != 0) return 3;
    if (S_ISLNK(st.st_mode)) return 2;
    if (S_ISDIR(st.st_mode)) return 1;
    if (S_ISREG(st.st_mode)) return 0;
    return 3;
#endif
}

static int64_t mono_nanos(void) {
#ifdef _WIN32
    static LARGE_INTEGER freq, origin;
    LARGE_INTEGER now;
    if (!freq.QuadPart) { QueryPerformanceFrequency(&freq); QueryPerformanceCounter(&origin); }
    QueryPerformanceCounter(&now);
    int64_t t = now.QuadPart - origin.QuadPart;
    return (int64_t)((double)t * 1e9 / (double)freq.QuadPart);
#else
    static int64_t origin = -1;
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    int64_t now = (int64_t)ts.tv_sec * 1000000000LL + ts.tv_nsec;
    if (origin < 0) origin = now;
    return now - origin;
#endif
}

/* Zustand eines SaveFile: temporäre Datei neben dem Ziel, beim commit umbenannt */
typedef struct {
    FILE* f;
    char target[4096];
    char temp[4096];
    int done;
} SaveState;

/* neue Datei, die es noch nicht gibt (wie create_new) */
static FILE* create_new(const char* path) {
#ifdef _WIN32
    wchar_t* wp = u8w(path);
    int fd = _wopen(wp, _O_WRONLY | _O_CREAT | _O_EXCL | _O_BINARY, _S_IREAD | _S_IWRITE);
    xfree(wp);
    return fd < 0 ? NULL : _fdopen(fd, "wb");
#else
    int fd = open(path, O_WRONLY | O_CREAT | O_EXCL, 0600);
    return fd < 0 ? NULL : fdopen(fd, "wb");
#endif
}

static void save_cancel(SaveState* s) {
    if (s->done) return;
    if (s->f) fclose(s->f);
    s->f = NULL;
    u_remove(s->temp);
    s->done = 1;
}

/* rename, das ein vorhandenes Ziel ersetzt (auf Windows schlägt rename sonst fehl) */
static int replace_file(const char* from, const char* to) {
    if (u_rename(from, to) == 0) return 0;
    if (is_file(to)) {
        if (u_remove(to) != 0) return -1;
        return u_rename(from, to);
    }
    return -1;
}

/* StringView: Felder source, byteStart, byteEnd */
static rv view_new(const RClass* cls, rv source, int64_t a, int64_t b) {
    RObj* v = rt_obj_new(cls, NULL, 0);
    v->slots[0] = rt_promote(rv_view(source));
    v->slots[1] = rv_i64(a);
    v->slots[2] = rv_i64(b);
    return rv_obj(v, F_OWN);
}

static int64_t bytes_find(const char* s, int64_t n, const char* pat, int64_t pn, int64_t from) {
    if (pn == 0) return from <= n ? from : -1;
    for (int64_t i = from; i + pn <= n; i++)
        if (memcmp(s + i, pat, (size_t)pn) == 0) return i;
    return -1;
}

static int64_t utf8_count(const char* s, int64_t n) {
    int64_t c = 0;
    for (int64_t i = 0; i < n; i++)
        if (((unsigned char)s[i] & 0xc0) != 0x80) c++;
    return c;
}

/* Byteposition des k-ten Zeichens (k == Anzahl: Ende); -1, wenn außerhalb */
static int64_t utf8_boundary(const char* s, int64_t n, int64_t k) {
    int64_t i = 0;
    for (int64_t j = 0; j < k; j++) {
        if (i >= n) return -1;
        uint32_t c;
        i += utf8_dec(s, n, i, &c);
    }
    return i;
}

/* Bytelänge des UTF-8-Zeichens, das vor Position i endet */
static int64_t utf8_back(const char* s, int64_t i) {
    int64_t j = i - 1;
    while (j > 0 && ((unsigned char)s[j] & 0xc0) == 0x80) j--;
    return i - j;
}

static int string_view(rv* out, RObj* o, const char* m, rv* args) {
    rv source = o->slots[0];
    RStr* src = rt_s(source);
    int64_t start = (int64_t)w_of(o->slots[1]).lo, end = (int64_t)w_of(o->slots[2]).lo;
    const char* t = src->data + start;
    int64_t tn = end - start;
    if (!strcmp(m, "length")) { *out = rv_i64(utf8_count(t, tn)); return ST_OK; }
    if (!strcmp(m, "charAt")) {
        int64_t i = w_neg_p(w_of(args[0])) ? -1 : utf8_boundary(t, tn, argi(args, 0));
        if (i < 0 || i >= tn) return rt_fatal("IndexError", "Index außerhalb der StringView");
        uint32_t c;
        utf8_dec(t, tn, i, &c);
        *out = rv_char(c);
        return ST_OK;
    }
    if (!strcmp(m, "slice")) {
        int64_t a = w_neg_p(w_of(args[0])) ? -1 : utf8_boundary(t, tn, argi(args, 0));
        int64_t b = w_neg_p(w_of(args[1])) ? -1 : utf8_boundary(t, tn, argi(args, 1));
        if (a < 0 || b < 0 || a > b) return rt_fatal("IndexError", "Bereich außerhalb der StringView");
        *out = view_new(o->cls, source, start + a, start + b);
        return ST_OK;
    }
    if (!strcmp(m, "trim") || !strcmp(m, "trimEnd")) {
        int64_t a = 0, b = tn;
        if (!strcmp(m, "trim")) {
            while (a < b) {
                uint32_t c;
                int l = utf8_dec(t, tn, a, &c);
                if (!u_is_whitespace(c)) break;
                a += l;
            }
        }
        while (b > a) {
            int64_t l = utf8_back(t, b);
            uint32_t c;
            utf8_dec(t, tn, b - l, &c);
            if (!u_is_whitespace(c)) break;
            b -= l;
        }
        *out = view_new(o->cls, source, start + a, start + b);
        return ST_OK;
    }
    if (!strcmp(m, "toString")) {
        *out = (start == 0 && end == src->len) ? rt_promote(rv_view(source)) : rt_str_from(t, tn);
        return ST_OK;
    }
    RStr* part = rt_s(args[0]);
    if (!strcmp(m, "startsWith")) { *out = rv_bool(part->len <= tn && memcmp(t, part->data, (size_t)part->len) == 0); return ST_OK; }
    if (!strcmp(m, "endsWith")) { *out = rv_bool(part->len <= tn && memcmp(t + tn - part->len, part->data, (size_t)part->len) == 0); return ST_OK; }
    if (!strcmp(m, "indexOf")) {
        int64_t at = bytes_find(t, tn, part->data, part->len, 0);
        *out = at < 0 ? rv_null() : rv_i64(utf8_count(t, at));
        return ST_OK;
    }
    if (!strcmp(m, "splitOnce")) {
        int64_t at = bytes_find(t, tn, part->data, part->len, 0);
        if (at < 0) { *out = rv_null(); return ST_OK; }
        rv items[2];
        items[0] = view_new(o->cls, source, start, start + at);
        items[1] = view_new(o->cls, source, start + at + part->len, end);
        *out = rt_tuple(2, items);
        return ST_OK;
    }
    if (!strcmp(m, "split")) {
        RObj* v = rt_vec_new(4);
        int64_t begin = 0, from = 0;
        for (;;) {
            int64_t at = bytes_find(t, tn, part->data, part->len, from);
            if (at < 0) break;
            rt_vec_push(v, view_new(o->cls, source, start + begin, start + at));
            begin = at + part->len;
            if (part->len == 0) {
                /* leerer Trenner: ein Treffer an jeder Zeichengrenze (wie match_indices) */
                if (at >= tn) break;
                uint32_t c;
                from = at + utf8_dec(t, tn, at, &c);
            } else {
                from = begin;
            }
        }
        rt_vec_push(v, view_new(o->cls, source, start + begin, end));
        *out = rv_obj(v, F_OWN);
        return ST_OK;
    }
    return rt_fatal("InternalError", "StringView.%s", m);
}

int rt_native(rv* out, const char* name, rv self, int n, rv* args, int mut_this, int at_cursor) {
    (void)n;
    (void)mut_this;
    *out = rv_void();
    RObj* o = self.tag == RT_OBJ ? (RObj*)self.v.p : NULL;
#define IS(x) (!strcmp(name, x))
    if (IS("rpp.io.Console.write")) {
        Buf b = { 0 };
        text_of(&b, args[0]);
        if (b.n) fwrite(b.p, 1, (size_t)b.n, stdout);
        xfree(b.p);
        return ST_OK;
    }
    if (IS("rpp.io.Console.error")) {
        Buf b = { 0 };
        text_of(&b, args[0]);
        buf_str(&b, "\n");
        fwrite(b.p, 1, (size_t)b.n, stderr);
        xfree(b.p);
        return ST_OK;
    }
    if (IS("rpp.io.Console.readLine")) {
        read_input();
        if (in_pos >= in_len) { *out = rv_null(); return ST_OK; }
        int64_t e = in_pos;
        while (e < in_len && in_buf[e] != '\n') e++;
        int64_t used = e < in_len ? e - in_pos + 1 : e - in_pos;
        int64_t l = e - in_pos;
        if (l > 0 && in_buf[in_pos + l - 1] == '\r') l--;
        *out = rt_str_from(in_buf + in_pos, l);
        in_pos += used;
        return ST_OK;
    }
    if (IS("rpp.io.File.openTextReader")) {
        const char* path = rt_s(args[0])->data;
        if (is_dir(path)) { errno = EISDIR; return path_io_error(path); }
        FILE* file = u_fopen(path, "rb");
        if (!file) return path_io_error(path);
        RObj* reader = rt_obj_new(rt_cls_text_reader, NULL, 0);
        reader->native = file;
        rt_vec_push(reader, rt_promote(rv_view(args[0])));
        *out = rv_obj(reader, F_OWN);
        return ST_OK;
    }
    if (IS("rpp.io.TextReader.close")) {
        if (o->native) fclose((FILE*)o->native);
        o->native = NULL;
        return ST_OK;
    }
    if (IS("rpp.io.TextReader.readLine")) {
        FILE* file = (FILE*)o->native;
        Buf line = { 0 };
        int c;
        while ((c = fgetc(file)) != EOF && c != '\n') {
            char byte = (char)c;
            buf_add(&line, &byte, 1);
        }
        if (c == EOF && ferror(file)) {
            xfree(line.p);
            return path_io_error(rt_s(o->slots[0])->data);
        }
        if (c == EOF && !line.n) { *out = rv_null(); return ST_OK; }
        if (c == '\n' && line.n && line.p[line.n - 1] == '\r') line.n--;
        if (!utf8_valid((const unsigned char*)line.p, line.n)) {
            xfree(line.p);
            return rt_throw_std("rpp.core.FormatException", "kein gültiges UTF-8");
        }
        *out = buf_take(&line);
        return ST_OK;
    }
    if (IS("rpp.io.File.open")) {
        RStr* p = rt_s(args[0]);
        if (!is_file(p->data)) {
            Buf m = { 0 };
            buf_fmt(&m, "Datei nicht gefunden: %s", p->data);
            int st = io_error(m.p, p->data);
            xfree(m.p);
            return st;
        }
        RObj* f = (RObj*)xmalloc(sizeof(RObj));
        memset(f, 0, sizeof *f);
        f->cls = rt_cls_file;
        f->alive = 1;
        f->slots = (rv*)xmalloc(sizeof(rv) * 4);
        f->cap = 4;
        rt_vec_push(f, rt_promote(rv_view(args[0])));
        *out = rv_obj(f, F_OWN);
        return ST_OK;
    }
    if (IS("rpp.io.File.readAllText") || IS("rpp.io.File.readAllBytes")) {
        RStr* p = rt_s(o && o->len > 0 ? o->slots[0] : rv_null());
        FILE* f = u_fopen(p->data, "rb");
        if (!f) {
            Buf m = { 0 };
            buf_fmt(&m, "%s: %s", p->data, sys_err());
            int st = io_error(m.p, p->data);
            xfree(m.p);
            return st;
        }
        Buf b = { 0 };
        buf_str(&b, "");
        char tmp[65536];
        size_t k;
        while ((k = fread(tmp, 1, sizeof tmp, f)) > 0) buf_add(&b, tmp, (int64_t)k);
        fclose(f);
        if (IS("rpp.io.File.readAllBytes")) {
            RObj* v = rt_vec_new(b.n);
            for (int64_t i = 0; i < b.n; i++) rt_vec_push(v, rv_byte((uint8_t)b.p[i]));
            xfree(b.p);
            *out = rv_obj(v, F_OWN);
            return ST_OK;
        }
        if (!utf8_valid((const unsigned char*)b.p, b.n)) {
            xfree(b.p);
            Buf m = { 0 };
            buf_fmt(&m, "%s: kein gültiges UTF-8", p->data);
            int st = rt_throw_std("rpp.core.FormatException", m.p);
            xfree(m.p);
            return st;
        }
        *out = buf_take(&b);
        return ST_OK;
    }
    if (IS("rpp.io.File.writeText") || IS("rpp.io.File.writeBytes")) {
        RStr* p = rt_s(args[0]);
        Buf b = { 0 };
        buf_str(&b, "");
        rv d = args[1];
        if (d.tag == RT_STR) buf_add(&b, rt_s(d)->data, rt_s(d)->len);
        else if (d.tag == RT_OBJ || d.tag == RT_SLICE) {
            RObj* v = (RObj*)d.v.p;
            int64_t a = d.tag == RT_SLICE ? d.x : 0, e = d.tag == RT_SLICE ? d.y : v->len;
            for (int64_t i = a; i < e; i++) {
                char c = v->slots[i].tag == RT_BYTE ? (char)(uint8_t)v->slots[i].v.u : 0;
                buf_add(&b, &c, 1);
            }
        }
        FILE* f = u_fopen(p->data, "wb");
        if (!f) {
            xfree(b.p);
            Buf m = { 0 };
            buf_fmt(&m, "%s: %s", p->data, sys_err());
            int st = io_error(m.p, p->data);
            xfree(m.p);
            return st;
        }
        if (b.n) fwrite(b.p, 1, (size_t)b.n, f);
        fclose(f);
        xfree(b.p);
        return ST_OK;
    }
    if (IS("rpp.io.Dir.list")) {
        RStr* p = rt_s(args[0]);
        const char** names = NULL;
        int64_t nn = 0, cap = 0;
#ifdef _WIN32
        char pat[4096];
        snprintf(pat, sizeof pat, "%s/*", p->data);
        WIN32_FIND_DATAW fd;
        wchar_t* wpat = u8w(pat);
        HANDLE h = FindFirstFileW(wpat, &fd);
        xfree(wpat);
        if (h == INVALID_HANDLE_VALUE) {
            Buf m = { 0 };
            errno = ENOENT;
            buf_fmt(&m, "%s: %s", p->data, sys_err());
            int st = io_error(m.p, p->data);
            xfree(m.p);
            return st;
        }
        do {
            if (!wcscmp(fd.cFileName, L".") || !wcscmp(fd.cFileName, L"..")) continue;
            if (nn == cap) { cap = cap ? cap * 2 : 16; names = (const char**)xrealloc((void*)names, sizeof(char*) * (size_t)cap); }
            names[nn++] = w2u8(fd.cFileName);
        } while (FindNextFileW(h, &fd));
        FindClose(h);
#else
        DIR* d = opendir(p->data);
        if (!d) {
            Buf m = { 0 };
            buf_fmt(&m, "%s: %s", p->data, sys_err());
            int st = io_error(m.p, p->data);
            xfree(m.p);
            return st;
        }
        struct dirent* e;
        while ((e = readdir(d))) {
            if (!strcmp(e->d_name, ".") || !strcmp(e->d_name, "..")) continue;
            if (nn == cap) { cap = cap ? cap * 2 : 16; names = (const char**)xrealloc((void*)names, sizeof(char*) * (size_t)cap); }
            char* s = (char*)xmalloc(strlen(e->d_name) + 1);
            strcpy(s, e->d_name);
            names[nn++] = s;
        }
        closedir(d);
#endif
        if (nn) qsort((void*)names, (size_t)nn, sizeof(char*), cmp_str);
        RObj* v = rt_vec_new(nn);
        for (int64_t i = 0; i < nn; i++) {
            rt_vec_push(v, sv(names[i]));
            xfree((void*)names[i]);
        }
        xfree((void*)names);
        *out = rv_obj(v, F_OWN);
        return ST_OK;
    }
    if (IS("rpp.io.File.exists")) { *out = rv_bool(is_file(rt_s(args[0])->data)); return ST_OK; }
    if (IS("rpp.io.Dir.exists")) { *out = rv_bool(is_dir(rt_s(args[0])->data)); return ST_OK; }
    if (IS("rpp.io.Path.absolute")) {
        Buf b = { 0 };
        normalize_path(rt_s(args[0])->data, &b);
        *out = buf_take(&b);
        return ST_OK;
    }
    if (IS("rpp.io.Dir.create")) {
        RStr* p = rt_s(args[0]);
        if (make_dirs(p->data) != 0) {
            Buf m = { 0 };
            buf_fmt(&m, "%s: %s", p->data, sys_err());
            int st = io_error(m.p, p->data);
            xfree(m.p);
            return st;
        }
        return ST_OK;
    }
    if (IS("rpp.io.Path.join")) {
        const char* a = slash_dup(rt_s(args[0])->data);
        const char* b = slash_dup(rt_s(args[1])->data);
        Buf r = { 0 };
        buf_str(&r, "");
        if (!*a) buf_str(&r, b);
        else if (a[strlen(a) - 1] == '/') { buf_str(&r, a); buf_str(&r, b); }
        else { buf_str(&r, a); buf_str(&r, "/"); buf_str(&r, b); }
        xfree((void*)a);
        xfree((void*)b);
        *out = buf_take(&r);
        return ST_OK;
    }
    if (IS("rpp.io.Path.fileName") || IS("rpp.io.Path.extension") || IS("rpp.io.Path.parent")) {
        const char* p = slash_dup(rt_s(args[0])->data);
        const char* slash = strrchr(p, '/');
        const char* file = slash ? slash + 1 : p;
        if (IS("rpp.io.Path.fileName")) *out = sv(file);
        else if (IS("rpp.io.Path.extension")) {
            const char* dot = strrchr(file, '.');
            *out = (dot && dot > file) ? sv(dot + 1) : sv("");
        } else {
            *out = slash ? rt_str_from(p, slash - p) : sv("");
        }
        xfree((void*)p);
        return ST_OK;
    }
    /* ---------------------------------------------------------------- Datei-Operationen (R-13.31) */
    if (IS("rpp.io.File.appendText")) {
        RStr* p = rt_s(args[0]);
        Buf b = { 0 };
        buf_str(&b, "");
        text_of(&b, args[1]);
        FILE* f = u_fopen(p->data, "ab");
        if (!f) { xfree(b.p); return path_io_error(p->data); }
        if (b.n) fwrite(b.p, 1, (size_t)b.n, f);
        fclose(f);
        xfree(b.p);
        return ST_OK;
    }
    if (IS("rpp.io.File.copyFile")) {
        RStr* from = rt_s(args[0]);
        RStr* to = rt_s(args[1]);
        FILE* f = u_fopen(from->data, "rb");
        if (!f || !is_file(from->data)) {
            if (f) fclose(f);
            if (!f) return path_io_error(from->data);
            errno = EISDIR;
            return path_io_error(from->data);
        }
        FILE* g = u_fopen(to->data, "wb");
        if (!g) { int e = errno; fclose(f); errno = e; return path_io_error(from->data); }
        char tmp[65536];
        size_t k;
        int64_t total = 0;
        while ((k = fread(tmp, 1, sizeof tmp, f)) > 0) {
            if (fwrite(tmp, 1, k, g) != k) { int e = errno; fclose(f); fclose(g); errno = e; return path_io_error(from->data); }
            total += (int64_t)k;
        }
        fclose(f);
        if (fclose(g) != 0) return path_io_error(from->data);
        *out = rv_i64(total);
        return ST_OK;
    }
    if (IS("rpp.io.File.moveFile")) {
        RStr* from = rt_s(args[0]);
        RStr* to = rt_s(args[1]);
        if (replace_file(from->data, to->data) != 0) return path_io_error(from->data);
        return ST_OK;
    }
    if (IS("rpp.io.File.delete")) {
        RStr* p = rt_s(args[0]);
        if (u_remove(p->data) != 0) return path_io_error(p->data);
        return ST_OK;
    }
    if (IS("rpp.io.File.size")) {
        RStr* p = rt_s(args[0]);
#ifdef _WIN32
        struct _stat64 st;
        if (u_stat64(p->data, &st) != 0) return path_io_error(p->data);
#else
        struct stat st;
        if (stat(p->data, &st) != 0) return path_io_error(p->data);
#endif
        *out = rv_i64((int64_t)st.st_size);
        return ST_OK;
    }
    /* ---------------------------------------------------------------- SaveFile */
    if (IS("rpp.io.SaveFile.open")) {
        RStr* p = rt_s(args[0]);
        const char* target = p->data;
        const char* slash = strrchr(target, '/');
        const char* bslash = strrchr(target, '\\');
        if (bslash && (!slash || bslash > slash)) slash = bslash;
        const char* file = slash ? slash + 1 : target;
        if (!*file || !strcmp(file, ".") || !strcmp(file, "..")) {
            errno = EINVAL;
            return path_io_error(target);
        }
        SaveState* s = (SaveState*)xmalloc(sizeof(SaveState));
        memset(s, 0, sizeof *s);
        snprintf(s->target, sizeof s->target, "%s", target);
        uint64_t seed = (uint64_t)mono_nanos() ^ ((uint64_t)(uintptr_t)s << 16);
        for (int attempt = 0; attempt < 128 && !s->f; attempt++) {
            seed = seed * 6364136223846793005ULL + 1442695040888963407ULL;
            if (slash) snprintf(s->temp, sizeof s->temp, "%.*s/.rpp-save-%016llx.tmp", (int)(slash - target), target, (unsigned long long)seed);
            else snprintf(s->temp, sizeof s->temp, ".rpp-save-%016llx.tmp", (unsigned long long)seed);
            if (is_file(s->temp)) continue;
            s->f = create_new(s->temp);
            if (!s->f && errno != EEXIST) {
                int e = errno;
                xfree(s);
                errno = e;
                return path_io_error(target);
            }
        }
        if (!s->f) {
            xfree(s);
            errno = EEXIST;
            return path_io_error(target);
        }
        RObj* f = rt_obj_new(rt_cls_save_file, NULL, 0);
        f->native = s;
        *out = rv_obj(f, F_OWN);
        return ST_OK;
    }
    if (!strncmp(name, "rpp.io.SaveFile.", 16)) {
        SaveState* s = o ? (SaveState*)o->native : NULL;
        if (!s) return rt_internal("SaveFile ohne Zustand");
        if (IS("rpp.io.SaveFile.cancel")) { save_cancel(s); return ST_OK; }
        if (s->done) {
            Buf m = { 0 };
            buf_fmt(&m, "%s: Schreibvorgang ist beendet", s->target);
            int st = io_error(m.p, s->target);
            xfree(m.p);
            return st;
        }
        if (IS("rpp.io.SaveFile.commit")) {
            int ok = fflush(s->f) == 0;
            ok = fclose(s->f) == 0 && ok;
            s->f = NULL;
            int e = errno;
            if (ok && replace_file(s->temp, s->target) == 0) { s->done = 1; return ST_OK; }
            e = errno;
            save_cancel(s);
            errno = e;
            return path_io_error(s->target);
        }
        Buf b = { 0 };
        buf_str(&b, "");
        rv d = args[0];
        if (d.tag == RT_STR) buf_add(&b, rt_s(d)->data, rt_s(d)->len);
        else if (d.tag == RT_OBJ || d.tag == RT_SLICE) {
            RObj* v = (RObj*)d.v.p;
            int64_t a = d.tag == RT_SLICE ? d.x : 0, e = d.tag == RT_SLICE ? d.y : v->len;
            for (int64_t i = a; i < e; i++) {
                char c = v->slots[i].tag == RT_BYTE ? (char)(uint8_t)v->slots[i].v.u : 0;
                buf_add(&b, &c, 1);
            }
        }
        if (b.n && fwrite(b.p, 1, (size_t)b.n, s->f) != (size_t)b.n) {
            int e = errno;
            xfree(b.p);
            save_cancel(s);
            errno = e;
            return path_io_error(s->target);
        }
        xfree(b.p);
        return ST_OK;
    }
    /* ---------------------------------------------------------------- Verzeichniseinträge, Pfade */
    if (IS("rpp.io.Dir.entries")) {
        RStr* p = rt_s(args[0]);
        rv lv;
        int st = rt_native(&lv, "rpp.io.Dir.list", self, 1, args, 0, 0);
        if (st) return st;
        RObj* names = (RObj*)lv.v.p;
        const char* dir = slash_dup(p->data);
        size_t dl = strlen(dir);
        int sep = dl > 0 && dir[dl - 1] != '/';
        RObj* v = rt_vec_new(names->len);
        for (int64_t i = 0; i < names->len; i++) {
            RStr* nm = rt_s(names->slots[i]);
            Buf full = { 0 };
            buf_str(&full, dir);
            if (sep) buf_str(&full, "/");
            buf_add(&full, nm->data, nm->len);
            int kind = file_kind(full.p);
            RObj* e = rt_obj_new(rt_cls_dir_entry, NULL, 0);
            e->slots[0] = rt_promote(rv_view(names->slots[i]));
            e->slots[1] = buf_take(&full);
            e->slots[2] = rv_enum(rt_file_type_def, kind);
            rt_vec_push(v, rv_obj(e, F_OWN));
        }
        xfree((void*)dir);
        st = rt_drop(lv);
        if (st) return st;
        *out = rv_obj(v, F_OWN);
        return ST_OK;
    }
    if (IS("rpp.io.Path.normalizedText")) {
        LexPath lp;
        lex_path(rt_s(args[0])->data, &lp);
        *out = lex_text(&lp, 0);
        lex_free(&lp);
        return ST_OK;
    }
    if (IS("rpp.io.Path.relativeText")) {
        LexPath t, b;
        lex_path(rt_s(args[0])->data, &t);
        lex_path(rt_s(args[1])->data, &b);
        int ok = !strcmp(t.prefix, b.prefix) && t.rooted == b.rooted && b.n <= t.n;
        for (int i = 0; ok && i < b.n; i++) ok = !strcmp(t.parts[i], b.parts[i]);
        for (int i = b.n; ok && i < t.n; i++) ok = strcmp(t.parts[i], "..") != 0;
        if (ok && b.n == 0 && (t.prefix[0] || t.rooted)) {
            /* gleiche Wurzel ohne Teile: der Rest sind die Teile des Ziels */
            LexPath r = t;
            r.prefix[0] = 0;
            r.rooted = 0;
            *out = lex_text(&r, 0);
        } else {
            *out = ok ? lex_text(&t, b.n) : rv_null();
        }
        lex_free(&t);
        lex_free(&b);
        return ST_OK;
    }
    /* ---------------------------------------------------------------- Zeit, Mathematik, StringView */
    if (IS("rpp.env.ElapsedTimer.monotonicNanos")) { *out = rv_i64(mono_nanos()); return ST_OK; }
    if (IS("rpp.math.Math.sqrt")) { *out = rv_f64(sqrt(as_f64(args[0]))); return ST_OK; }
    if (!strncmp(name, "rpp.text.StringView.", 20)) {
        if (!o) return rt_internal("StringView ohne Objekt");
        return string_view(out, o, name + 20, args);
    }
    if (IS("rpp.env.Env.get")) {
        const char* v = getenv(rt_s(args[0])->data);
        *out = v ? sv(v) : rv_null();
        return ST_OK;
    }
    if (IS("rpp.env.Env.os")) {
        const REnumDef* d = rt_enums[rt_os_def];
#ifdef _WIN32
        const char* nm = "Windows";
#elif defined(__APPLE__)
        const char* nm = "MacOS";
#else
        const char* nm = "Linux";
#endif
        int idx = 0;
        for (int i = 0; i < d->nvariants; i++)
            if (!strcmp(d->vnames[i], nm)) idx = i;
        *out = rv_enum(rt_os_def, idx);
        return ST_OK;
    }
    if (IS("rpp.fmt.Fmt.padLeft") || IS("rpp.fmt.Fmt.padRight")) {
        Buf s = { 0 };
        buf_str(&s, "");
        text_of(&s, args[0]);
        int64_t w = argi(args, 1);
        if (w_neg_p(w_of(args[1]))) w = 0;
        int64_t nc = 0;
        for (int64_t i = 0; i < s.n; i++)
            if (((unsigned char)s.p[i] & 0xc0) != 0x80) nc++;
        Buf r = { 0 };
        buf_str(&r, "");
        if (IS("rpp.fmt.Fmt.padRight")) buf_add(&r, s.p, s.n);
        for (int64_t i = nc; i < w; i++) buf_str(&r, " ");
        if (IS("rpp.fmt.Fmt.padLeft")) buf_add(&r, s.p, s.n);
        xfree(s.p);
        *out = buf_take(&r);
        return ST_OK;
    }
    if (IS("rpp.fmt.Fmt.hex")) {
        char t[32];
        rv a = args[0];
        if (a.tag == RT_U64) snprintf(t, sizeof t, "%llx", (unsigned long long)a.v.u);
        else snprintf(t, sizeof t, "%llx", (unsigned long long)(int64_t)w_of(a).lo);
        *out = sv(t);
        return ST_OK;
    }
    if (IS("rpp.fmt.Fmt.fixed")) {
        double x = as_f64(args[0]);
        int64_t d = argi(args, 1);
        if (w_neg_p(w_of(args[1]))) d = 0;
        if (d > 300) d = 300;
        if (x != x) { *out = sv("NaN"); return ST_OK; }
        if (x == HUGE_VAL || x == -HUGE_VAL) { *out = sv(x > 0 ? "inf" : "-inf"); return ST_OK; }
        int len = snprintf(NULL, 0, "%.*f", (int)d, x);
        char* t = (char*)xmalloc((size_t)len + 1);
        snprintf(t, (size_t)len + 1, "%.*f", (int)d, x);
        *out = rt_str_from(t, len);
        xfree(t);
        return ST_OK;
    }
    if (!strncmp(name, "rpp.text.StringBuilder.", 23)) {
        if (!o) return rt_internal("StringBuilder ohne Objekt");
        if (!o->native) {
            SB* s = (SB*)xmalloc(sizeof(SB));
            memset(s, 0, sizeof *s);
            buf_str(&s->b, "");
            o->native = s;
        }
        SB* s = (SB*)o->native;
        const char* m = name + 23;
        if (!strcmp(m, "length")) { *out = rv_i64(s->chars); return ST_OK; }
        if (!strcmp(m, "toString")) { *out = rt_str_from(s->b.p, s->b.n); return ST_OK; }
        if (!strcmp(m, "clear")) { s->b.n = 0; s->b.p[0] = 0; s->chars = 0; return ST_OK; }
        Buf t = { 0 };
        buf_str(&t, "");
        text_of(&t, args[0]);
        if (!strcmp(m, "appendLine")) buf_str(&t, "\n");
        buf_add(&s->b, t.p, t.n);
        for (int64_t i = 0; i < t.n; i++)
            if (((unsigned char)t.p[i] & 0xc0) != 0x80) s->chars++;
        xfree(t.p);
        return ST_OK;
    }
    if (!strncmp(name, "rpp.collections.", 16)) {
        if (!o) return rt_internal("Sammlung ohne Objekt");
        int map = !strncmp(name, "rpp.collections.Map.", 20);
        const char* m = strrchr(name, '.') + 1;
        int64_t len = o->len;
        if (!strcmp(m, "length")) { *out = rv_i64(len); return ST_OK; }
        if (!strcmp(m, "begin")) { *out = rv_i64(0); return ST_OK; }
        if (!strcmp(m, "done")) { *out = rv_bool(!w_neg_p(w_of(args[0])) && argi(args, 0) >= len); return ST_OK; }
        if (!strcmp(m, "next")) { *out = rv_i64(argi(args, 0) + 1); return ST_OK; }
        if (!strcmp(m, "at") && at_cursor) {
            int64_t i = argi(args, 0);
            if (w_neg_p(w_of(args[0])) || i >= len) return rt_fatal("IndexError", "Cursor %lld außerhalb von 0..%lld", (long long)i, (long long)len);
            *out = rv_view(o->slots[i]);
            return ST_OK;
        }
        if (!strcmp(m, "contains")) {
            int64_t pos;
            int st = find_key(&pos, o, args[0], map);
            if (st) return st;
            *out = rv_bool(pos >= 0);
            return ST_OK;
        }
        if (!strcmp(m, "get") || !strcmp(m, "at")) {
            int64_t pos;
            int st = find_key(&pos, o, args[0], 1);
            if (st) return st;
            if (pos >= 0) {
                RObj* e = (RObj*)o->slots[pos].v.p;
                *out = rv_view(e->slots[1]);
                return ST_OK;
            }
            if (!strcmp(m, "get")) { *out = rv_null(); return ST_OK; }
            return rt_fatal("IndexError", "der Schlüssel fehlt in der Map");
        }
        if (!strcmp(m, "put")) {
            rv key = args[0], value = args[1];
            int64_t pos;
            int st = find_key(&pos, o, key, 1);
            if (st) return st;
            if (pos >= 0) {
                RObj* e = (RObj*)o->slots[pos].v.p;
                rv old = e->slots[1];
                e->slots[1] = value;
                st = rt_drop(old);
                if (st) return st;
                return rt_drop(key);
            }
            int64_t hv;
            st = rt_hash(&hv, key);
            if (st) return st;
            RObj* e = rt_obj_new(rt_cls_entry, NULL, 0);
            e->slots[0] = key;
            e->slots[1] = value;
            rt_vec_push(o, rv_obj(e, F_OWN));
            hidx_add(o, hv, o->len - 1);
            return ST_OK;
        }
        if (!strcmp(m, "add")) {
            rv x = args[0];
            int64_t pos;
            int st = find_key(&pos, o, x, 0);
            if (st) return st;
            if (pos >= 0) return rt_drop(x);
            int64_t hv;
            st = rt_hash(&hv, x);
            if (st) return st;
            rt_vec_push(o, x);
            hidx_add(o, hv, o->len - 1);
            return ST_OK;
        }
        if (!strcmp(m, "getOrPut")) {
            /* R-13.33: Wert zum Schlüssel, sonst factory() einsetzen */
            rv key = args[0];
            int64_t pos;
            int st = find_key(&pos, o, key, 1);
            if (st) return st;
            if (pos >= 0) {
                st = rt_drop(key);
                if (st) return st;
            } else {
                rv value;
                st = rt_call_value(&value, args[1], NULL);
                if (st) { rt_drop(key); return st; }
                int64_t hv;
                st = rt_hash(&hv, key);
                if (st) return st;
                RObj* e = rt_obj_new(rt_cls_entry, NULL, 0);
                e->slots[0] = key;
                e->slots[1] = value;
                rt_vec_push(o, rv_obj(e, F_OWN));
                pos = o->len - 1;
                hidx_add(o, hv, pos);
            }
            RObj* e = (RObj*)o->slots[pos].v.p;
            *out = rv_view(e->slots[1]);
            return ST_OK;
        }
        if (!strcmp(m, "appendEntry")) {
            /* Eintrag einer anderen Menge übernehmen, der schon auf Eindeutigkeit geprüft ist */
            RObj* src = (RObj*)args[0].v.p;
            int64_t i = argi(args, 1);
            /* own-Objekte werden wie im Interpreter (Halter) von beiden Mengen geteilt */
            src->slots[i] = rt_to_shared(src->slots[i]);
            rv x = rt_own_copy(rv_view(src->slots[i]));
            int64_t hv;
            int st = rt_hash(&hv, x);
            if (st) return st;
            rt_vec_push(o, x);
            hidx_add(o, hv, o->len - 1);
            return ST_OK;
        }
        if (!strcmp(m, "remove")) {
            int64_t pos;
            int st = find_key(&pos, o, args[0], map);
            if (st) return st;
            if (pos < 0) { *out = map ? rv_null() : rv_void(); return ST_OK; }
            rv item = o->slots[pos];
            memmove(o->slots + pos, o->slots + pos + 1, sizeof(rv) * (size_t)(o->len - pos - 1));
            o->len--;
            st = reindex(o, map);
            if (st) return st;
            if (!map) return rt_drop(item);
            RObj* e = (RObj*)item.v.p;
            rv value = e->slots[1];
            e->slots[1] = rv_moved();
            st = rt_drop(item);
            if (st) return st;
            *out = value;
            return ST_OK;
        }
        if (!strcmp(m, "clear")) {
            rv* items = o->slots;
            int64_t k = o->len;
            o->slots = (rv*)xmalloc(sizeof(rv) * 4);
            o->len = 0;
            o->cap = 4;
            HIdx* h = hidx(o);
            h->nb = 0;
            for (int64_t i = 0; i < k; i++) {
                int st = rt_drop(items[i]);
                if (st) { xfree(items); return st; }
            }
            xfree(items);
            return ST_OK;
        }
    }
#undef IS
    return rt_fatal("InternalError", "%s ist nicht eingebaut", name);
}

/* ================================================================ Hilfen für den erzeugten Code */


rv rt_temp_str(rv v) {
    if (v.tag == RT_STR) return rt_temp(v);
    return v;
}

int rt_case_int(rv v, uint64_t mag, int neg) {
    if (!rt_is_int_tag(v.tag)) return 0;
    W w = w_from_u(mag);
    if (neg) w = w_negate(w);
    return w_cmp(w_of(v), w) == 0;
}

int rt_variant_is(rv v, int32_t def, int64_t idx) {
    if (v.tag == RT_ENUM) return (def < 0 || v.x == def) && v.v.i == idx;
    if (v.tag == RT_ENUMD) {
        RAgg* a = (RAgg*)v.v.p;
        return (def < 0 || a->def == def) && a->idx == idx;
    }
    return 0;
}

rv rt_variant_item(rv v, int i) {
    if (v.tag == RT_ENUMD) {
        RAgg* a = (RAgg*)v.v.p;
        if (i < a->n) return rv_view(a->items[i]);
    }
    return rv_null();
}

rv rt_str_char_at(RStr* s, int64_t pos) {
    uint32_t c;
    utf8_dec(s->data, s->len, pos, &c);
    return rv_char(c);
}

int64_t rt_str_char_len(RStr* s, int64_t pos) {
    uint32_t c;
    return utf8_dec(s->data, s->len, pos, &c);
}

int rt_fatal_text(const char* name, rv msg) {
    Buf b = { 0 };
    buf_str(&b, "");
    text_of(&b, msg);
    return fatal_v(name, b.p);
}

const RTy** rt_targs_copy(const RTy** a, int n) {
    const RTy** r = (const RTy**)xmalloc(sizeof(RTy*) * (size_t)(n + 1));
    for (int i = 0; i < n; i++) r[i] = a[i];
    return r;
}

const RTy* rt_targ(rv self, int32_t cls, int idx) {
    if (self.tag != RT_OBJ || idx < 0) return NULL;
    RObj* o = (RObj*)self.v.p;
    if (!o->cls) return NULL;
    if (o->cls->id == cls) return idx < o->ntargs ? o->targs[idx] : NULL;
    if (o->cls->tmap) return o->cls->tmap(o->targs, o->ntargs, cls, idx);
    return NULL;
}

const RTy* rt_tyv(int32_t def, int32_t n, ...) {
    const RTy* a[16];
    va_list ap;
    va_start(ap, n);
    for (int i = 0; i < n && i < 16; i++) a[i] = va_arg(ap, const RTy*);
    va_end(ap);
    return rt_ty(1, def, n, a);
}

int rt_new_param(rv* out, const RTy* t) {
    if (!t || t->kind != 1 || !rt_classes[t->def])
        return rt_internal("new T(): T ist zur Laufzeit nicht bekannt (Typargumente generischer Methoden werden noch nicht mitgeführt)");
    const RClass* c = rt_classes[t->def];
    RObj* o = rt_obj_new(c, t->args, t->n);
    int st = c->dflt(o, NULL);
    if (st) {
        rt_obj_fail(o);
        return st;
    }
    *out = rv_obj(o, F_OWN);
    return ST_OK;
}

static const RTy** pending_mtargs = NULL;

void rt_set_mtargs(const RTy** t) { pending_mtargs = t; }

const RTy** rt_take_mtargs(void) {
    const RTy** t = pending_mtargs;
    pending_mtargs = NULL;
    return t;
}

int rt_no_method(rv* ret, rv self, rv* args) {
    (void)ret;
    (void)self;
    (void)args;
    return rt_internal("die Klasse ist nicht iterierbar");
}

/* ================================================================ Start */

typedef struct MainArgs {
    int argc;
    char** argv;
    int (*body)(rv*, rv);
    int exit;
} MainArgs;

static void print_trace(RTrace t) {
    if (rt_release) return;
    for (int i = 0; i < t.n; i++) fprintf(stderr, "    bei %s\n", t.labels[i]);
}

static void run_main(MainArgs* m) {
    RObj* v = rt_vec_new(m->argc);
#ifdef _WIN32
    /* Argumente als UTF-8 (argv ist auf Windows in der ANSI-Codepage) */
    int wn = 0;
    wchar_t** wa = CommandLineToArgvW(GetCommandLineW(), &wn);
    if (wa && wn == m->argc) {
        for (int i = 1; i < wn; i++) {
            char* u = w2u8(wa[i]);
            rt_vec_push(v, rt_str_c(u));
            xfree(u);
        }
        LocalFree(wa);
    } else {
        if (wa) LocalFree(wa);
        for (int i = 1; i < m->argc; i++) rt_vec_push(v, rt_str_c(m->argv[i]));
    }
#else
    for (int i = 1; i < m->argc; i++) rt_vec_push(v, rt_str_c(m->argv[i]));
#endif
    rv ret = rv_void();
    int st = m->body(&ret, rv_obj(v, F_OWN));
    fflush(stdout);
    if (st == ST_OK) {
        m->exit = rt_is_int_tag(ret.tag) ? (int32_t)w_of(ret).lo : 0;
    } else if (st == ST_THROW) {
        rv ex = rt_take_exc();
        RTrace t = rt_exc_trace;
        rv msg;
        rt_describe(&msg, ex);
        const char* name = (ex.tag == RT_OBJ && ((RObj*)ex.v.p)->cls) ? ((RObj*)ex.v.p)->cls->name : "";
        fprintf(stderr, "%s: %s\n", name, rt_s(msg)->data);
        print_trace(t);
        rt_drop(ex);
        m->exit = 1;
    } else if (st == ST_FATAL) {
        fprintf(stderr, "%s: %s\n", fatal_name ? fatal_name : "?", fatal_msg ? fatal_msg : "");
        print_trace(fatal_trace);
        m->exit = 2;
    } else {
        m->exit = 0;
    }
    fflush(stdout);
    fflush(stderr);
}

#ifdef _WIN32
/* RPP_PROFILE=DATEI: Stichproben des Befehlszählers (1 ms) als Adressen relativ zum Modul; je Zeile
 * der Befehlszähler und bis zu sieben Aufrufer (aus den Abwicklungsdaten des Moduls) */
static volatile int prof_stop = 0;
static HANDLE prof_target;
static DWORD WINAPI prof_thread(LPVOID p) {
    FILE* f = u_fopen((const char*)p, "w");
    if (!f) return 0;
    uintptr_t base = (uintptr_t)GetModuleHandleA(NULL);
    timeBeginPeriod(1);
    while (!prof_stop) {
        Sleep(1);
        if (SuspendThread(prof_target) == (DWORD)-1) break;
        CONTEXT c;
        memset(&c, 0, sizeof c);
        c.ContextFlags = CONTEXT_FULL;
        if (GetThreadContext(prof_target, &c)) {
            fprintf(f, "%llx", (unsigned long long)(c.Rip - base));
            for (int k = 0; k < 7; k++) {
                DWORD64 ib = 0;
                PRUNTIME_FUNCTION fe = RtlLookupFunctionEntry(c.Rip, &ib, NULL);
                if (!fe) {
                    c.Rip = *(DWORD64*)c.Rsp;
                    c.Rsp += 8;
                } else {
                    PVOID hd = NULL;
                    DWORD64 ef = 0;
                    RtlVirtualUnwind(UNW_FLAG_NHANDLER, ib, c.Rip, fe, &c, &hd, &ef, NULL);
                }
                if (c.Rip < base || c.Rip - base > 0x10000000) break;
                fprintf(f, " %llx", (unsigned long long)(c.Rip - base));
            }
            fputc('\n', f);
        }
        ResumeThread(prof_target);
    }
    fclose(f);
    return 0;
}

static DWORD WINAPI main_thread(LPVOID p) {
    run_main((MainArgs*)p);
    return 0;
}
#else
static void* main_thread(void* p) {
    run_main((MainArgs*)p);
    return NULL;
}
#endif

int rt_main(int argc, char** argv, int (*body)(rv*, rv), int release) {
    rt_release = release;
    rt_exc = rv_null();
    if (getenv("RPP_NOFREE")) rt_nofree = 1;
#ifdef _WIN32
    _setmode(_fileno(stdout), _O_BINARY);
    _setmode(_fileno(stderr), _O_BINARY);
    _setmode(_fileno(stdin), _O_BINARY);
#endif
    static char outbuf[1 << 16];
    setvbuf(stdout, outbuf, _IOFBF, sizeof outbuf);
    MainArgs m;
    m.argc = argc;
    m.argv = argv;
    m.body = body;
    m.exit = 0;
    /* RPP_MAINTHREAD: ohne eigenen Stack laufen (für Profiler, die nur den Hauptthread sehen) */
    if (getenv("RPP_MAINTHREAD")) {
        run_main(&m);
        return m.exit;
    }
#ifdef _WIN32
    HANDLE h = CreateThread(NULL, (SIZE_T)1 << 31, main_thread, &m, STACK_SIZE_PARAM_IS_A_RESERVATION, NULL);
    if (!h) {
        run_main(&m);
    } else {
        HANDLE ph = NULL;
        const char* prof = getenv("RPP_PROFILE");
        if (prof) {
            prof_target = h;
            ph = CreateThread(NULL, 0, prof_thread, (LPVOID)prof, 0, NULL);
        }
        WaitForSingleObject(h, INFINITE);
        if (ph) {
            prof_stop = 1;
            WaitForSingleObject(ph, INFINITE);
        }
        CloseHandle(h);
    }
#else
    pthread_attr_t a;
    pthread_attr_init(&a);
    pthread_attr_setstacksize(&a, (size_t)1 << 31);
    pthread_t t;
    if (pthread_create(&t, &a, main_thread, &m) != 0) run_main(&m);
    else pthread_join(t, NULL);
#endif
    return m.exit;
}
