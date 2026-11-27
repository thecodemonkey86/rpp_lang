//! Das Verzeichnis im Dateisystem (Kapitel 11, R-11.26 bis R-11.30): Versionen,
//! Prüfsummen und die Sperrdatei `rpp.lock`. Die Auflösung selbst steht in `project.rs`.

use crate::yaml::{self, Node, Value};
use std::fmt;
use std::path::{Path, PathBuf};

/// Eine Version `MAJOR.MINOR.PATCH` (R-11.27).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(pub u64, pub u64, pub u64);

impl Version {
    fn parse_parts(text: &str, min_parts: usize) -> Option<Version> {
        let parts: Vec<&str> = text.split('.').collect();
        if parts.len() < min_parts || parts.len() > 3 {
            return None;
        }
        let mut nums = [0u64; 3];
        for (i, p) in parts.iter().enumerate() {
            let ok = !p.is_empty()
                && p.len() <= 9
                && p.bytes().all(|b| b.is_ascii_digit())
                && (p.len() == 1 || !p.starts_with('0'));
            if !ok {
                return None;
            }
            nums[i] = p.parse().ok()?;
        }
        Some(Version(nums[0], nums[1], nums[2]))
    }

    /// Vollständige Version, wie sie `version` in einer `rpp.yaml` verlangt.
    pub fn parse(text: &str) -> Option<Version> {
        Version::parse_parts(text, 3)
    }

