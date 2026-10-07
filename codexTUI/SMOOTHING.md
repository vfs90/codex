# Infobar resize smoothing — 2026-10-06

The full and compact appearances are preserved. Intermediate widths now lose
meter decoration progressively, and unchanged redraws reuse their layout.

Before this follow-up, each height calculation and render could join and measure
nine candidate lines. All minimum-size meters also disappeared together. The
new implementation measures field widths when values change and computes the
single-row meter budget directly. Once every meter fits, extra columns grow
meters one cell at a time. At smaller budgets, minimum-size meters disappear
individually, retaining context longest and quota meters in configured order.
Retained meters never grow while the terminal shrinks with values unchanged.
Every selected field, percentage, token count, label, and color stays in order;
only its meter decoration changes during this transition.

`InfobarLayoutCache` retains the natural layout for the current width and at
most one additional layout for a constrained row budget. Height calculation
and rendering share those lines. A width change replaces the cache; changed
values already rebuild the infobar through the existing native refresh path.
Text fields no longer store nine identical meter variants. Output remains
inside Codex's existing synchronized draw, with no resize delay, new frame
timer, network polling, configuration format, or dependency.

Main sources:

- `codex-rs/tui/src/infobar.rs`: measured budget, cached layouts, three new tests.
- `codex-rs/tui/src/chatwidget/rendering_tests.rs`: known context and both quota
  meters while streaming, typing a draft, shrinking, and expanding.
- `codex-rs/tui/src/snapshots/codex_tui__infobar__tests__responsive_infobar.snap`:
  reviewed intermediate appearances and unchanged full/wrapped styles.
- `docs/infobar.md`: progressive decoration and cache behavior.

## Before and after

This fixture has 85 columns of complete text. These are measured examples,
not global breakpoints; real values and configuration determine the boundary.

| Columns | Previous polished implementation | Current implementation                |
| ------- | -------------------------------- | ------------------------------------- |
| 160     | All meters: ten cells            | Same                                  |
| 120     | All meters: eight cells          | Context/5h/weekly: 9/9/8 cells        |
| 100     | All meters: two cells            | Same                                  |
| 99–95   | All meters disappear             | Context and 5h retain two-cell meters |
| 94–90   | Text only                        | Context retains its two-cell meter    |
| 89–85   | Text only, one row               | Same                                  |
| 84–80   | Text wraps, two rows             | Same                                  |
| 40      | Three compact rows               | Same                                  |
| 20      | Five complete groups             | Same                                  |

[RESPONSIVE-PREVIEW.txt](RESPONSIVE-PREVIEW.txt) exports the actual current
snapshot. `infobar-smoothing.patch` contains this follow-up against the
previously inspected polished implementation: four files, 275 additions and
28 deletions, including tests and the expanded snapshot. The complete feature
against the recorded upstream HEAD has 31 files, 2,664 additions and 32 deletions.

## Verification

All final checks passed:

- Selected TUI suite: **462 passed, zero failures**, 5,272 filtered out; 11.59 s
  execution. Includes every width 1–180 in both directions, Unicode values
  across widths 1–240 and row budgets 1–5, short/zero heights, colors, stale-cell
  clearing, prompt/footer preservation, and measured context semantics.
- New intermediate-width test: every width 85–125, then reversed, checks
  monotonic meter size, individually added/removed meters, stable field order,
  complete values, and no overflow. No layout depends on resize history.
- Cache tests: repeated height/render reuse at 160/120/100/95/90/80/40/20;
  width changes including 0/1, and row budgets 5/2/1/0/1/2/5, compared with a
  fresh layout to catch stale or constrained output.
- Native streaming/key-entry test: 160/110/107/106/102/101/97/96/90/40/20,
  then expansion back through the same intermediate region to 160.
- `cargo clippy -p codex-tui --lib --tests -j 4`: exit 0. The existing unrelated
  `app-server/src/message_processor.rs:372` warning remains.
- `cargo build -p codex-cli --bin codex -j 4`: exit 0, 49.35 s.
- `cargo fmt --all --check`, Prettier on `docs/infobar.md`, and patch whitespace:
  exit 0. Stable rustfmt retains the existing nightly-option warning.
- Native CLI PTY runs with color and `NO_COLOR=1`: **25 settled size changes
  plus 42 rapid resizes each** while a loopback fixture streams. The draft
  survives, measured context remains visible after settling, the picker opens,
  and resizing produces no extra model requests. Short-height checks include
  8/3/2/1 rows and restoration.
- Latest captures were rendered into the three PNGs and the 20-second GIF replay.
  The GIF replays sampled frames; it does not reproduce measured frame timing.

Exact commands, environment, logs, and the broader implementation's previous
checks are in [TESTING.md](TESTING.md). A missing test-fixture import and expected
snapshot mismatch were corrected during development; the final suite is green.

The native fixture reports synthetic context usage. Unsigned account quota and
reset fields remain absent; the three-meter transitions are exercised by native
state/rendering tests and snapshots. PTY checks verify output and input state,
not optical flicker or display latency in a specific terminal emulator. No
frame-rate improvement is claimed. Windows/macOS and a live authenticated
account were not retested.

## Try it locally

Restart the locally built CLI using:

```bash
/home/vfs/codex-infobar-review/run-local.sh
```

The launcher preserves its existing test-home settings. Resize while a response
streams and while entering an unfinished prompt. The installed Codex executable
is unchanged. The native feature is committed locally; see README.md for its commit and publishing status. Branch publication status is recorded
in README.md. [Feature request #51863](https://github.com/openai/codex/issues/51863)
was submitted on 2026-10-07; no pull request was created.

## Portable review package

The paths above record the original validation environment. For a fresh clone,
use the portable build, launcher, and loopback commands in [README.md](README.md).
User-provided screenshots and the sampled recording preview supplement the
controlled synthetic captures; they do not expand the automated account/platform
verification claims.
