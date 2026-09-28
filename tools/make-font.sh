#!/usr/bin/env bash
# ソース中の文字だけを含む日本語フォントを生成する (要: uv, Noto Sans CJK)。
# 使い方: tools/make-font.sh [/path/to/NotoSansCJK-Bold.ttc]
set -euo pipefail
cd "$(dirname "$0")/.."
SRC="${1:-/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc}"
CHARS=$(mktemp)
# ASCII 全部 + ソースに出てくる文字
{ printf '%s' ' !"#$%&'"'"'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\]^_`abcdefghijklmnopqrstuvwxyz{|}~'; cat src/*.rs; } > "$CHARS"
# TTC の 0 番目 = JP
uvx --from fonttools pyftsubset "$SRC" --font-number=0 --text-file="$CHARS" \
  --output-file=assets/fonts/game-font.otf --layout-features='*' --no-hinting --desubroutinize
rm -f "$CHARS"
ls -l assets/fonts/game-font.otf