    /// Mindestversion einer Abhängigkeit; hintere Teile dürfen fehlen (`1.4` = `1.4.0`).
    pub fn parse_min(text: &str) -> Option<Version> {
        Version::parse_parts(text, 1)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// Der Name einer Bibliothek aus dem Verzeichnis: genau `besitzer.name`, beide Teile
/// ASCII-Bezeichner und keine Schlüsselwörter (R-11.17, R-11.27).
pub fn valid_registry_name(name: &str) -> bool {
    let parts: Vec<&str> = name.split('.').collect();
    parts.len() == 2
        && parts.iter().all(|p| {
            let b = p.as_bytes();
            !b.is_empty()
                && (b[0].is_ascii_alphabetic() || b[0] == b'_')
                && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_')
                && rpp_syntax::lexer::Keyword::lookup(p).is_none()
        })
}

/// Die veröffentlichten Versionen einer Bibliothek, aufsteigend; `true` heißt zurückgezogen.
pub fn versions(lib_dir: &Path) -> Vec<(Version, bool)> {
    let Ok(rd) = std::fs::read_dir(lib_dir) else { return Vec::new() };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !e.path().is_dir() {
            continue;
        }
        if let Some(v) = Version::parse(&name) {
            let withdrawn = lib_dir.join(format!("{name}.withdrawn")).is_file();
            out.push((v, withdrawn));
        }
    }
    out.sort();
    out
}

// ---------------------------------------------------------------- SHA-256 (FIPS 180-4)

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// SHA-256 von `data` als 64 Kleinbuchstaben-Hexziffern.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut msg = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());
    let mut w = [0u32; 64];
    for chunk in msg.chunks(64) {
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[4 * i], chunk[4 * i + 1], chunk[4 * i + 2], chunk[4 * i + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(y);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

// ---------------------------------------------------------------- Prüfsumme (R-11.29)

fn collect(dir: &Path, rel: &str, out: &mut Vec<(String, PathBuf)>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        let r = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
        if p.is_dir() {
            collect(&p, &r, out);
        } else if p.extension().is_some_and(|x| x == "rpp") || (rel.is_empty() && name == "rpp.yaml") {
            out.push((r, p));
        }
    }
}

/// Prüfsumme einer Bibliothek im Ordner `dir` (R-11.29): `rpp.yaml` und alle `.rpp`-Dateien
/// darunter, je mit SHA-256 ihres Inhalts (CRLF als LF), sortiert nach dem relativen Pfad.
pub fn checksum(dir: &Path) -> std::io::Result<String> {
    let mut files = Vec::new();
    collect(dir, "", &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut summary = String::new();
    for (rel, path) in files {
        let bytes = std::fs::read(&path)?;
        let mut text = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
                i += 1;
                continue;
            }
            text.push(bytes[i]);
            i += 1;
        }
        summary.push_str(&sha256_hex(&text));
        summary.push_str("  ");
        summary.push_str(&rel);
        summary.push('\n');
    }
    Ok(format!("sha256:{}", sha256_hex(summary.as_bytes())))
}

// ---------------------------------------------------------------- rpp.lock (R-11.30)

#[derive(Debug, Clone)]
pub struct LockEntry {
    pub name: String,
    pub version: Version,
    pub checksum: String,
    pub line: u32,
    pub checksum_line: u32,
}

/// Liest `rpp.lock`. Ein Fehler nennt Zeile und Meldung.
pub fn parse_lock(text: &str) -> Result<Vec<LockEntry>, (u32, String)> {
    let doc = yaml::parse(text).map_err(|e| (e.line, e.message))?;
    let bad = |n: &Node, msg: &str| Err((n.line, msg.to_string()));
    for (key, node) in doc.entries() {
        if key != "libraries" {
            return bad(node, &format!("Unknown entry '{key}' in rpp.lock"));
        }
    }
    let Some(list) = doc.get("libraries") else {
        return Err((1, "rpp.lock requires the entry libraries".into()));
    };
    if !matches!(list.value, Value::List(_)) {
        return bad(list, "libraries must be a list");
    }
    let mut out: Vec<LockEntry> = Vec::new();
    for item in list.as_list() {
        if !matches!(item.value, Value::Map(_)) {
            return bad(item, "An entry of rpp.lock requires name, version and checksum");
        }
        for (key, node) in item.entries() {
            if !["name", "version", "checksum"].contains(&key.as_str()) {
                return bad(node, &format!("Unknown entry '{key}' in rpp.lock"));
            }
        }
        let field = |k: &str| item.get(k).and_then(Node::as_str);
        let (Some(name), Some(version), Some(sum)) = (field("name"), field("version"), field("checksum")) else {
            return bad(item, "An entry of rpp.lock requires name, version and checksum");
        };
        let Some(v) = Version::parse(version) else {
            return bad(item.get("version").unwrap(), &format!("Invalid version '{version}' in rpp.lock"));
        };
        if out.iter().any(|e| e.name == name) {
            return bad(item, &format!("rpp.lock lists '{name}' twice"));
        }
        out.push(LockEntry {
            name: name.to_string(),
            version: v,
            checksum: sum.to_string(),
            line: item.get("name").map_or(item.line, |n| n.line),
            checksum_line: item.get("checksum").map_or(item.line, |n| n.line),
        });
    }
    Ok(out)
}

/// Text der Sperrdatei; `libs` ist nach Namen sortiert.
pub fn format_lock(libs: &[(String, Version, String)]) -> String {
    let mut out = String::from(
        "# rpp.lock: exact versions and checksums of the libraries from the registry (R-11.20).\n\
         # Written by the lock command; do not edit.\n",
    );
    if libs.is_empty() {
        out.push_str("libraries: []\n");
        return out;
    }
    out.push_str("libraries:\n");
    for (name, version, sum) in libs {
        out.push_str(&format!("  - name: {name}\n    version: {version}\n    checksum: {sum}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_values() {
        assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(sha256_hex(&[b'a'; 56]), "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a");
    }

    #[test]
    fn versions_parse() {
        assert_eq!(Version::parse("1.2.3"), Some(Version(1, 2, 3)));
        assert_eq!(Version::parse("1.2"), None);
        assert_eq!(Version::parse_min("1.4"), Some(Version(1, 4, 0)));
        assert_eq!(Version::parse_min("01.4"), None);
        assert_eq!(Version::parse_min("1.x"), None);
        assert!(Version(1, 10, 0) > Version(1, 9, 9));
    }
}
