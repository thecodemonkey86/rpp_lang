"""Bedingte Kompilierung für R++: löst #if / #elif / #else / #endif auf.

Bedingungen bestehen aus Symbolen, !, &&, || und Klammern, z. B.
    #if debug && !windows

Vordefiniert ist genau eines von debug oder release sowie das Betriebssystem
(windows, linux, macos); weitere Symbole kommen aus der Projektdatei.
Symbole können im Quelltext nicht definiert werden (kein #define).

Direktiven und die Zeilen inaktiver Zweige werden durch Leerzeilen ersetzt,
damit Zeilennummern in Fehlermeldungen stimmen. Inaktive Zweige werden nicht
geprüft. Die Bedingungen von Direktiven werden immer geprüft, auch in
inaktiven Zweigen.
"""

import re


class DirectiveError(Exception):
    pass


_TOKEN = re.compile(r"\s*(?:(&&|\|\||!|\(|\))|([A-Za-z_][A-Za-z_0-9]*))")


def _tokenize(text, number):
    tokens, pos = [], 0
    text = text.rstrip()
    while pos < len(text):
        match = _TOKEN.match(text, pos)
        if not match:
            raise DirectiveError(f"Zeile {number}: ungültige Bedingung {text!r}")
        tokens.append(match.group(1) or match.group(2))
        pos = match.end()
    return tokens


def evaluate(text, symbols, number):
    """Wertet eine Bedingung aus; && bindet stärker als ||."""
    tokens = _tokenize(text, number)
    pos = 0

    def peek():
        return tokens[pos] if pos < len(tokens) else None

    def take():
        nonlocal pos
        token = peek()
        pos += 1
        return token

    def or_expr():
        value = and_expr()
        while peek() == "||":
            take()
            right = and_expr()
            value = value or right
        return value

    def and_expr():
        value = unary()
        while peek() == "&&":
            take()
            right = unary()
            value = value and right
        return value

    def unary():
        token = take()
        if token == "!":
            return not unary()
        if token == "(":
            value = or_expr()
            if take() != ")":
                raise DirectiveError(f"Zeile {number}: ')' fehlt")
            return value
        if token is None or token in ("&&", "||", ")"):
            raise DirectiveError(f"Zeile {number}: Symbol erwartet")
        return token in symbols

    if not tokens:
        raise DirectiveError(f"Zeile {number}: Bedingung fehlt")
    result = or_expr()
    if pos != len(tokens):
        raise DirectiveError(f"Zeile {number}: unerwartetes {tokens[pos]!r}")
    return result


def preprocess(source: str, symbols: set[str]) -> str:
    out = []
    # je Ebene: {"outer": Umgebung aktiv, "active": Zweig aktiv,
    #            "taken": ein Zweig war schon aktiv, "else": #else gesehen}
    stack = []

    def all_active():
        return all(level["active"] for level in stack)

    for number, line in enumerate(source.split("\n"), start=1):
        stripped = line.strip()
        if not stripped.startswith("#"):
            out.append(line if all_active() else "")
            continue
        match = re.match(r"#([A-Za-z]+)\s*(.*)$", stripped)
        directive = match.group(1) if match else stripped
        rest = match.group(2) if match else ""
        rest = re.sub(r"\s*//.*$", "", rest)        # Zeilenkommentar nach einer Direktive
        if directive == "if":
            outer = all_active()
            value = evaluate(rest, symbols, number)
            stack.append({"outer": outer, "active": outer and value,
                          "taken": value, "else": False, "line": number})
        elif directive == "elif":
            if not stack or stack[-1]["else"]:
                raise DirectiveError(f"Zeile {number}: #elif ohne passendes #if")
            level = stack[-1]
            value = evaluate(rest, symbols, number)
            level["active"] = level["outer"] and not level["taken"] and value
            level["taken"] = level["taken"] or value
        elif directive == "else":
            if not stack or stack[-1]["else"] or rest:
                raise DirectiveError(f"Zeile {number}: #else ohne passendes #if")
            level = stack[-1]
            level["active"] = level["outer"] and not level["taken"]
            level["taken"] = True
            level["else"] = True
        elif directive == "endif":
            if not stack or rest:
                raise DirectiveError(f"Zeile {number}: #endif ohne passendes #if")
            stack.pop()
        else:
            raise DirectiveError(f"Zeile {number}: unbekannte Direktive #{directive}")
        out.append("")
    if stack:
        raise DirectiveError(f"Zeile {stack[-1]['line']}: #if ohne #endif")
    return "\n".join(out)
