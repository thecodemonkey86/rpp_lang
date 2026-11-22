/* Laufzeit für den C-Codegenerator von rpp-compiler (Meilenstein M6).
 *
 * Die Werte und Regeln folgen dem Debug-Interpreter von rppc (rpp-interp, Referenz nach
 * R-12.3): Ein Wert (rv) trägt seine Art und bei Objekten die Besitzform am Verweis (own,
 * shared, weak, Borrow). Freigegeben wird ausdrücklich, wenn der Besitzer den Wert aufgibt
 * (V-7, R-12.11). Fehler laufen als Status durch die Aufrufe: 0 normal, 1 Exception
 * (rt_exc), 2 Programmfehler (rt_fatal_*); der erzeugte Code räumt seine Gültigkeitsbereiche
 * auf dem Weg nach außen auf.
 *
 * Portables C17 (MSVC, gcc, clang), ohne Erweiterungen.
 */
#ifndef RPP_RT_H
#define RPP_RT_H

#include <stdint.h>
#include <stddef.h>

/* ---------------------------------------------------------------- Status */
#define ST_OK 0
#define ST_THROW 1
#define ST_FATAL 2
/* nur im erzeugten Code innerhalb einer Funktion */
#define ST_RET 3
#define ST_BRK 4
#define ST_CONT 5

/* ---------------------------------------------------------------- Werte */
enum {
    RT_VOID, RT_NULL, RT_MOVED, RT_UNINIT,
    RT_BOOL, RT_CHAR, RT_BYTE,
    RT_I8, RT_I16, RT_I32, RT_I64, RT_U8, RT_U16, RT_U32, RT_U64, RT_F32, RT_F64,
    RT_STR,
    RT_OBJ,     /* Objekt oder Vektor (RObj) */
    RT_TUP,     /* Tupel (RAgg) */
    RT_ENUM,    /* Enum-Wert ohne Daten: v.i Variante, x Typ */
    RT_ENUMD,   /* Enum-Wert mit Daten (RAgg) */
    RT_FUNC,    /* Funktionswert (RFun) */
    RT_SLICE    /* Bereich eines Vektors: v.p Vektor, x Anfang, y Ende */
};

enum { F_OWN, F_SHARED, F_WEAK, F_BORROW, F_WEAKB /* geliehene Sicht auf einen weak-Verweis */ };

typedef struct rv {
    union {
        int64_t i;
        uint64_t u;
        double f;
        float f32;
        void* p;
    } v;
    int32_t x, y;
    uint8_t tag;
    uint8_t form;
} rv;

typedef struct RStr {
    int64_t rc;      /* < 0: unvergänglich (Literal) */
    int64_t len;     /* Bytes */
    int64_t chars;   /* Zeichen; < 0: noch nicht gezählt */
    char data[];     /* UTF-8, mit 0 abgeschlossen */
} RStr;

struct RClass;
struct RTy;

typedef struct RObj {
    const struct RClass* cls;   /* NULL: Vektor */
    int64_t rc;                 /* Anzahl der shared-Verweise */
    int32_t weak;               /* Anzahl der weak-Verweise (halten den Speicher) */
    int32_t lock;               /* 0 frei, n > 0 lesend, -1 schreibend (V-1) */
    uint8_t alive, dying, freed;
    int64_t len, cap;
    rv* slots;                  /* Felder (geerbte zuerst) bzw. Elemente */
    const struct RTy** targs;   /* Typargumente der Klasse (R-7.26) */
    int32_t ntargs;
    void* native;               /* eingebaute Inhalte (StringBuilder, Hash-Index) */
} RObj;

/* Tupel und Enum-Werte mit Daten: unveränderlich, gezählt geteilt */
typedef struct RAgg {
    int64_t rc;
    int32_t def, idx;   /* Enum: Typ und Variante */
    int32_t n;
    rv items[];
} RAgg;

