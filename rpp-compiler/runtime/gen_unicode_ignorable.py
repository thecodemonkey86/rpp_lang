"""Erzeugt runtime/rpp_unicode_ign.h: Case_Ignorable nach Unicode (Kategorien Mn, Me, Cf, Lm, Sk
und Word_Break MidLetter, MidNumLet, Single_Quote), für das Schluss-Sigma in toLower.
Aufruf (aus runtime/): py gen_unicode_ignorable.py > rpp_unicode_ign.h"""
import unicodedata

EXTRA = {0x27, 0x2E, 0x3A, 0xB7, 0x387, 0x55F, 0x5F4, 0x2018, 0x2019, 0x2024, 0x2027, 0xFE13, 0xFE52, 0xFE55, 0xFF07, 0xFF0E, 0xFF1A}
ranges = []
start = None
for cp in range(0x110000):
    yes = cp in EXTRA or unicodedata.category(chr(cp)) in ("Mn", "Me", "Cf", "Lm", "Sk")
    if yes and start is None:
        start = cp
    elif not yes and start is not None:
        ranges.append((start, cp - 1))
        start = None
if start is not None:
    ranges.append((start, 0x10FFFF))
print("/* erzeugt von runtime/gen_unicode_ignorable.py (Unicode " + unicodedata.unidata_version + "); nicht von Hand ändern */")
print("static const uint32_t u_ignorable[][2] = {")
for a, b in ranges:
    print(f"    {{0x{a:X}, 0x{b:X}}},")
print("};")
print(f"#define U_IGNORABLE_N {len(ranges)}")
