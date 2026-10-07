# Feature request: Optional adaptive top infobar for the Codex CLI TUI

## Problem

During long Codex sessions, I want to monitor context capacity, remaining
5-hour and weekly quota, and the active model without leaving the conversation
or using the prompt/footer area for a dense status display.

The existing `/statusline` provides configurable fields at the bottom. This
proposal adds a separate, opt-in top information strip for fast visual scanning,
including small graphical meters, with independent configuration.

## Proposed behavior

- A built-in `/infobar` command opens a multi-select picker for choosing and
  ordering displayed metrics. Preferences persist through normal Codex configuration.
- The bar occupies the top of the Codex TUI and stays visible while scrolling
  in full-screen mode; the normal footer remains unchanged.
- Fields can include model/reasoning effort, context token counts and capacity,
  5-hour and weekly limits, available banked resets, reset timing, and workspace
  permissions.
- Context and quota meters communicate remaining capacity by default; an
  explicitly selected `context-used` field instead communicates usage.
- The layout responds to actual terminal columns and measured display-cell
  widths. Meters shorten from ten to two cells, then disappear individually,
  retaining context longest before switching to compact text. Whole fields wrap
  where needed. Narrow layouts prioritize context and quota information without
  truncating numbers or disturbing input. Cached layouts reduce repeated work
  during streaming redraws.
- Colors and meters aid scanning, while `NO_COLOR`, unavailable values, Unicode
  widths, unusually long names, and very short terminals have graceful fallbacks.
- No new remote API, polling loop, or runtime dependency is required. The
  prototype derives metrics from existing Codex state and authenticated account
  rate-limit refreshes. It preserves the existing statusline.

## Working native prototype

I implemented this as a native Rust TUI change against Codex commit
`3e238776e857eccd3bde6bff3026e2e9798f6524` (2026-10-03). It modifies the built-in
command/picker, configuration, metric derivation, refresh integration, and TUI
rendering, including full-screen and inline paths. The feature itself runs inside
Codex rather than depending on a terminal overlay, log parser, or separate service.

For review:

- [Implementation branch](https://github.com/vfs90/codex/tree/feat/top-infobar) and [feature documentation](https://github.com/vfs90/codex/blob/feat/top-infobar/docs/infobar.md).
- [20-second resize demo with synthetic usage](https://raw.githubusercontent.com/vfs90/codex/refs/heads/feat/top-infobar/codexTUI/infobar-resize.gif), replaying
  captured native terminal frames. This is a sampled replay, not a timing benchmark.
- [User-provided resize preview](https://raw.githubusercontent.com/vfs90/codex/refs/heads/feat/top-infobar/codexTUI/codexTUI-preview.gif). This recording illustrates the
  visual behavior; its account values were not independently verified.
- Screenshots: [large meters](https://github.com/vfs90/codex/blob/feat/top-infobar/codexTUI/large.png), [compact meters](https://github.com/vfs90/codex/blob/feat/top-infobar/codexTUI/compact.png), and
  [wrapped text](https://github.com/vfs90/codex/blob/feat/top-infobar/codexTUI/wrapped.png). Startup `Context unknown` means usage has not yet
  been measured; the synthetic demo also shows measured context counts.
- [Technical report](https://github.com/vfs90/codex/blob/feat/top-infobar/codexTUI/TECHNICAL-REPORT.md), [exact verification commands](https://github.com/vfs90/codex/blob/feat/top-infobar/codexTUI/TESTING.md),
  and [resize-smoothing results](https://github.com/vfs90/codex/blob/feat/top-infobar/codexTUI/SMOOTHING.md).
- [Native feature patch](https://github.com/vfs90/codex/blob/feat/top-infobar/codexTUI/top-infobar.patch), excluding the review assets.

The native patch spans **31 files (+2,664/−32 lines)**, including tests,
snapshots, and documentation. Review material under `codexTUI/` is separate
from that count. I understand this may be larger than desired for upstream
integration; the implementation is offered as a concrete design and technical
reference, and I welcome a smaller or differently structured implementation
by the maintainers.

## Local verification and limitations

- Selected baseline TUI tests: **452 passed**; latest selected tests: **462 passed**.
- Additional usage/headline selection: **164 passed**, partly overlapping.
- Infobar configuration test: **one passed**.
- TUI Clippy, native CLI build, Rust/Markdown formatting, patch whitespace and
  applicability checks: passed. A pre-existing app-server `let_and_return`
  warning remains.
- Width tests cover 1–240 columns plus zero-size bounds. Terminal resize,
  streaming, and input tests cover 20–160 columns and short heights. Native
  color and `NO_COLOR` runs each exercise 25 settled size changes and 42 rapid
  resizes, preserving the draft without extra model requests.

These are selected tests, not a claim that the complete repository suite
passes. Linux x86_64 has been exercised through a Debian build/test container
and AlmaLinux execution. Live authenticated account comparison, Windows/macOS,
screen readers, full-workspace tests, and performance benchmarks have not been
verified. Cached quota values currently lack an explicit stale-data badge after
failed refreshes. The user screenshots are additional visual examples rather
than controlled platform or account-validation evidence.

## Request

Would the Codex TUI maintainers consider an opt-in, adaptive top infobar as a
first-class interface surface? I am sharing a working prototype and demo to
make the design easy to evaluate, and I am happy for the team to adapt or
independently reimplement the idea. This is a feature proposal, with no PR
submitted, in keeping with the repository's
[current contribution policy](https://github.com/openai/codex/blob/main/docs/contributing.md).

Related requests:

- [#31118](https://github.com/openai/codex/issues/31118): configurable fields in
  the existing bottom status bar (closed).
- [#20012](https://github.com/openai/codex/issues/20012): optional color styling
  for the footer statusline (closed).
- [#17827](https://github.com/openai/codex/issues/17827): customizable statusline
  using an external command (open).

The specific request here is a native, persistent top strip with adaptive
graphical presentation and its own field configuration.
