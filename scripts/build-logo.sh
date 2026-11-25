#!/bin/sh
set -e
cd "$(dirname "$0")/.."
BLUE="#7091AC"
LIGHT="#9DB6CA"
MARK="(F.bb_(neg neg neg T.up.small)).arrow.square"
WORD="F.bb.square neg u.up.tilde_(neg u.up.tilde.turn) neg c.frak neg neg T.up neg join(o.up, r.up.arrow)"
cargo build -q --release -p functor-cli
BIN=target/release/functor
paint() {
  sed -e "s/ class=\"fn-math\"//" -e "s/ style=\"[^\"]*\"//" -e "s/fill=\"currentColor\"/fill=\"$1\"/" -e "s/stroke=\"currentColor\"/stroke=\"$1\"/g" -e "s/aria-label=\"[^\"]*\"/aria-label=\"Functor\"/"
}
$BIN glyph "$MARK" | paint "$BLUE" > docs/logo/mark.svg
$BIN glyph "$WORD" | paint "$BLUE" > docs/logo/wordmark.svg
$BIN glyph "$WORD" | paint "#ffffff" > docs/logo/wordmark-white.svg
$BIN glyph "$MARK" | paint "#ffffff" > docs/logo/mark-white.svg
$BIN glyph "$WORD" | paint "$LIGHT" > web/public/wordmark-dark.svg
cp docs/logo/wordmark.svg web/public/wordmark.svg 2>/dev/null || { mkdir -p web/public && $BIN glyph "$WORD" | paint "$LIGHT" > web/public/wordmark-dark.svg
cp docs/logo/wordmark.svg web/public/wordmark.svg; }
INNER=$(cat docs/logo/mark-white.svg)
printf '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect width="64" height="64" rx="14" fill="%s"/><svg x="8" y="8" width="48" height="48">%s</svg></svg>\n' "$BLUE" "$INNER" > docs/logo/icon.svg
cp docs/logo/icon.svg web/public/favicon.svg
echo "wrote docs/logo/*.svg, web/public/wordmark.svg and web/public/favicon.svg"
