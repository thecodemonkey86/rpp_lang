//! Bedingte Kompilierung (Kapitel 11, R-11.23 bis R-11.25).
//!
//! Löst `#if`, `#elif`, `#else` und `#endif` vor dem Lexer auf. Direktiven und die
//! Zeilen inaktiver Zweige werden durch Leerzeilen ersetzt, damit Zeilennummern
//! erhalten bleiben. Inaktive Zweige werden nicht geprüft, die Bedingungen von
//! Direktiven dagegen immer. Entspricht `rpp-grammar/tools/preprocess.py`.

use std::collections::HashSet;
use std::fmt;

/// Fehler in einer Direktive, mit der Zeile, in der er auftritt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectiveError {
    pub line: u32,
    pub message: String,
}

impl fmt::Display for DirectiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Line {}: {}", self.line, self.message)
    }
}

fn err<T>(line: u32, message: impl Into<String>) -> Result<T, DirectiveError> {
    Err(DirectiveError { line, message: message.into() })
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CondToken {
    And,
    Or,
    Not,
    Open,
    Close,
    Symbol(String),
}

impl fmt::Display for CondToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CondToken::And => write!(f, "'&&'"),
            CondToken::Or => write!(f, "'||'"),
            CondToken::Not => write!(f, "'!'"),
            CondToken::Open => write!(f, "'('"),
            CondToken::Close => write!(f, "')'"),
            CondToken::Symbol(s) => write!(f, "'{s}'"),
        }
    }
}

fn tokenize(text: &str, line: u32) -> Result<Vec<CondToken>, DirectiveError> {
    let chars: Vec<char> = text.trim_end().chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        let two = |a: char, b: char| c == a && chars.get(i + 1) == Some(&b);
        if two('&', '&') {
            tokens.push(CondToken::And);
            i += 2;
        } else if two('|', '|') {
            tokens.push(CondToken::Or);
            i += 2;
        } else if c == '!' {
            tokens.push(CondToken::Not);
            i += 1;
        } else if c == '(' {
            tokens.push(CondToken::Open);
            i += 1;
        } else if c == ')' {
            tokens.push(CondToken::Close);
            i += 1;
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            tokens.push(CondToken::Symbol(chars[start..i].iter().collect()));
        } else {
            return err(line, format!("invalid condition {:?}", text.trim_end()));
        }
    }
    Ok(tokens)
}

/// Wertet eine Bedingung aus; `&&` bindet stärker als `||`.
pub fn evaluate(text: &str, symbols: &HashSet<String>, line: u32) -> Result<bool, DirectiveError> {
    let tokens = tokenize(text, line)?;
    if tokens.is_empty() {
        return err(line, "missing condition");
    }
    let mut eval = Eval { tokens: &tokens, pos: 0, symbols, line };
    let value = eval.or_expr()?;
    if eval.pos != tokens.len() {
        return err(line, format!("unexpected {}", tokens[eval.pos]));
    }
    Ok(value)
}

struct Eval<'a> {
    tokens: &'a [CondToken],
    pos: usize,
    symbols: &'a HashSet<String>,
    line: u32,
}

impl Eval<'_> {
    fn peek(&self) -> Option<&CondToken> {
        self.tokens.get(self.pos)
    }

    fn or_expr(&mut self) -> Result<bool, DirectiveError> {
        let mut value = self.and_expr()?;
        while self.peek() == Some(&CondToken::Or) {
            self.pos += 1;
            let right = self.and_expr()?;
            value = value || right;
        }
        Ok(value)
    }

    fn and_expr(&mut self) -> Result<bool, DirectiveError> {
        let mut value = self.unary()?;
        while self.peek() == Some(&CondToken::And) {
            self.pos += 1;
            let right = self.unary()?;
            value = value && right;
        }
        Ok(value)
    }

    fn unary(&mut self) -> Result<bool, DirectiveError> {
        let token = self.peek().cloned();
        self.pos += 1;
        match token {
            Some(CondToken::Not) => Ok(!self.unary()?),
            Some(CondToken::Open) => {
                let value = self.or_expr()?;
                let close = self.peek().cloned();
                self.pos += 1;
                if close != Some(CondToken::Close) {
                    return err(self.line, "missing ')'");
                }
                Ok(value)
            }
            Some(CondToken::Symbol(s)) => Ok(self.symbols.contains(&s)),
            _ => err(self.line, "expected a symbol"),
        }
    }
}

