// Erzeugt runtime/rpp_unicode.h: Unicode-Tabellen für die C-Laufzeit aus der
// Standardbibliothek von Rust, damit `char` und Strings sich genau wie im Interpreter
// (rppc) verhalten. Aufruf (aus runtime/):
//   rustc -O gen_unicode.rs -o gen_unicode.exe && gen_unicode.exe > rpp_unicode.h
use std::fmt::Write;

fn ranges(name: &str, pred: impl Fn(char) -> bool, out: &mut String) {
    let mut list: Vec<(u32, u32)> = Vec::new();
    let mut start: Option<u32> = None;
    for cp in 0..=0x10FFFFu32 {
        let yes = char::from_u32(cp).map(&pred).unwrap_or(false);
        match (yes, start) {
            (true, None) => start = Some(cp),
            (false, Some(s)) => {
                list.push((s, cp - 1));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        list.push((s, 0x10FFFF));
    }
    writeln!(out, "static const uint32_t {name}[][2] = {{").unwrap();
    for (a, b) in &list {
        writeln!(out, "    {{0x{a:X}, 0x{b:X}}},").unwrap();
    }
    writeln!(out, "}};\n#define {}_N {}\n", name.to_uppercase(), list.len()).unwrap();
}

fn mapping(name: &str, map: impl Fn(char) -> Vec<char>, out: &mut String) {
    writeln!(out, "static const uint32_t {name}[][4] = {{").unwrap();
    let mut n = 0;
    for cp in 0..=0x10FFFFu32 {
        let Some(c) = char::from_u32(cp) else { continue };
        let m = map(c);
        if m.len() == 1 && m[0] == c {
            continue;
        }
        let mut cols = [0u32; 3];
        for (i, x) in m.iter().enumerate().take(3) {
            cols[i] = *x as u32;
        }
        writeln!(out, "    {{0x{cp:X}, 0x{:X}, 0x{:X}, 0x{:X}}},", cols[0], cols[1], cols[2]).unwrap();
        n += 1;
    }
    writeln!(out, "}};\n#define {}_N {}\n", name.to_uppercase(), n).unwrap();
}

fn main() {
    let mut out = String::new();
    out.push_str("/* erzeugt von runtime/gen_unicode.rs aus der Standardbibliothek von Rust; nicht von Hand ändern */\n");
    out.push_str("#ifndef RPP_UNICODE_H\n#define RPP_UNICODE_H\n#include <stdint.h>\n\n");
    ranges("u_alpha", |c| c.is_alphabetic(), &mut out);
    ranges("u_upper", |c| c.is_uppercase(), &mut out);
    ranges("u_lower", |c| c.is_lowercase(), &mut out);
    ranges("u_space", |c| c.is_whitespace(), &mut out);
    // Cased (für das Schluss-Sigma): Großbuchstabe, Kleinbuchstabe oder Titelbuchstabe
    ranges("u_cased", |c| c.is_uppercase() || c.is_lowercase() || (!c.to_lowercase().eq(std::iter::once(c)) && !c.to_uppercase().eq(std::iter::once(c))), &mut out);
    mapping("u_upmap", |c| c.to_uppercase().collect(), &mut out);
    mapping("u_lowmap", |c| c.to_lowercase().collect(), &mut out);
    // Case_Ignorable (für das Schluss-Sigma) erzeugt gen_unicode_ignorable.py aus den Kategorien
    out.push_str("#include \"rpp_unicode_ign.h\"\n#endif\n");
    print!("{out}");
}
