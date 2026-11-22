//! Lexer (Kapitel 2 des Spezifikationsgerüsts, Lexer-Regeln in `RPP.g4`).

use std::fmt;

/// Position im Quelltext: Zeile ab 1, Spalte ab 0 (wie ANTLR).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct Span {
    pub line: u32,
    pub col: u32,
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

macro_rules! keywords {
    ($($variant:ident = $text:literal),* $(,)?) => {
        /// Reservierte Wörter. `main` ist keines (Gerüst, Abschnitt 2).
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Keyword { $($variant),* }

        impl Keyword {
            pub fn lookup(s: &str) -> Option<Keyword> {
                match s { $($text => Some(Keyword::$variant),)* _ => None }
            }
            pub fn as_str(self) -> &'static str {
                match self { $(Keyword::$variant => $text),* }
            }
        }
    };
}

keywords! {
    Abstract = "abstract", As = "as", Base = "base", Bool = "bool", Break = "break",
    Byte = "byte", Case = "case", Cast = "cast", Catch = "catch", Char = "char",
    Class = "class", Const = "const", Constructor = "constructor", Continue = "continue",
    Copy = "copy", Default = "default", Destructor = "destructor", Discard = "discard",
    Do = "do", Else = "else", Enum = "enum", Export = "export", F32 = "f32", F64 = "f64",
    False = "false", Finally = "finally", For = "for", I8 = "i8", I16 = "i16", I32 = "i32",
    I64 = "i64", If = "if", Import = "import", Mixin = "mixin", Mut = "mut", New = "new",
    Nodiscard = "nodiscard", Null = "null", Override = "override", Own = "own",
    Package = "package", Private = "private", Protected = "protected", Public = "public",
    Ref = "ref", Return = "return", Shared = "shared", Static = "static", String = "string",
    Switch = "switch", This = "this", Throw = "throw", True = "true", Try = "try",
    Ui8 = "ui8", Ui16 = "ui16", Ui32 = "ui32", Ui64 = "ui64", Using = "using", Var = "var",
    Virtual = "virtual", Void = "void", Weak = "weak", While = "while",
}

macro_rules! puncts {
    ($($variant:ident = $text:literal),* $(,)?) => {
        /// Satzzeichen und Operatoren. `<<` und `>>` gibt es nicht als Token.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Punct { $($variant),* }

        impl Punct {
            pub fn as_str(self) -> &'static str {
                match self { $(Punct::$variant => $text),* }
            }
            /// Alle Satzzeichen, längste zuerst (größter Treffer gewinnt).
            const ALL: &'static [Punct] = &[$(Punct::$variant),*];
        }
    };
}

