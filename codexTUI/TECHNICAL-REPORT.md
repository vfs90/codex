# Codex CLI `/infobar`: implementation and upstream review report

Inspected and improved locally on 2026-10-05; resize smoothing verified on
2026-10-06. Repository:
`/home/vfs/codex-infobar`, branch `feat/top-infobar`. The native implementation is committed locally; publishing status is tracked
in README.md. No upstream issue or PR was submitted. The Python launcher, terminal harness, and visual artifacts in this
review directory are development tools outside the product patch.

## 1. Changes made

The existing native implementation and appearance were preserved. This polish
changes seven source/documentation files plus two responsive snapshots:

- `codex-rs/tui/src/infobar.rs`: measures display-cell widths, progressively
  shrinks the existing meters from ten to two cells one column at a time,
  removes meter decorations individually, then uses compact and wrapped text.
  Cached full and constrained layouts avoid rebuilding candidate lines on
  unchanged streaming redraws. Groups stay intact. Extremely constrained layouts prioritize
  context and quota values while preserving retained selection order. Metadata
  uses existing grapheme-aware ellipsis; metric numerals and qualifiers are
  never partially displayed. The header remains bounded to five rows and clears
  its allocated cells before drawing. Existing theme styling is retained;
  unsupported color output and `NO_COLOR` strip foreground/background colors.
- `codex-rs/tui/src/chatwidget/infobar.rs`: one display model feeds every layout.
  Known zero, unknown, pending, and invalid context states remain distinct.
  Invalid reset counts and nonfinite percentages cannot become fabricated zero
  values. Added account invalidation and workspace-headline refresh tests.
- `codex-rs/tui/src/chatwidget/rate_limits.rs`: explicit quota clearing also
  invalidates the shared banked-reset count. Existing account changes already
  clear both via `clear_pending_rate_limit_reset_requests`; that behavior was
  verified, not reimplemented.
- `codex-rs/tui/src/chatwidget/status_surfaces.rs`: the existing interval-limited
  workspace-headline refresh recognizes infobar selection as well as footer
  selection. Resizing remains a local layout operation.
- `codex-rs/tui/src/chatwidget/rendering_tests.rs`: exercises real streaming
  delta/commit paths, prompt key entry, resize boundaries, and unchanged footer
  rendering; existing inline composer-priority tests remain.
- `codex-rs/tui/src/app/tests/rate_limits.rs`: covers failed refreshes, missing
  reset summaries, successful recovery, and rejected old responses.
- `docs/infobar.md`: documents sizing, availability, meanings, and local testing.
  Responsive infobar and owned-transcript snapshots were reviewed and updated.

The ordered `[tui].infobar` list and existing picker still enable, disable,
toggle, and reorder fields. Automatic presentation is implemented; optional
forced modes were deferred to keep configuration and scope small. The normal
footer model indicator and explicitly selected duplicate effort were preserved.

Before/after examples use the same synthetic values in the snapshots:

| Width | Baseline on inspection    | Polished behavior                         |
| ----- | ------------------------- | ----------------------------------------- |
| 160   | Full ten-cell meters      | Same full appearance                      |
| 120   | Five-cell meters          | Nine/nine/eight-cell meters by actual fit |
| 100   | Two rows with full meters | One row with two-cell meters              |
| 90    | Wrapped meters            | One row, retaining the context meter      |
| 89    | Wrapped meters            | One compact text row                      |
| 80    | Two rows with meters      | Two compact text rows                     |
| 40    | Four rows with meters     | Three compact text rows                   |
| 20    | Five rows, some meters    | Five complete text groups                 |

These are fixture-specific measurements, not fixed global breakpoints. Real
model names, effort, count strings, selected fields, and available height change
the boundaries. The restored compact/wrapped text appearances match the user's
requested baseline aesthetics.

## 2. Verification

