import re
import sys
import unicodedata
from pathlib import Path

root = Path(__file__).resolve().parent.parent
glyphs = root / "crates/functor-core/data/glyphs.tsv"
out = root / "crates/functor-core/data/classes.tsv"

REL = re.compile(r"ARROW|HARPOON|EQUAL|LESS-THAN|GREATER-THAN|SUBSET|SUPERSET|ELEMENT OF|CONTAINS AS|PRECEDES|SUCCEEDS|TACK|EQUIVALENT|APPROXIMATELY|PARALLEL|DIVIDES|PROPORTIONAL|ASYMPTOTICALLY|MODELS|FORCES|\bTRUE\b|SIMILAR|TILDE OPERATOR|IMAGE OF|ORIGINAL OF|NORMAL SUBGROUP|CONTAINS AS NORMAL|BETWEEN|PITCHFORK|BOWTIE|MULTIMAP|THEREFORE|BECAUSE|IDENTICAL|COLON EQUALS|EQUALS COLON|ESTIMATES|CORRESPONDS|MEASURED BY|RATIO|PERPENDICULAR|DOUBLE UP TACK|NOT SIGN XXX")
BIN = re.compile(r"PLUS|MINUS|TIMES|CIRCLED|SQUARED|INTERSECTION|UNION|LOGICAL AND|LOGICAL OR|SEMIDIRECT|DOT OPERATOR|ASTERISK OPERATOR|RING OPERATOR|DIVISION|WREATH|AMALGAMATION|SQUARE CAP|SQUARE CUP|SMASH|XOR|NAND|NOR\b|RELATIONAL COMPOSITION|BULLET OPERATOR|STAR OPERATOR|DIAMOND OPERATOR|SET MINUS|REVERSE SOLIDUS OPERATOR|DIVISION SLASH|TURNED AMPERSAND|CURLY LOGICAL")
OP = re.compile(r"^N-ARY|INTEGRAL|SUMMATION|PRODUCT OPERATOR|^DOUBLE-STRUCK N-ARY")
OPEN = re.compile(r"\bLEFT\b.*(BRACKET|PARENTHESIS|CEILING|FLOOR|CORNER|DELIMITER)|BRACKET.*\bLEFT\b")
CLOSE = re.compile(r"\bRIGHT\b.*(BRACKET|PARENTHESIS|CEILING|FLOOR|CORNER|DELIMITER)|BRACKET.*\bRIGHT\b")


def classify(cp):
    try:
        name = unicodedata.name(chr(cp))
    except ValueError:
        return None
    if cp < 0x2000 and cp not in (0xAC, 0xB1, 0xD7, 0xF7):
        return None
    if name.startswith("COMBINING ") or "LETTER" in name or re.search(r"^MATHEMATICAL .*(CAPITAL|SMALL|DIGIT|SYMBOL)", name):
        return None
    if name.startswith("PRODUCT") and "OPERATOR" not in name:
        return None
    if OPEN.search(name):
        return "L"
    if CLOSE.search(name):
        return "C"
    if OP.search(name):
        return "O"
    if REL.search(name):
        return "R"
    if BIN.search(name):
        return "B"
    if cp in (0xB1, 0xD7, 0xF7):
        return "B"
    return None


rows = []
seen = set()
for line in glyphs.read_text(encoding="utf8").splitlines():
    v, cp = line.split("\t")[:2]
    cp = int(cp, 16)
    if v != "n" or cp in seen:
        continue
    seen.add(cp)
    c = classify(cp)
    if c:
        rows.append(f"{cp:x}\t{c}")
out.write_text("\n".join(rows) + "\n", encoding="utf8")
print(f"wrote {len(rows)} classes to {out}", file=sys.stderr)