puncts! {
    // drei Zeichen gibt es nicht; zwei Zeichen zuerst
    QuestionDot = "?.", QuestionQuestion = "??", ColonColon = "::", DotDot = "..",
    PlusPlus = "++", MinusMinus = "--", AndAnd = "&&", OrOr = "||", EqEq = "==",
    NotEq = "!=", LtEq = "<=", GtEq = ">=", PlusEq = "+=", MinusEq = "-=", StarEq = "*=",
    SlashEq = "/=", PercentEq = "%=", Arrow = "=>",
    LBrace = "{", RBrace = "}", LParen = "(", RParen = ")", LBracket = "[", RBracket = "]",
    Semi = ";", Comma = ",", Dot = ".", Question = "?", Colon = ":", Assign = "=",
    Lt = "<", Gt = ">", Plus = "+", Minus = "-", Star = "*", Slash = "/", Percent = "%",
    Bang = "!", Tilde = "~", Amp = "&", Pipe = "|", Caret = "^",
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Keyword(Keyword),
    /// Bezeichner, einschließlich `main`
    Ident(String),
    Int(String),
    Float(String),
    /// Inhalt ohne Anführungszeichen, Escapes noch nicht aufgelöst
    Str(String),
    Char(String),
    Punct(Punct),
    /// Zeichen, das kein Token beginnt; der Parser meldet es als Fehler.
    Error(String),
    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Keyword(k) => write!(f, "'{}'", k.as_str()),
            TokenKind::Ident(s) | TokenKind::Int(s) | TokenKind::Float(s) => write!(f, "'{s}'"),
            TokenKind::Str(s) => write!(f, "'\"{s}\"'"),
            TokenKind::Char(s) => write!(f, "''{s}''"),
            TokenKind::Punct(p) => write!(f, "'{}'", p.as_str()),
            TokenKind::Error(s) => write!(f, "{s}"),
            TokenKind::Eof => write!(f, "Dateiende"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

struct Lexer<'a> {
    chars: Vec<char>,
    pos: usize,
    line: u32,
    col: u32,
    _src: &'a str,
}

/// Zerlegt den (vorverarbeiteten) Quelltext in Tokens. Ein Zeichen, das kein Token
/// beginnt, wird zu einem `Error`-Token; danach endet die Zerlegung.
pub fn lex(source: &str) -> Vec<Token> {
    let mut lx = Lexer { chars: source.chars().collect(), pos: 0, line: 1, col: 0, _src: source };
    let mut tokens = Vec::new();
    loop {
        lx.skip_trivia_or_error(&mut tokens);
        if matches!(tokens.last(), Some(Token { kind: TokenKind::Error(_), .. })) {
            break;
        }
        let span = Span { line: lx.line, col: lx.col };
        if lx.pos >= lx.chars.len() {
            tokens.push(Token { kind: TokenKind::Eof, span });
            break;
        }
        let kind = lx.next_token();
        let stop = matches!(kind, TokenKind::Error(_));
        tokens.push(Token { kind, span });
        if stop {
            break;
        }
    }
    tokens
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

impl Lexer<'_> {
    fn peek(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn bump(&mut self) -> char {
        let c = self.chars[self.pos];
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 0;
        } else {
            self.col += 1;
        }
        c
    }

    fn skip_trivia_or_error(&mut self, tokens: &mut Vec<Token>) {
        loop {
            match self.peek(0) {
                Some(' ' | '\t' | '\r' | '\n') => {
                    self.bump();
                }
                Some('/') if self.peek(1) == Some('/') => {
                    while !matches!(self.peek(0), None | Some('\r' | '\n')) {
                        self.bump();
                    }
                }
                Some('/') if self.peek(1) == Some('*') => {
                    let span = Span { line: self.line, col: self.col };
                    self.bump();
                    self.bump();
                    loop {
                        match self.peek(0) {
                            None => {
                                tokens.push(Token {
                                    kind: TokenKind::Error("Kommentar ohne '*/'".into()),
                                    span,
                                });
                                return;
                            }
                            Some('*') if self.peek(1) == Some('/') => {
                                self.bump();
                                self.bump();
                                break;
                            }
                            _ => {
                                self.bump();
                            }
                        }
                    }
                }
                _ => return,
            }
        }
    }

    /// Länge einer Ziffernfolge ab `offset`: `[0-9] ([0-9_]* [0-9])?` bzw. hexadezimal.
    fn digits_len(&self, offset: usize, hex: bool) -> usize {
        let ok = |c: char| if hex { c.is_ascii_hexdigit() } else { c.is_ascii_digit() };
        if !self.peek(offset).is_some_and(ok) {
            return 0;
        }
        let mut end = offset + 1;
        let mut last_digit = end;
        while let Some(c) = self.peek(end) {
            if ok(c) {
                end += 1;
                last_digit = end;
            } else if c == '_' {
                end += 1;
            } else {
                break;
            }
        }
        last_digit - offset
    }

    fn take(&mut self, n: usize) -> String {
        (0..n).map(|_| self.bump()).collect()
    }

    fn next_token(&mut self) -> TokenKind {
        let c = self.peek(0).unwrap();
        if is_ident_start(c) {
            let mut n = 1;
            while self.peek(n).is_some_and(is_ident_char) {
                n += 1;
            }
            let text = self.take(n);
            return match Keyword::lookup(&text) {
                Some(k) => TokenKind::Keyword(k),
                None => TokenKind::Ident(text),
            };
        }
        if c.is_ascii_digit() {
            return self.number();
        }
        if c == '"' {
            return self.quoted('"', true);
        }
        if c == '\'' {
            return self.quoted('\'', false);
        }
        for p in Punct::ALL {
            let text = p.as_str();
            if text.chars().enumerate().all(|(i, ch)| self.peek(i) == Some(ch)) {
                self.take(text.chars().count());
                return TokenKind::Punct(*p);
            }
        }
        self.bump();
        TokenKind::Error(format!("unerwartetes Zeichen '{c}'"))
    }

    fn number(&mut self) -> TokenKind {
        if self.peek(0) == Some('0') && self.peek(1) == Some('x') {
            let hex = self.digits_len(2, true);
            if hex > 0 {
                return TokenKind::Int(self.take(2 + hex));
            }
        }
        let int = self.digits_len(0, false);
        // FloatLiteral : Digits '.' Digits ([eE] [+-]? Digits)?
        if self.peek(int) == Some('.') {
            let frac = self.digits_len(int + 1, false);
            if frac > 0 {
                let mut n = int + 1 + frac;
                if matches!(self.peek(n), Some('e' | 'E')) {
                    let sign = usize::from(matches!(self.peek(n + 1), Some('+' | '-')));
                    let exp = self.digits_len(n + 1 + sign, false);
                    if exp > 0 {
                        n += 1 + sign + exp;
                    }
                }
                return TokenKind::Float(self.take(n));
            }
        }
        TokenKind::Int(self.take(int))
    }

    /// StringLiteral : '"' (~["\\\r\n] | '\\' .)* '"'
    /// CharLiteral   : '\'' (~['\\\r\n] | '\\' .) '\''
    fn quoted(&mut self, quote: char, many: bool) -> TokenKind {
        // Länge eines Elements ab n: ein Zeichen oder ein Escape; None, wenn keins folgt.
        let element = |lx: &Self, n: usize| -> Option<usize> {
            match lx.peek(n) {
                Some('\\') => lx.peek(n + 1).map(|_| 2),
                Some(c) if c != quote && c != '\r' && c != '\n' => Some(1),
                _ => None,
            }
        };
        let mut n = 1;
        let mut count = 0;
        loop {
            if self.peek(n) == Some(quote) && (many || count == 1) {
                break;
            }
            match element(self, n) {
                Some(len) if many || count == 0 => {
                    n += len;
                    count += 1;
                }
                _ => {
                    self.bump();
                    let what = if many { "String" } else { "Zeichen" };
                    return TokenKind::Error(format!("ungültiges {what}-Literal"));
                }
            }
        }
        let text = self.take(n + 1);
        let inner: String = text.chars().skip(1).take(n - 1).collect();
        if many {
            TokenKind::Str(inner)
        } else {
            TokenKind::Char(inner)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        lex(src).into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn zahlen() {
        use TokenKind::*;
        assert_eq!(kinds("1..3"), vec![Int("1".into()), Punct(super::Punct::DotDot), Int("3".into()), Eof]);
        assert_eq!(kinds("1.0e10 2.5E-3"), vec![Float("1.0e10".into()), Float("2.5E-3".into()), Eof]);
        assert_eq!(kinds("1.0e"), vec![Float("1.0".into()), Ident("e".into()), Eof]);
        assert_eq!(kinds("1_"), vec![Int("1".into()), Ident("_".into()), Eof]);
        assert_eq!(kinds("0xFF_FF"), vec![Int("0xFF_FF".into()), Eof]);
        assert_eq!(kinds("1e10"), vec![Int("1".into()), Ident("e10".into()), Eof]);
    }

    #[test]
    fn satzzeichen() {
        use super::Punct as P;
        use TokenKind::Punct as T;
        assert_eq!(kinds("a?.b ?? c"), vec![
            TokenKind::Ident("a".into()), T(P::QuestionDot), TokenKind::Ident("b".into()),
            T(P::QuestionQuestion), TokenKind::Ident("c".into()), TokenKind::Eof]);
        assert_eq!(kinds(">>"), vec![T(P::Gt), T(P::Gt), TokenKind::Eof]);
    }
}