The baseline selected TUI suite passed 452 tests before edits. The polished
selected suite passes 462 tests; an additional shared usage/headline selection
passes 164 tests (some overlap). Initial development runs exposed new fixture
setup errors and intentional snapshot differences; these were corrected or
reviewed before the final passing run. Exact final commands, results and
platform limitations are recorded in [TESTING.md](TESTING.md). The latest
smoothing results and before/after examples are in [SMOOTHING.md](SMOOTHING.md).

Layout tests sweep every width 1–180 in shrink/expand order, and widths 1–240
with long Unicode model/directory values at row budgets 1–5. Zero width/height
rendering is safe. Owned-transcript snapshots exercise widths 160/80/40/20 and
a three-row terminal; renderer bounds tests include width 0/1/8/20/40/80/160 and
height 0/1/3/5. The native PTY harness additionally resizes while a loopback
Responses fixture streams and an unfinished draft is present.

Live authenticated account comparison, Windows/macOS, alternate terminal
emulators/fonts, screen readers, and the entire workspace test suite are not
claimed as tested. Cached quota data has no new infobar-specific age/error
indicator. Below the minimum width for a complete metric, an ellipsis is the
honest fallback; below the height needed for input, the header yields its area.

## 3. Answers for OpenAI

### 1. Native architecture or external extension?

