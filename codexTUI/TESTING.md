# Local validation — adaptive `/infobar` polish

Initially validated on 2026-10-05; resize smoothing verified on 2026-10-06 in `/home/vfs/codex-infobar`, branch `feat/top-infobar`,
based on upstream commit `3e238776e857eccd3bde6bff3026e2e9798f6524`.

Build/test environment: Rust 1.95.0 in `rust:1.95.0-bookworm` Docker, target
`x86_64-unknown-linux-gnu`. The local native CLI and PTY harness run on AlmaLinux
10.2 x86_64. Debug info and incremental compilation were disabled to limit disk
usage (`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`,
`CARGO_INCREMENTAL=0`). Tests use the repository's 8 MiB stack setting.

## Final results

| Check                                        | Result                                                                                                                                                                                           |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Selected TUI baseline, before polish         | PASS: 452 tests, zero failures                                                                                                                                                                   |
| Final selected TUI regression suite          | PASS: 462 tests, zero failures; 5,272 other tests filtered out; 11.59 s test execution                                                                                                           |
| Additional shared usage/headline regressions | PASS: 164 tests, zero failures; 5,567 filtered out; 5.66 s; overlaps some tests in the main selection                                                                                            |
| Infobar config deserialization/order/disable | PASS: one test, zero failures; 353 filtered out                                                                                                                                                  |
| Clippy for TUI library and tests             | PASS: exit 0; one existing dependency warning in `app-server/src/message_processor.rs:372` (`clippy::let_and_return`); no infobar lint errors                                                    |
| Native CLI build                             | PASS: `codex-cli` binary, exit 0                                                                                                                                                                 |
| Rust formatting                              | PASS: `cargo fmt --all --check`, exit 0; stable rustfmt warns about the existing nightly-only `imports_granularity` option                                                                       |
| Changed Markdown formatting                  | PASS: Prettier checks all three docs                                                                                                                                                             |
| Patch whitespace                             | PASS: both worktree and staged `git diff --check`                                                                                                                                                |
| Patch applicability                          | PASS: full patch applies to recorded HEAD in an isolated temporary Git index; reverse check against local tree passes                                                                            |
| Schema consistency inspection                | PASS: generated `Tui.infobar` is an array of string identifiers; no config-format change in this polish                                                                                          |
| Native CLI startup                           | PASS: `--version` reports `codex-cli 0.0.0`; `--help` exits 0                                                                                                                                    |
| Color PTY streaming/input/resize smoke       | PASS: 25 settled size changes plus 42 rapid resizes, draft retained, picker opens; exactly two conversation requests and the existing automatic-title request, no extra requests during resizing |
| NO_COLOR PTY streaming/input/resize smoke    | PASS: same matrix; header cells have default foreground/background colors                                                                                                                        |
| Review recording/artifacts                   | PASS: 18 actual native terminal captures rendered to three PNGs and a 20-second GIF replay                                                                                                       |

During development, the first test run exposed fixture setup errors and expected
snapshot changes. The reviewed snapshots were updated and the fixtures corrected.
Clippy also caught two production `Option::unwrap` calls; those were replaced
with guarded handling and verified by the final checks above. The initial PTY
harness incorrectly treated native automatic title generation as an unexpected
model request; it now classifies that pre-existing request independently.
No unresolved infobar test, lint, build, format, or smoke failure remains.

## Exact Rust/documentation checks

The commands below ran inside container `codex-infobar-build` at
`/work/codex-rs`, mounted from the repository's `codex-rs` directory:

```bash
export RUST_MIN_STACK=8388608
cargo test -p codex-tui --lib -j 4 -- infobar settings_picker rate_limits status_surface status_line terminal_title owned_transcript slash_command local_settings rendering
cargo test -p codex-tui --lib -j 4 -- usage workspace_headline
cargo test -p codex-config infobar
cargo clippy -p codex-tui --lib --tests -j 4
cargo build -p codex-cli --bin codex -j 4
cargo fmt --all --check
```

