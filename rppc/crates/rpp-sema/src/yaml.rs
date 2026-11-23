//! Der YAML-Teil, den `rpp.yaml` und `rpp.lock` verwenden (R-11.13): Zuordnungen,
//! Listen, einfache Werte, `[a, b]` und Kommentare. Jeder Wert kennt seine Zeile.

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    List(Vec<Node>),
    Map(Vec<(String, Node)>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub value: Value,
    /// Zeile, in der der Wert bzw. sein Schlüssel steht (ab 1)
    pub line: u32,
}

impl Node {
    pub fn as_str(&self) -> Option<&str> {
        match &self.value {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_list(&self) -> &[Node] {
        match &self.value {
            Value::List(l) => l,
            _ => &[],
        }
    }
    pub fn entries(&self) -> &[(String, Node)] {
        match &self.value {
            Value::Map(m) => m,
            _ => &[],
        }
    }
    pub fn get(&self, key: &str) -> Option<&Node> {
        self.entries().iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
}

#[derive(Debug)]
pub struct YamlError {
    pub line: u32,
    pub message: String,
}

struct Line {
    indent: usize,
    text: String,
    number: u32,
}

fn strip_comment(s: &str) -> &str {
    let mut in_quote = false;
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'"' => in_quote = !in_quote,
            b'#' if !in_quote && (i == 0 || bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') => return &s[..i],
            _ => {}
        }
    }
    s
}

fn scalar(s: &str, line: u32) -> Node {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        let items = inner
            .split(',')
            .map(str::trim)
            .filter(|x| !x.is_empty())
            .map(|x| Node { value: Value::Str(unquote(x)), line })
            .collect();
        return Node { value: Value::List(items), line };
    }
    Node { value: Value::Str(unquote(s)), line }
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\''))) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Liest ein YAML-Dokument der eingeschränkten Form.
pub fn parse(text: &str) -> Result<Node, YamlError> {
    let lines: Vec<Line> = text
        .lines()
        .enumerate()
        .filter_map(|(i, raw)| {
            let content = strip_comment(raw).trim_end();
            if content.trim().is_empty() {
                return None;
            }
            let indent = content.len() - content.trim_start().len();
            Some(Line { indent, text: content.trim_start().to_string(), number: i as u32 + 1 })
        })
        .collect();
    let mut pos = 0;
    let node = block(&lines, &mut pos, 0)?;
    if pos < lines.len() {
        return Err(YamlError { line: lines[pos].number, message: "unerwartete Einrückung".into() });
    }
    Ok(node.unwrap_or(Node { value: Value::Map(Vec::new()), line: 1 }))
}

/// Liest einen Block mit genau der Einrückung `indent`.
fn block(lines: &[Line], pos: &mut usize, indent: usize) -> Result<Option<Node>, YamlError> {
    let Some(first) = lines.get(*pos) else { return Ok(None) };
    if first.indent != indent {
        return Ok(None);
    }
    let line = first.number;
    if first.text.starts_with("- ") || first.text == "-" {
        let mut items = Vec::new();
        while let Some(l) = lines.get(*pos) {
            if l.indent != indent || !(l.text.starts_with("- ") || l.text == "-") {
                break;
            }
            let rest = l.text[1..].trim_start().to_string();
            let number = l.number;
            *pos += 1;
            if rest.is_empty() {
                let inner = block(lines, pos, lines.get(*pos).map_or(indent + 2, |n| n.indent).max(indent + 1))?;
                items.push(inner.unwrap_or(Node { value: Value::Str(String::new()), line: number }));
            } else if let Some((k, v)) = split_key(&rest) {
                // Liste von Zuordnungen: "- name: x" und weitere Schlüssel darunter
                let item_indent = indent + 2;
                let mut entries = vec![(k.clone(), value_after(lines, pos, &v, number, item_indent)?)];
                while let Some(n) = lines.get(*pos) {
                    if n.indent != item_indent || n.text.starts_with('-') {
                        break;
                    }
                    let Some((k2, v2)) = split_key(&n.text) else {
                        return Err(YamlError { line: n.number, message: "Schlüssel erwartet".into() });
                    };
                    let nl = n.number;
                    *pos += 1;
                    entries.push((k2, value_after(lines, pos, &v2, nl, item_indent)?));
                }
                items.push(Node { value: Value::Map(entries), line: number });
            } else {
                items.push(scalar(&rest, number));
            }
        }
        return Ok(Some(Node { value: Value::List(items), line }));
    }
    let mut entries = Vec::new();
    while let Some(l) = lines.get(*pos) {
        if l.indent != indent {
            if l.indent > indent {
                return Err(YamlError { line: l.number, message: "unerwartete Einrückung".into() });
            }
            break;
        }
        let Some((k, v)) = split_key(&l.text) else {
            return Err(YamlError { line: l.number, message: "Schlüssel erwartet".into() });
        };
        let number = l.number;
        *pos += 1;
        entries.push((k, value_after(lines, pos, &v, number, indent)?));
    }
    Ok(Some(Node { value: Value::Map(entries), line }))
}

fn value_after(lines: &[Line], pos: &mut usize, v: &str, number: u32, indent: usize) -> Result<Node, YamlError> {
    if !v.trim().is_empty() {
        return Ok(scalar(v, number));
    }
    // Wert auf den folgenden, tiefer (oder bei Listen gleich) eingerückten Zeilen
    match lines.get(*pos) {
        Some(n) if n.indent > indent || (n.indent == indent && n.text.starts_with('-')) => {
            let ind = n.indent;
            let mut node = block(lines, pos, ind)?.unwrap();
            node.line = number;
            Ok(node)
        }
        _ => Ok(Node { value: Value::Str(String::new()), line: number }),
    }
}

fn split_key(text: &str) -> Option<(String, String)> {
    let i = text.find(':')?;
    let key = text[..i].trim();
    if key.is_empty() || key.contains(' ') {
        return None;
    }
    let rest = &text[i + 1..];
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    Some((key.to_string(), rest.trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projektdatei() {
        let text = "# Kopf\nname: app   # Name\nversion: 0.1.0\nsources:\n  - src\ndependencies:\n  - name: acme.a\n    path: a   # FEHLER\nsymbols:\n  debug: [TRACE, X]\n  release: []\n";
        let doc = parse(text).unwrap();
        assert_eq!(doc.get("name").unwrap().as_str(), Some("app"));
        assert_eq!(doc.get("name").unwrap().line, 2);
        assert_eq!(doc.get("sources").unwrap().as_list()[0].as_str(), Some("src"));
        let dep = &doc.get("dependencies").unwrap().as_list()[0];
        assert_eq!(dep.get("path").unwrap().as_str(), Some("a"));
        assert_eq!(dep.get("path").unwrap().line, 8);
        assert_eq!(doc.get("symbols").unwrap().get("debug").unwrap().as_list().len(), 2);
        assert_eq!(doc.get("symbols").unwrap().get("release").unwrap().as_list().len(), 0);
    }
}