struct Level {
    outer: bool,
    active: bool,
    taken: bool,
    seen_else: bool,
    line: u32,
}

/// Wendet die Direktiven mit den gesetzten Symbolen an (R-11.24).
pub fn preprocess(source: &str, symbols: &HashSet<String>) -> Result<String, DirectiveError> {
    let mut out: Vec<&str> = Vec::new();
    let mut stack: Vec<Level> = Vec::new();
    let all_active = |stack: &Vec<Level>| stack.iter().all(|l| l.active);

    for (index, line) in source.split('\n').enumerate() {
        let number = index as u32 + 1;
        let stripped = line.trim();
        if !stripped.starts_with('#') {
            out.push(if all_active(&stack) { line } else { "" });
            continue;
        }
        let body = &stripped[1..];
        let name_len = body.chars().take_while(|c| c.is_ascii_alphabetic()).count();
        let (directive, rest) = if name_len > 0 {
            (&body[..name_len], body[name_len..].trim_start())
        } else {
            (stripped, "")
        };
        // Zeilenkommentar nach einer Direktive (R-11.23)
        let rest = match rest.find("//") {
            Some(i) => rest[..i].trim_end(),
            None => rest,
        };
        match directive {
            "if" => {
                let outer = all_active(&stack);
                let value = evaluate(rest, symbols, number)?;
                stack.push(Level { outer, active: outer && value, taken: value, seen_else: false, line: number });
            }
            "elif" => {
                if stack.last().map_or(true, |l| l.seen_else) {
                    return err(number, "#elif without a matching #if");
                }
                let value = evaluate(rest, symbols, number)?;
                let level = stack.last_mut().unwrap();
                level.active = level.outer && !level.taken && value;
                level.taken = level.taken || value;
            }
            "else" => {
                if stack.last().map_or(true, |l| l.seen_else) || !rest.is_empty() {
                    return err(number, "#else without a matching #if");
                }
                let level = stack.last_mut().unwrap();
                level.active = level.outer && !level.taken;
                level.taken = true;
                level.seen_else = true;
            }
            "endif" => {
                if stack.is_empty() || !rest.is_empty() {
                    return err(number, "#endif without a matching #if");
                }
                stack.pop();
            }
            _ => {
                let shown = if name_len > 0 { directive } else { &stripped[1..] };
                return err(number, format!("unknown directive #{shown}"));
            }
        }
        out.push("");
    }
    if let Some(level) = stack.last() {
        return err(level.line, "#if without #endif");
    }
    Ok(out.join("\n"))
}

/// Die Symbole, die ohne Projektdatei gesetzt sind: Modus und Betriebssystem.
pub fn default_symbols() -> HashSet<String> {
    ["debug", "linux"].iter().map(|s| s.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn syms(list: &[&str]) -> HashSet<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn verschachtelt() {
        let src = "a\n#if debug && !windows\nb\n#elif linux\nc\n#else\nd\n#endif\ne";
        assert_eq!(preprocess(src, &syms(&["debug", "linux"])).unwrap(), "a\n\nb\n\n\n\n\n\ne");
        assert_eq!(preprocess(src, &syms(&["release", "linux"])).unwrap(), "a\n\n\n\nc\n\n\n\ne");
    }

    #[test]
    fn fehler() {
        assert!(preprocess("#define X 1", &syms(&[])).is_err());
        assert_eq!(preprocess("x\n#if debug\ny", &syms(&[])).unwrap_err().line, 2);
        assert!(preprocess("#if (debug && linux\n#endif", &syms(&[])).is_err());
        assert!(preprocess("#if debug && && linux\n#endif", &syms(&[])).is_err());
    }
}