Formatting was applied first with `cargo fmt --all`; the final check passed on
the completed patch. The final lint ran after the last source change. Logs are
copied to [logs](verification-logs), including the passing tests, configuration, lint, build,
formatting, and original baseline results.

From the repository root on the host:

```bash
npx --no-install prettier --check docs/config.md docs/infobar.md docs/slash_commands.md
git diff --check
git diff --cached --check
codex-rs/target/debug/codex --version
codex-rs/target/debug/codex --help
```

Patch checking used `git read-tree HEAD` with a temporary `GIT_INDEX_FILE`, then
`git apply --cached --check /home/vfs/codex-infobar-review/top-infobar.patch`.
`git apply --reverse --check` verified the corresponding local working tree.
The actual index and source tree were not altered by these checks.

## Dimensions and behavior covered

- Pure layout: every column width 1–180 in shrink and expand order; dynamic bar
  lengths 10–2, one-cell growth, individual meter removal, compact single row, wrapped text, severe field prioritization,
  and complete left/used qualifiers. Snapshot widths: 160/120/100/99/98/95/94/90/89/85/84/80/40/20/160.
  Intermediate-width tests additionally sweep 85–125 in both directions.
  Cache tests verify layout reuse and correct width/row-budget invalidation.
- Long Unicode model/directory values: every width 1–240 with row budgets 1–5;
  CJK, emoji/ZWJ, combining accents, no overflow or panic.
- Bounds: width 0/1/8/20/40/80/160 with height 0/1/3/5; no painting outside the
  assigned rectangle. Header heights never exceed five rows.
- Owned transcript: 160/80/40/20 columns at height 24, height 3, restoration to
  160, scrolling, cursor inside the composer and preserved draft.
- Inline: 80/40/20/80, composer priority in short viewports.
- Streaming/key entry: 160/110/107/106/102/101/97/96/90/40/20/40/90/96/97/101/102/106/107/110/160; native delta/commit
  paths, typed draft, unchanged footer, no draft submission.
- Native PTY: widths 160/80/50/49/48/47/46/45/44/40/24/22/20/22/24/40/80/160
  at height 30; heights 8/3/2/1/3/8/30 at width 40, then restore 160×30. A separate burst rapidly shrinks from 60 to 40 columns
  and expands from 40 to 60 (42 resizes), then checks the settled frame.
- Data: measured zero, 100%, exhausted/over-capacity context, context-used,
  reserved-baseline percentage, active versus session usage, compaction,
  unknown/pending/invalid context, delayed quota/reset values, refresh errors,
  missing reset summaries, stale responses, account changes, clear/hide, actual
  duration labels, reset timing, multiline sanitization, unavailable/nonfinite
  values. Reset counts and times never truncate mid-value.
- Cell clearing: wrapped header contracts to one row, buffer equals a fresh
  rendering; fullscreen redraw snapshots check restored transcript rows.
- Color: explicit plain-style tests plus an actual CLI process launched with
  `NO_COLOR=1`. No-color assertions apply to the infobar itself.

## Native smoke and visual artifacts

```bash
/home/vfs/codex-infobar-review/terminal-env/bin/python /home/vfs/codex-infobar-review/smoke_terminal.py
/home/vfs/codex-infobar-review/terminal-env/bin/python /home/vfs/codex-infobar-review/smoke_terminal.py --no-color
/home/vfs/codex-infobar-review/terminal-env/bin/python /home/vfs/codex-infobar-review/render_recording.py
```

The harness uses separate test homes, a read-only sandbox, `--no-daemon`, and a
local HTTP Responses fixture on `127.0.0.1`. It submits only synthetic prompts
to that local provider and types an unfinished draft while a second response
streams. It does not spend live model/account quota. The existing title request
also goes to the same local provider. Request summaries contain only paths and
classifications, not request bodies or account data.

