/* C-Testbibliothek für die Konformitätstests von Kapitel 15 (C-Anbindung).
 *
 * Der Testlauf übersetzt diese Datei vor den Tests zu einer gemeinsam genutzten Bibliothek
 * im Ordner build/ daneben: rpptest.dll (mit rpptest.lib) unter Windows, librpptest.so unter
 * Linux, librpptest.dylib unter macOS. Die Testprojekte nennen diesen Ordner in nativePaths.
 * Alle Namen beginnen mit tl_, damit sie keinen Namen einer C-Laufzeit einer Implementierung
 * treffen (die von rppcc beginnen mit rt_).
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#define API __declspec(dllexport)
#else
#define API __attribute__((visibility("default")))
#endif

/* ------------------------------------------------------------ Ganzzahlen (R-15.6) */

API int32_t tl_add_i32(int32_t a, int32_t b) { return a + b; }
API int64_t tl_mul_i64(int64_t a, int64_t b) { return a * b; }
API int8_t tl_neg_i8(int8_t a) { return (int8_t)-a; }
API int16_t tl_twice_i16(int16_t a) { return (int16_t)(a * 2); }
API uint8_t tl_inc_u8(uint8_t a) { return (uint8_t)(a + 1); }
API uint16_t tl_inc_u16(uint16_t a) { return (uint16_t)(a + 1); }
API uint32_t tl_xor_u32(uint32_t a, uint32_t b) { return a ^ b; }
API uint64_t tl_max_u64(void) { return UINT64_MAX; }
API uint8_t tl_not_byte(uint8_t b) { return (uint8_t)~b; }

/* Viele Parameter gemischter Art: prüft die Aufrufkonvention über die Register hinaus. */
API int64_t tl_sum8(int8_t a, int16_t b, int32_t c, int64_t d, uint8_t e, uint16_t f, uint32_t g, uint64_t h) {
    return (int64_t)a + b + c + d + e + f + (int64_t)g + (int64_t)h;
}

/* ------------------------------------------------------------ Gleitkomma und bool */

API double tl_scale(double x, float factor) { return x * (double)factor; }
API float tl_half(float x) { return x / 2.0f; }
API bool tl_and(bool a, bool b) { return a && b; }
API bool tl_is_positive(double x) { return x > 0.0; }
API double tl_mix(int32_t a, double b, int64_t c, float d, bool e, double f, uint8_t g, double h, double i, double j) {
    return a + b + (double)c + d + (e ? 1.0 : 0.0) + f + g + h + i + j;
}

/* ------------------------------------------------------------ Strings (R-15.7, R-15.8) */

static char greeting[256];

API const char* tl_greet(const char* name) {
    snprintf(greeting, sizeof greeting, "Hallo, %s!", name);
    return greeting;
}

API int64_t tl_byte_length(const char* s) { return (int64_t)strlen(s); }

API const char* tl_unicode(void) { return "gr\xC3\xBC\xC3\x9F" "e \xE2\x82\xAC"; }

API const char* tl_maybe(bool give) { return give ? "da" : NULL; }

API const char* tl_null_string(void) { return NULL; }

API const char* tl_invalid_utf8(void) { return "\xFF\xFE"; }

/* ------------------------------------------------------------ byte[] (R-15.9) */

API uint32_t tl_sum_bytes(const uint8_t* data, size_t len) {
    uint32_t s = 0;
    for (size_t i = 0; i < len; i++) {
        s += data[i];
    }
    return s;
}

API void tl_fill(uint8_t* data, size_t len, uint8_t value) {
    for (size_t i = 0; i < len; i++) {
        data[i] = (uint8_t)(value + i);
    }
}

API int64_t tl_length(const uint8_t* data, size_t len) {
    (void)data;
    return (int64_t)len;
}

/* ------------------------------------------------------------ Handles (R-15.5, R-15.10) */

typedef struct {
    int64_t value;
} Counter;

static int64_t live_counters = 0;
static int64_t freed_counters = 0;

API Counter* tl_counter_new(int64_t start) {
    Counter* c = (Counter*)malloc(sizeof(Counter));
    if (c == NULL) {
        return NULL;
    }
    c->value = start;
    live_counters++;
    return c;
}

API Counter* tl_counter_none(void) { return NULL; }

API int64_t tl_counter_next(Counter* c) { return c->value++; }

API int64_t tl_counter_peek(const Counter* c) { return c->value; }

API void tl_counter_free(void* p) {
    if (p != NULL) {
        free(p);
        live_counters--;
        freed_counters++;
    }
}

/* übernimmt das Handle (own) und gibt es selbst frei */
API int64_t tl_counter_consume(Counter* c) {
    int64_t v = c->value;
    tl_counter_free(c);
    return v;
}

API int64_t tl_live_counters(void) { return live_counters; }
API int64_t tl_freed_counters(void) { return freed_counters; }

/* ------------------------------------------------------------ Ausgabe (V-15.6) */

API void tl_print(const char* text) {
#ifdef _WIN32
    /* wie die Ausgabe von R++: \n ohne \r (die Bibliothek hat ihre eigene C-Laufzeit) */
    _setmode(_fileno(stdout), _O_BINARY);
#endif
    printf("%s\n", text);
    fflush(stdout);
}
