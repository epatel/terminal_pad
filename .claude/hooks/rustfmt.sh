#!/bin/sh
# PostToolUse (Edit|Write): rustfmt the edited .rs file so `make lint`'s
# fmt-check stays clean. Fails open: missing jq/rustfmt or a parse error
# never blocks an edit.
command -v jq >/dev/null 2>&1 || exit 0
f=$(jq -r '.tool_input.file_path // empty')
case "$f" in
  *.rs) ;;
  *) exit 0 ;;
esac
[ -f "$f" ] || exit 0
rf=$(command -v rustfmt || echo "$HOME/.cargo/bin/rustfmt")
[ -x "$rf" ] || exit 0
"$rf" --edition 2024 "$f" >/dev/null 2>&1 || true
exit 0