typedef struct RFun RFun;
typedef int (*RFn)(rv* ret, RFun* c, rv* args);
struct RFun {
    int64_t rc;
    uint8_t own;
    RFn fn;
    rv self;        /* this (nur normale Lambdas) */
    int32_t n;
    rv** cells;     /* erfasste Variablen */
    rv* vals;       /* own-Lambda: die erfassten Werte selbst */
};

/* Laufzeittypen (für Typargumente und new T()) */
typedef struct RTy {
    int32_t kind;   /* 0 sonst, 1 Klasse */
    int32_t def;
    int32_t n;
    const struct RTy** args;
} RTy;

typedef int (*RMethod)(rv* ret, rv self, rv* args);
typedef int (*RInit)(RObj* o);
typedef int (*RCopy)(RObj* n, rv src);
typedef int (*RDtor)(RObj* o);

typedef struct RClass {
    int32_t id;
    const char* name;          /* einfacher Name (Meldungen) */
    const char* full;          /* voller Name mit Paket */
    const struct RClass* base;
    int32_t nfields;           /* alle Felder einschließlich der geerbten */
    int32_t first;             /* erstes eigenes Feld */
    int32_t nsupers;
    const int32_t* supers;     /* Klassen und Mixins, in die die Klasse umgewandelt wird */
    RDtor dtor;                /* Destruktor dieser Stufe oder NULL */
    RInit init;                /* Feldinitialisierer dieser Stufe */
    RCopy copy;                /* copy-Rumpf dieser Stufe oder NULL */
    RMethod equals, compare, hash, describe;
    int32_t native;            /* 1 StringBuilder, 2 Map, 3 Set, 4 File */
    const uint8_t* fowned;     /* Feld ist ein own-Objekt (für copy ohne Rumpf) */
    int (*dflt)(RObj* o, rv* args);  /* parameterloser Konstruktor (new T()) */
    const struct RTy* (*tmap)(const struct RTy** targs, int n, int32_t c, int idx);  /* Typargumente der Obertypen */
} RClass;

/* Typen: Enums (für Kopie oder Move und cast) */
typedef struct REnumDef {
    const char* name;
    int32_t nvariants;
    const int64_t* values;     /* Wert je Variante (cast) */
    const char* const* vnames;
    uint8_t plain;             /* keine Variante mit Daten */
} REnumDef;

/* vom erzeugten Code bereitgestellt */
extern const REnumDef* const rt_enums[];
extern const RClass* const rt_classes[];
extern const int32_t rt_def_count;
extern const RClass* rt_cls_entry;           /* rpp.collections.Entry */
extern const RClass* rt_cls_file;            /* rpp.io.File */
extern int32_t rt_os_def;                    /* rpp.env.Os */
extern int rt_std_new(rv* out, const char* full, int nargs, rv* args); /* Exception der Standardbibliothek */

/* ---------------------------------------------------------------- Zustand */
extern int rt_release;          /* Releasemodus */
extern rv rt_exc;               /* laufende Exception */
extern int rt_fatals;

typedef struct RTrace {
    int32_t n;
    const char** labels;
} RTrace;
extern RTrace rt_exc_trace;