The fixture reports 100K tokens. Native fallback model metadata makes 95% of
configured 256K usable, so the CLI correctly shows `100K/243K` and `62% left`.
Unit snapshots supply an explicit usable 256K capacity and correctly show 64%.
Account quotas/resets are unavailable in this unsigned fixture and omitted;
those metrics are covered by native state/event tests, not fabricated into
the terminal demonstration.

`color/terminal-resize.txt`, `no-color/terminal-resize.txt`, raw `.ansi` captures,
and `frames.json` record actual terminal output. `infobar-full.png`,
`infobar-compact.png`, `infobar-wrapped.png`, and `infobar-resize.gif` render
those captured native terminal cells. The GIF is a 20-second replay of sampled
frames, not a timing-accurate screen recording. The rendering font does not
include block/shade glyphs, so the artifact renderer draws the captured `█`
and `░` cells directly. Runtime Codex rendering is unchanged.
`RESPONSIVE-PREVIEW.txt` exports the actual all-metrics unit snapshot;
`RESPONSIVE-BASELINE.txt` preserves the earlier layout for comparison.

## Scope and remaining limitations

Native feature patch against the recorded upstream base (excluding this review folder): 31 files, 2,664 additions, 32 deletions.
This polish relative to the inspected existing feature: nine files, 925
additions, 134 deletions, including tests, two snapshots and documentation.
Cargo manifests and lockfile are unchanged; no runtime dependencies were added.

Windows/macOS/other architectures, alternate terminal emulators/fonts, screen
readers and a signed-in live account were not tested. The full workspace and
all 5,734 TUI tests were not run; the selected 462-test suite plus the 164-test
shared-state selection cover affected and neighboring behavior. Quota/reset absence is represented by omission without
inventing an unsupported-versus-loading cause. Last-good quota data can remain
visible after refresh failures without an infobar-specific stale badge.
Terminals too narrow for a complete metric show an ellipsis; terminals too short
for input and transcript cause the header to yield its area. Forced presentation
modes remain optional follow-up work. No upstream approval or acceptance is
claimed.

## Try the local build before publishing

```bash
/home/vfs/codex-infobar-review/run-local.sh login
/home/vfs/codex-infobar-review/run-local.sh
```

The launcher uses `/home/vfs/codex-infobar-review/test-home`, retaining its
existing settings. It does not replace the installed Codex binary. `/infobar`
opens the picker, Enter saves, Esc cancels, and clearing all fields hides it.
Use `/status` and `/usage` for a live-account comparison after signing in.
The native implementation is committed locally. `top-infobar.patch` is the complete
feature and `infobar-polish.patch` is the refinement of the inspected baseline.
The native feature has been committed locally and a public fork exists at
https://github.com/vfs90/codex. Branch publication status is recorded in README.md.
No upstream issue or pull request was created.

## Resize smoothing follow-up

[SMOOTHING.md](SMOOTHING.md) records the 2026-10-06 changes. Latest logs are
`verification-logs/smoothing-tui-final.log`, `verification-logs/smoothing-clippy.log`,
`verification-logs/smoothing-build.log`, and `verification-logs/smoothing-format.log`; native smoke
results are in `smoke-color-smoothing.log` and `smoke-no-color-smoothing.log`.
The new tests first exposed a missing fixture import and the expected snapshot
difference; both were corrected. A run that started before the reviewed
snapshot was installed is preserved as `smoothing-tui-snapshot-race.log`.
The final selected suite passes all 462 tests. The smoothing diff relative to
the previous polished implementation is `infobar-smoothing.patch` (four files,
275 additions, 28 deletions). Historical pre-smoothing copies remain in the original local review directory;
this package includes the current reports and selected verification logs.

PTY checks validate terminal output and input state; they do not measure visible
flicker or frame timing in a particular terminal emulator. Try the rebuilt local
CLI in your own shell to assess the visual result.

## Portable review package

The paths above record the original validation environment. For a fresh clone,
use the portable build, launcher, and loopback commands in [README.md](README.md).
User-provided screenshots and the sampled recording preview supplement the
controlled synthetic captures; they do not expand the automated account/platform
verification claims.