This is a local native Rust source patch against `openai/codex`, not a runtime
plugin, operator framework, hook, wrapper, or log parser. `SlashCommand::Infobar`
in [slash_command.rs](../codex-rs/tui/src/slash_command.rs#L64)
is dispatched by `ChatWidget` through its normal command system. The runtime
header uses Ratatui `Line`, `Span`, `Buffer`, and `Renderable`. The review-only
`run-local.sh` simply launches the compiled binary with an isolated `CODEX_HOME`.

### 2. Files, entry points, attachment, lifecycle?

| Responsibility            | Source evidence                                                                                                                                    |
| ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| Command                   | `tui/src/slash_command.rs`: `SlashCommand::Infobar`; `chatwidget/slash_dispatch.rs`: dispatch and `open_infobar_setup`                             |
| Selection and preview     | `tui/src/infobar.rs`: `InfobarSetupView`, existing `MultiSelectPicker`                                                                             |
| Config persistence        | `tui/src/app_event.rs`: `InfobarSetup`; `app/event_dispatch.rs`: standard `ConfigEdit::SetPath` at `tui.infobar`                                   |
| Config resolution         | `config/src/types.rs`: `Tui::infobar`; `core/src/config/mod.rs`: `Config::tui_infobar`; `tui/src/local_settings.rs`; generated config schema       |
| Values and cache          | `tui/src/chatwidget/infobar.rs`: `infobar_value`, `refresh_infobar`; `ChatWidget::infobar: Option<Infobar>`                                        |
| Layout and paint          | `tui/src/infobar.rs`: `InfobarValue`, `InfobarLayoutCache`, `Infobar::lines`, `layout_lines`, `pack_fields`, `height`, `Renderable::render`        |
| Fullscreen top attachment | `tui/src/app/owned_transcript.rs`: `render_owned_transcript` reserves header rows and renders `Rect::new(0, 0, screen_size.width, infobar_height)` |
| Inline attachment         | `tui/src/chatwidget/rendering.rs`: `as_backdrop_renderable` inserts the infobar before active transcript flex children; composer gets space first  |
| Shared refresh            | `tui/src/chatwidget/status_surfaces.rs`: `refresh_status_surfaces` syncs shared metadata and refreshes footer, infobar, and title                  |
| Accepted account reads    | `tui/src/app/event_dispatch.rs`: `AppEvent::RateLimitsLoaded`, refresh request/generation gating                                                   |

Existing TUI resize and draw events supply the current `Rect`/screen dimensions
on every draw. There is no separate resize event loop. Fullscreen scrolling
keeps the header at y=0. Inline mode places it above the current live viewport,
not above all historical terminal scrollback. Fullscreen dashboard views retain
their existing ownership of the screen.

### 3. Fork requirement and packaging?

Users need a locally patched build while this remains outside upstream; stock
Codex cannot load these source changes as a supported plugin. A separately
maintained fork is one distribution choice, not an architectural requirement.
The complete diff is self-contained against the recorded base commit. There
are no new Cargo dependencies, backend API schemas, authentication methods,
ANSI interception, or monkey patches. Review concerns are the header's screen
allocation, additional configurable status surface, cache semantics, and policy
fit. Upstream maintainers may choose another implementation or decline it.

### 4. Base version and actually tested platforms?

The inspected upstream base is `3e238776e857eccd3bde6bff3026e2e9798f6524`, dated 2026-10-03, subject
“Show model and reasoning effort near the top of task details (#50727)”.
`git describe --tags --always` returns `3e23877`; the workspace and local binary
report `0.0.0`, a source build rather than a numbered release. Only this base
and the local patch were validated here. Rust 1.95.0, target
`x86_64-unknown-linux-gnu`, Debian bookworm Docker build/tests, and an AlmaLinux
10.2 x86_64 host running the binary and PTY harness were tested. Other releases,
Windows, macOS and other architectures remain untested. Earlier visual demos
reported by the user are not independently assigned platform/version coverage.

### 5. Metric sources?

| Metric                    | Actual source and meaning                                                                                                                                                                               |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Model                     | `ChatWidget::current_model`, existing `ModelCatalog` display names, `settings.rs::model_display_name`; no separate model lookup                                                                         |
| Effort                    | `effective_reasoning_effort`, `status_surfaces.rs::reasoning_display_name`; model-with-reasoning uses the same label and existing service-tier suffix when applicable                                   |
| Context counts/capacity/% | `token_info.last_token_usage.total_tokens`, model context window or existing config capacity; `tui/src/token_usage.rs::percent_of_context_window_remaining` preserves the reserved baseline calculation |
| 5h/weekly                 | `rate_limit_snapshots_by_limit_id["codex"]`, existing `five_hour_status_window`/`weekly_status_window` duration matching and fallbacks; `limit_label_for_window` determines the actual label            |
| Banked resets             | Shared `available_rate_limit_reset_credits`, populated from accepted `GetAccountRateLimitsResponse.rate_limit_reset_credits.available_count` and existing usage/reset flows                             |
| Reset timing              | Cached primary/secondary `resets_at`, already formatted into local time by `status/rate_limits.rs::RateLimitWindowDisplay::from_window`                                                                 |
| Literal Workspace         | `status_surfaces.rs::permissions_display`, a permission summary for workspace-write without network access                                                                                              |
| Directory/project         | `status_line_cwd` with `format_directory_display`, or cached project root; distinct optional identifiers                                                                                                |
| Workspace headline        | Existing cached authenticated workspace-message service; distinct optional identifier                                                                                                                   |

Quota reads are the existing official app-server `account/rateLimits/read` RPC,
implemented by `app-server/src/request_processors/account_processor.rs` and
`backend-client/src/client/rate_limit_resets.rs`. The backend already uses
`/api/codex/usage` or `/wham/usage`, plus reset-credit reads. This patch adds no
public API contract or calls to an undocumented endpoint of its own. No metric
is obtained by log parsing. Account-backed availability is plan/auth dependent.

### 6. Refresh, auth, stale data, errors, network?

The existing startup, periodic, recovery, `/status`, `/usage`, and reset-flow
reads feed the same caches. Existing polling is auth-gated and adapts to usage
(5/15/30/60 seconds; `chatwidget/rate_limits.rs::rate_limit_refresh_interval`).
The infobar does not add a timer or trigger quota requests on resize/key entry.
Accepted request IDs/generations protect against old account results. Account
changes already clear account caches and outstanding reset requests through
`settings.rs::update_account_state` and `usage.rs::clear_pending_rate_limit_reset_requests`.
An explicit no-snapshot update now also clears banked resets.

Unknown quota/reset fields are omitted. Context visibly distinguishes unknown,
pending, invalid/unavailable, and measured zero. Failed reads preserve cached
last-good values; a successful missing reset summary clears its value. The
header does not mark capture age or transient errors, unlike some `/status`
output. Rolling rate-limit notifications drive warnings/recovery but intentionally
do not overwrite the full account-read display cache. Optional workspace
headlines and thread cost fields can activate their existing shared authenticated
fetches, and selected git fields use existing metadata lookups; these are not
new infobar endpoints or per-resize polling.

### 7. Resets 0, the extra high, and Workspace?

`Resets 0` means zero available banked usage-reset credits reported by the
account; it is not elapsed reset cycles, reset time, or a inferred counter.
An unknown count omits the field. The observed selected identifiers include
both `model-with-reasoning` and `reasoning`, so the second `high` repeats the
same effort intentionally selected by the user. It is unrelated to resets.
`Workspace` comes from selected `permissions`; it is not the directory name.
All selections are preserved. Optional user cleanup: deselect standalone
`reasoning`, or choose `current-dir`/`project-name` if a directory label is wanted.

### 8. Remaining or used fill percentage?

For defaults, yes: context-remaining and both quota meters fill the percentage
left; quota uses `100 - used_percent`, clamped and rounded only for display.
`context-used` is an explicit supported alternative whose fill is percentage
used and whose qualifier says `used`. All size variants use the same
`InfobarValue` and qualifier. Thus it would be false to claim every configurable
meter always shows remaining capacity. Context count numerator is raw active
usage; percentages include the existing 12,000-token reserved baseline, so
they need not equal the raw count ratio. Two-cell bars have coarse visual
granularity; the accompanying percentage is retained.

Token notifications can report a usable window smaller than the configured
maximum (`protocol/src/openai_models.rs::ModelInfo::usable_context_window`).
The header preserves that authoritative capacity. For example, the native
loopback fixture's fallback model uses 95% of configured 256K, displaying
100K/243K and 62% left. The unit snapshots explicitly supply a 256K usable
window, so their same 100K count correctly shows 64% left.

### 9. Scope needed for native optional adoption?

The provided diff already supplies the native feature: renderer/value module,
builtin command/picker, standard config field and generated schema, app event
persistence, shared-state hooks, top-space allocation for both transcript modes,
tests, snapshots, and docs. It touches config, core config, and TUI source but
needs no app-server/backend changes or new crates. Exact final diff statistics
are in `TESTING.md`. The native feature patch, excluding this review folder, has 2,664 additions and 32 deletions across 31 files, including
substantial tests/snapshots/docs; the renderer/picker production module is about
half its total size. The polish itself changes nine files, with 925 additions and 134 deletions including tests, documentation and snapshots.
Maintainers would review/rebase those integrations against their current main
branch and decide defaults/UX and freshness policy. No schedule is estimated.

### 10. Compatibility, performance, accessibility and stability concerns?

Terminal and font differences can change emoji/grapheme widths despite using
the project's Ratatui-aligned Unicode helpers. Block and shade glyphs require
font support; percent text remains readable without color. Styles reuse the
existing theme/contrast policy, but screen readers, high-contrast themes and
all terminal emulators were not manually evaluated. Color capability is cached
by the project's existing `supports_color` helper at startup, so changing an
environment variable in an already running process is not a supported toggle.

Layout uses at most nine single-row meter candidates plus bounded wrapped
packing; field variants are cached with refreshed values, and height is capped
at five. No performance benchmark is claimed. Native full-frame clearing and
the header's area clear prevent stale cells; owned and inline tests exercise
layout, cursor/input, scrolling and short-height behavior. Extremely small
screens inevitably lose detail or yield all header space to input. The remaining
freshness concern is retained quota values without a header age indicator.

### 11. Difference from /statusline and existing status UI?

`/statusline` configures a compact bottom status line using shared fields and
theme styling; `/status` inserts a fuller status report, and `/usage` provides
account/reset interactions. `/infobar` is an independent opt-in top strip with
meters, token ratios, banked reset availability/timing, and responsive wrapping.
It keeps information in a fixed fullscreen location during scroll and streams,
without taking over the footer or repeatedly opening account views. The
existing footer model indicator remains unchanged.

### 12. Tests/docs and responsiveness additions?

Existing tests cover picker confirmation/reorder/hide/cancel, persistent
configuration, command behavior, metric resolution, account refresh rejection,
owned scrolling/resizing snapshots and inline composer priority. This polish
adds seven TUI test functions, extends refresh/error/context cases, sweeps all
intermediate widths in both directions, checks complete metric semantics at
extreme widths, long Unicode values, color-free styles, stale cell clearing,
account invalidation, infobar-only headline refresh and real streaming/input
paths with the footer unchanged. Three additional responsiveness tests verify individual meter transitions,
unchanged-layout reuse, and cache invalidation across width/height changes.
The streaming regression includes all three known meters and more intermediate
widths. Both responsive snapshots are updated.
`docs/infobar.md` is linked from config and slash-command docs. Review-only
terminal captures and the reproduction launcher supplement tests.

### 13. License, dependencies and provenance?

The repository and workspace package license are Apache-2.0 (`LICENSE`,
`docs/license.md`, `codex-rs/Cargo.toml`). `NOTICE` records existing MIT-derived
Ratatui code. This implementation reuses existing Ratatui, Crossterm, Strum,
supports-color, Unicode helpers, configuration APIs and pickers. Cargo manifests
and lockfile are unchanged; no third-party code was copied in this polish.
The provenance of all pre-existing repository/transitive code was not audited,
so this is not a comprehensive dependency-license certification.

Outside the patch, the optional local terminal harness uses pyte 0.8.2
(LGPLv3 according to installed metadata) and wcwidth 0.9.2 (MIT). The optional
recording renderer uses Pillow 12.3.0 (MIT-CMU). None is imported by or bundled
into Codex. Sharing the source diff does not add these development dependencies
to the product.

### 14. Smallest shareable package if external PRs are refused?

The [current contribution policy](https://github.com/openai/codex/blob/main/docs/contributing.md)
accepts feature requests and analysis through issues and excludes outside code
PRs. A concise issue can include `FEATURE-PROPOSAL.md`, base commit, concrete
before/after examples, reproducible commands, test results, and the small
`top-infobar.patch` as an implementation reference. The native terminal GIF replay,
PNGs and text captures demonstrate resize behavior; synthetic usage is labeled
and account-specific values are absent from those captures. Maintainers can
review the two main modules and integration points, then implement the idea
themselves. A public fork exists at https://github.com/vfs90/codex; this package targets
its `feat/top-infobar` branch. Publication status is recorded in README.md.

User confirmation still needed for any claimed live-account comparison,
additional platforms/terminal applications, earlier demonstration provenance,
or later publication. None was inferred from prior screenshots or comments.

## 4. Upstream-ready description

Please consider an optional top information bar for the Codex CLI. During long
responses and tool runs, users need to see the active model, reasoning effort,
context capacity, account usage windows, and available banked resets without
opening a status view or changing their existing footer configuration.

A working local implementation modifies Codex's native Rust TUI. `/infobar`
opens the existing multi-select picker to choose and order fields; selections
persist through the standard configuration mechanism. The feature is opt-in
and leaves the normal footer, input ownership, and command bindings intact.

The existing color palette and filled/hatched meters are preserved. Layout uses
measured terminal cell widths: meters progressively shorten one cell at a time, then disappear individually
before yielding to compact text and wrapping between complete groups. Cached
layouts reduce repeated work during streaming redraws. Very constrained layouts
prioritize context and quota values and reserve room for input. Expanding the
terminal restores detail automatically. Context fields retain both token
counts and the existing percentage semantics; missing data is not represented
as a measured zero.

The bar reuses Codex's internal state and existing account refreshes, with no
new polling loop. Local tests cover intermediate widths, Unicode values,
short heights, no-color rendering, refresh failures, and resizing during
streaming and prompt entry. A reviewable patch and local terminal recording
can accompany this proposal as implementation references.