/* ---------------------------------------------------------------- Werte bauen */
static inline rv rv_make(uint8_t tag) { rv r; r.v.i = 0; r.x = 0; r.y = 0; r.tag = tag; r.form = F_OWN; return r; }
static inline rv rv_void(void) { return rv_make(RT_VOID); }
static inline rv rv_null(void) { return rv_make(RT_NULL); }
static inline rv rv_moved(void) { return rv_make(RT_MOVED); }
static inline rv rv_uninit(void) { return rv_make(RT_UNINIT); }
static inline rv rv_bool(int b) { rv r = rv_make(RT_BOOL); r.v.i = b ? 1 : 0; return r; }
static inline rv rv_char(uint32_t c) { rv r = rv_make(RT_CHAR); r.v.i = c; return r; }
static inline rv rv_byte(uint8_t b) { rv r = rv_make(RT_BYTE); r.v.i = b; return r; }
static inline rv rv_f64(double f) { rv r = rv_make(RT_F64); r.v.f = f; return r; }
static inline rv rv_f32(float f) { rv r = rv_make(RT_F32); r.v.i = 0; r.v.f32 = f; return r; }
static inline rv rv_i64(int64_t i) { rv r = rv_make(RT_I64); r.v.i = i; return r; }
static inline rv rv_i32(int32_t i) { rv r = rv_make(RT_I32); r.v.i = i; return r; }
static inline rv rv_enum(int32_t def, int64_t idx) { rv r = rv_make(RT_ENUM); r.v.i = idx; r.x = def; return r; }
static inline rv rv_obj(RObj* o, uint8_t form) { rv r = rv_make(RT_OBJ); r.v.p = o; r.form = form; return r; }
rv rv_int(int64_t v, uint8_t tag);              /* make_int für Werte, die in i64 passen */
rv rv_uint(uint64_t v, uint8_t tag);
static inline int rv_truthy(rv v) { return v.tag == RT_BOOL && v.v.i != 0; }
static inline int rv_isnull(rv v) { return v.tag == RT_NULL; }
static inline RObj* rv_o(rv v) { return (RObj*)v.v.p; }
int rt_is_int_tag(uint8_t t);
int rt_is_num_tag(uint8_t t);
int rt_is_prim(rv v);

/* Strings */
rv rt_lit(RStr** cache, const char* s, int64_t len);
rv rt_str_from(const char* s, int64_t len);     /* neuer String (own) */
rv rt_str_c(const char* s);
RStr* rt_s(rv v);
int64_t rt_str_chars(RStr* s);
rv rt_text(rv v);                               /* to_text: Text eines Werts (own) */

/* ---------------------------------------------------------------- Besitz */
rv rv_view(rv v);
int rt_owning(rv v);
int rt_drop(rv v);
rv rt_temp(rv v);                               /* besitzten Zwischenwert bis zum Ende der Anweisung halten */
int64_t rt_tmark(void);
int rt_trelease(int64_t mark, int st);          /* Zwischenwerte freigeben, Status zusammenführen */
void rt_tdiscard(int64_t mark);
int rt_combine(int st, int d);
rv rt_promote(rv v);
rv rt_own_copy(rv v);
int rt_plain(rv v);
rv rt_transfer_var(rv* cell);                   /* Variable an einen Besitzer übergeben (transfer) */
rv rt_to_shared(rv v);                          /* convert in einen shared-Typ */
int rt_to_weak(rv* v);                          /* convert in einen weak-Typ */
rv rt_num_to(rv v, uint8_t tag);                /* Zahl in einen anderen Zahltyp */
int rt_convert_exact(rv* v, uint8_t tag);       /* Ergebnis einer Verbundzuweisung */
int rt_scope_drop(int st, rv* vars, int n);     /* Variablen in umgekehrter Reihenfolge freigeben */

/* ---------------------------------------------------------------- Objekte */
RObj* rt_obj_new(const RClass* c, const RTy** targs, int32_t ntargs);
void rt_obj_fail(RObj* o);                      /* Konstruktion gescheitert: nicht mehr lebendig */
int rt_drop_fields(RObj* o, int64_t upto);
int rt_run_dtors(RObj* o, const RClass* from);  /* Destruktoren ab Stufe from bis zur Wurzel */
RObj* rt_vec_new(int64_t cap);
void rt_vec_push(RObj* v, rv x);
rv rt_vec_from(int n, rv* items);               /* Array-Literal */
rv rt_tuple(int n, rv* items);
rv rt_enumd(int32_t def, int32_t idx, int n, rv* items);
int rt_is_subclass(const RClass* c, int32_t def);
const RTy* rt_ty(int32_t kind, int32_t def, int32_t n, const RTy** args);
const RTy* rt_tyv(int32_t def, int32_t n, ...);
int rt_field_index(RObj* o, int idx);
int rt_check_lock(RObj* o, int mutable, int line);
int rt_acquire(RObj* o, int mutable, int line);
void rt_unlock(RObj* o, int mutable);
static inline RObj* rt_shared_root(rv v) { return (v.tag == RT_OBJ && v.form == F_SHARED) ? (RObj*)v.v.p : NULL; }
int rt_lock_weak(rv* out, rv w);                /* w.lock() */
int rt_unpack(rv v, int n, rv* out);            /* Tupel oder own-Objekt zerlegen (R-5.25) */
int rt_index(rv* out, rv obj, rv idx);          /* a[i] lesend (Sicht) */
int rt_index_slot(rv** out, rv obj, rv idx);    /* a[i] als Ort */
int rt_slice(rv* out, rv obj, int has_from, rv from, int has_to, rv to);
int rt_member_len(rv* out, rv obj, int bytes);  /* length / byteLength */
int rt_deep_copy(rv* out, rv v);
int rt_describe(rv* out, rv ex);

