#!/usr/bin/env bash
set -euo pipefail
infobar_review_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
infobar_binary="$infobar_review_dir/../codex-rs/target/debug/codex"
infobar_test_home="${CODEX_INFOBAR_TEST_HOME:-$infobar_review_dir/test-home}"
mkdir -p -- "$infobar_test_home"
if [[ ! -f "$infobar_test_home/config.toml" ]]; then
  cat > "$infobar_test_home/config.toml" <<'TOML'
[tui]
infobar = ["model-with-reasoning", "context-remaining", "five-hour-limit", "weekly-limit", "banked-resets"]
TOML
fi
exec env CODEX_HOME="$infobar_test_home" "$infobar_binary" --no-daemon "$@"
