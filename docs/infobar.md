# Top infobar

Use `/infobar` in the Codex CLI to choose and order information displayed above
the transcript. The picker uses the same keyboard bindings as `/statusline`:
up/down to navigate, Space to toggle, left/right to reorder, Enter to save, and
Esc to cancel. Clear every selection and save to hide the bar.

The bar is opt-in. On first opening the picker, model with reasoning, context
remaining, primary and secondary usage meters, and banked resets are selected.
Saving persists the ordered identifiers under `[tui].infobar` in `config.toml`.
Canceling leaves the current selection untouched.

```toml
[tui]
infobar = ["model-with-reasoning", "context-remaining", "five-hour-limit", "weekly-limit", "banked-resets", "primary-reset", "secondary-reset"]
```

All `/statusline` field identifiers and aliases also work in the infobar.
Additional identifiers are:

| Identifier        | Value                                                  |
| ----------------- | ------------------------------------------------------ |
| `banked-resets`   | Available usage resets reported by the account service |
| `primary-reset`   | Primary account usage window reset time                |
| `secondary-reset` | Secondary account usage window reset time              |

Context fields include the current context token count and capacity, such as
`Context 100K/256K [██████░░░░] 64% left`. The numerator is the latest active
context, rather than accumulated session usage, and updates after compaction.
The existing percentage calculation is preserved, including its reserved token
baseline; it can differ from the raw token ratio. `context-used` displays the
same counts with the percentage used. Until both usage and capacity are known,
the field reads `Context unknown`; pending token updates read `Context pending`.
Invalid usage or capacity reads `Context unavailable`. A measured zero still
displays `0/capacity` and its actual percentage.

Context and usage-limit fields include a ten-cell meter at wider widths. Meter
lengths shrink down to two cells according to measured terminal display widths.
Widths between full and compact layouts distribute spare columns one cell at
a time. When all minimum-size meters no longer fit, quota meters disappear
individually, retaining the context meter longest. Text values remain in their
configured positions. Other meters do not grow as the terminal shrinks.
Once even the context meter no longer fits, the bar switches to text-only
groups, first on one row and then wrapped between complete groups. Expanding
restores meters automatically. Colors, labels, percentages, and selected order
stay consistent across these formats; `NO_COLOR` retains readable text and
meter glyphs without foreground or background colors.
Limit labels follow the actual account window duration,
including daily limits. Unavailable information is omitted; an unknown reset
count is distinct from a known count of zero. The bar reads existing account
refreshes and never consumes resets. Use `/usage` to inspect or spend a reset.

In the default fullscreen transcript, the bar stays pinned at the top when
scrolling or resizing. In inline scrollback mode it appears above the current
live viewport. Fields wrap in the selected order, using up to five rows. Very
narrow layouts shorten the context label while preserving its counts and
percentage. Individual text fields too wide for a row end with an ellipsis;
selections exceeding the available rows omit lower-priority metadata before
context and quota values, preserving the retained fields' order. An overflow
indicator appears when space permits. At extreme widths the token ratio can
yield to a complete labeled percentage; `Ctx` and `wk` abbreviate labels only
when needed. If no complete metric fits, an ellipsis replaces that group rather
than cutting a number or its `left`/`used` qualifier. Expanding
the shell restores the detailed layout automatically. On very short terminals
the bar yields space to the composer and transcript. Fullscreen modal
views retain their normal input ownership.

`Resets 0` means the account service reported zero available banked resets; it
does not count elapsed reset cycles. Window reset times use separate fields.
Selecting both `model-with-reasoning` and `reasoning` intentionally repeats the
effort. The `permissions` field can read `Workspace`, meaning workspace-write
permissions without network access. Use `current-dir` or `project-root` for a
directory name. These fields remain independently configurable.

The infobar shares cached state and existing refresh events with the footer and
`/usage`; resizing does not request account data. Failed account refreshes retain
the last valid values without an infobar-specific stale badge. Successful reads
with missing values omit those fields, and account changes invalidate account
data. Selecting remote workspace headlines uses the existing authenticated,
interval-limited headline refresh. There is no additional polling loop.

Automatic presentation is the only mode. Visibility and ordering use the
existing picker and configuration list; forced presentation modes are deferred.
Measured layouts are reused between height calculation and redraws until the
width or visible values change. A bounded additional layout handles short
viewports, while the native synchronized terminal draw continues to own output.

## Local testing before publishing

From `codex-rs`, run:

```bash
export RUST_MIN_STACK=8388608 # Match the repository's test runner and CI.
cargo test -p codex-tui --lib infobar
cargo test -p codex-config infobar
cargo test -p codex-tui --lib settings_picker
cargo build -p codex-cli --bin codex
./target/debug/codex -c 'tui.infobar=["model-with-reasoning","context-remaining","five-hour-limit","weekly-limit","banked-resets"]'
```

The final command launches the locally built CLI without replacing an installed
Codex executable. The command-line override lasts for that launch; using the
picker to save writes configuration normally. To keep testing separate from
your regular settings, use a separate `CODEX_HOME` directory and sign in there.
No push or pull request is needed for any of these tests.