/* Lambdas */
RFun* rt_fun_new(RFn fn, int own, int n);
rv rv_fun(RFun* f, uint8_t form);
int rt_call_value(rv* ret, rv f, rv* args);
int rt_capture_own(rv* cell, rv* out);          /* own-Lambda: Erfassung per Wert (R-10.8) */

/* Rahmen */
int rt_enter(const char* label);
void rt_leave(void);
int rt_depth(void);
RTrace rt_trace(void);

/* Fehler */
int rt_throw(rv ex);                            /* throw mit Stacktrace */
int rt_fatal(const char* name, const char* fmt, ...);
int rt_fatal_at(int st, int line);              /* (Zeile N) an einen Programmfehler anhängen */
int rt_internal(const char* msg);
int rt_throw_std(const char* full, const char* msg);
int rt_catch_matches(int32_t def);              /* passt die laufende Exception zu catch(def)? */
rv rt_take_exc(void);

/* Arithmetik und Vergleiche */
enum { OP_ADD, OP_SUB, OP_MUL, OP_DIV, OP_REM, OP_AND, OP_OR, OP_XOR, OP_SHL, OP_SHR };
int rt_arith(rv* out, int op, rv a, rv b);
int rt_equal(int* out, rv a, rv b);
int rt_compare(int* out, rv a, rv b);
int rt_neg(rv* out, rv a);
rv rt_bitnot(rv a);
int rt_incdec(rv* out, rv cur, int d);
int rt_cast_prim(rv* out, rv v, uint8_t tag);
int rt_cast_enum(rv* out, rv v, int32_t def);
int rt_int_result_i64(rv* out, int64_t v, uint8_t tag);
rv rt_minmax(uint8_t tag, int max);
rv rt_float_lit(const char* text, int negative, int f32);
rv rt_int_lit(const char* text, int negative, uint8_t tag);

/* eingebaute Funktionen und Methoden */
void rt_print(rv v);
int rt_builtin_method(rv* out, rv recv, const char* name, int n, rv* args);
int rt_builtin_static(rv* out, uint8_t ptag, const char* name, int n, rv* args);
int rt_prim_mixin(rv* out, const char* name, rv v, rv* args, int* handled);
int rt_hash(int64_t* out, rv v);
int rt_native(rv* out, const char* name, rv self, int n, rv* args, int mut_this, int at_cursor);

/* Hilfen für den erzeugten Code */
rv rt_conv_num(rv v, uint8_t tag);              /* convert in einen Zahltyp */
rv rt_own_str(rv v);                            /* gespeicherter String besitzt seinen Text */
rv rt_temp_str(rv v);
int rt_case_int(rv v, uint64_t mag, int neg);
int rt_variant_is(rv v, int32_t def, int64_t idx);
rv rt_variant_item(rv v, int i);
rv rt_str_char_at(RStr* s, int64_t pos);
int64_t rt_str_char_len(RStr* s, int64_t pos);
int rt_fatal_text(const char* name, rv msg);
const RTy** rt_targs_copy(const RTy** a, int n);
const RTy* rt_targ(rv self, int32_t cls, int idx);
int rt_new_param(rv* out, const RTy* t);
int rt_no_method(rv* ret, rv self, rv* args);

/* Start */
int rt_main(int argc, char** argv, int (*body)(rv* ret, rv args), int release);

#endif
