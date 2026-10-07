//! Optional top status bar and its local `/infobar` configuration picker.
//!
//! Ordinary fields reuse the footer's identifiers and live values. Reset fields
//! read the account state already used by `/usage`; displaying them never spends
//! a reset or starts a separate polling loop.

use std::cell::Ref;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use codex_protocol::ThreadId;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::Widget;
use strum::IntoEnumIterator;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::MultiSelectItem;
use crate::bottom_pane::MultiSelectPicker;
use crate::bottom_pane::StatusLineItem;
use crate::bottom_pane::status_line_from_segments;
use crate::keymap::ListKeymap;
use crate::line_truncation::line_width;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::render::renderable::Renderable;
use crate::style::secondary_text_style;

pub(crate) const DEFAULT_INFOBAR_ITEMS: [&str; 5] = [
    "model-with-reasoning",
    "context-remaining",
    "five-hour-limit",
    "weekly-limit",
    "banked-resets",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum InfobarItem {
    Status(StatusLineItem),
    BankedResets,
    PrimaryReset,
    SecondaryReset,
}

impl fmt::Display for InfobarItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Status(item) => item.fmt(f),
            Self::BankedResets => f.write_str("banked-resets"),
            Self::PrimaryReset => f.write_str("primary-reset"),
            Self::SecondaryReset => f.write_str("secondary-reset"),
        }
    }
}

impl FromStr for InfobarItem {
    type Err = strum::ParseError;

    fn from_str(id: &str) -> Result<Self, Self::Err> {
        match id {
            "banked-resets" => Ok(Self::BankedResets),
            "primary-reset" => Ok(Self::PrimaryReset),
            "secondary-reset" => Ok(Self::SecondaryReset),
            _ => id.parse().map(Self::Status),
        }
    }
}

impl InfobarItem {
    pub(crate) fn iter() -> impl Iterator<Item = Self> {
        StatusLineItem::iter().map(Self::Status).chain([
            Self::BankedResets,
            Self::PrimaryReset,
            Self::SecondaryReset,
        ])
    }

    pub(crate) fn status_item(self) -> Option<StatusLineItem> {
        match self {
            Self::Status(item) => Some(item),
            _ => None,
        }
    }

    pub(crate) fn accent_item(self) -> StatusLineItem {
        self.status_item().unwrap_or(StatusLineItem::WeeklyLimit)
    }

    fn description(self) -> &'static str {
        match self {
            Self::Status(item) => item.description(),
            Self::BankedResets => "Available banked usage resets (omitted when unknown)",
            Self::PrimaryReset => "Primary usage limit reset time (omitted when unknown)",
            Self::SecondaryReset => "Secondary usage limit reset time (omitted when unknown)",
        }
    }
}

pub(crate) enum InfobarValue {
    Text(String),
    Meter {
        label: String,
        percent: f64,
        suffix: &'static str,
        tokens: Option<String>,
    },
}

#[derive(Clone, Copy)]
enum InfobarLabel {
    Full,
    Short,
    Hidden,
}

impl InfobarValue {
    fn text(&self, cells: usize, label_detail: InfobarLabel) -> String {
        let text = match self {
            Self::Text(text) => text.clone(),
            Self::Meter {
                label,
                percent,
                suffix,
                tokens,
            } => {
                if !percent.is_finite() {
                    return Self::sanitize(format!("{label} unavailable"));
                }
                let percent = percent.clamp(0.0, 100.0).round() as usize;
                let filled = (percent * cells + 50) / 100;
                let meter = if cells > 0 {
                    format!("[{}{}]", "█".repeat(filled), "░".repeat(cells - filled))
                } else {
                    String::new()
                };
                let label = if tokens.is_some() {
                    match label_detail {
                        InfobarLabel::Full => label.as_str(),
                        InfobarLabel::Short => "Ctx",
                        InfobarLabel::Hidden => "",
                    }
                } else {
                    label
                };
                let mut parts = Vec::new();
                if !label.is_empty() {
                    parts.push(label.to_string());
                }
                if let Some(tokens) = tokens {
                    parts.push(tokens.clone());
                }
                if !meter.is_empty() {
                    parts.push(meter);
                }
                parts.push(format!("{percent}% {suffix}"));
                parts.join(" ")
            }
        };
        Self::sanitize(text)
    }

    fn sanitize(text: String) -> String {
        text.chars()
            .map(|ch| if ch.is_control() { ' ' } else { ch })
            .collect()
    }

    pub(crate) fn full_text(&self) -> String {
        self.text(10, InfobarLabel::Full)
    }

    fn narrow_texts(&self) -> Vec<String> {
        if let Self::Text(text) = self {
            let mut texts = vec![self.full_text()];
            if let Some(state) = text.strip_prefix("Context ") {
                texts.push(Self::sanitize(format!("Ctx {state}")));
            }
            return texts;
        }
        let Self::Meter {
            label,
            percent,
            suffix,
            tokens,
        } = self
        else {
            return vec![self.full_text()];
        };
        let labels = if tokens.is_some() {
            &[
                InfobarLabel::Full,
                InfobarLabel::Short,
                InfobarLabel::Hidden,
            ][..]
        } else {
            &[InfobarLabel::Full][..]
        };
        let mut texts = labels
            .iter()
            .map(|label| self.text(0, *label))
            .collect::<Vec<_>>();
        if tokens.is_some() {
            // Raw counts are secondary to a complete labeled percentage in a
            // terminal too narrow to show both. Never cut through a numeral.
            for label in [label.as_str(), "Ctx"] {
                texts.push(
                    Self::Meter {
                        label: label.into(),
                        percent: *percent,
                        suffix,
                        tokens: None,
                    }
                    .full_text_without_meter(),
                );
            }
        }
        if label == "weekly" {
            texts.push(
                Self::Meter {
                    label: "wk".into(),
                    percent: *percent,
                    suffix,
                    tokens: None,
                }
                .full_text_without_meter(),
            );
        }
        texts
    }

    fn full_text_without_meter(&self) -> String {
        self.text(0, InfobarLabel::Full)
    }
}

struct InfobarField {
    item: InfobarItem,
    compact: Line<'static>,
    compact_width: usize,
    meters: Vec<Line<'static>>,
    narrow: Vec<Line<'static>>,
}

#[derive(Default)]
struct InfobarLayoutCache {
    width: Option<u16>,
    full: Vec<Line<'static>>,
    constrained_rows: u16,
    constrained: Vec<Line<'static>>,
}

/// Styled field variants are cached with runtime values; layout uses terminal cells.
pub(crate) struct Infobar {
    fields: Vec<InfobarField>,
    separator: Span<'static>,
    compact_width: usize,
    meter_fields: Vec<usize>,
    layout: RefCell<InfobarLayoutCache>,
}

fn infobar_colors_enabled() -> bool {
    supports_color::on_cached(supports_color::Stream::Stdout).is_some()
}

impl Infobar {
    // Bound custom selections so they cannot occupy the entire live viewport.
    const MAX_ROWS: u16 = 5;
    const MIN_METER_CELLS: usize = 2;
    const MAX_METER_CELLS: usize = 10;

    pub(crate) fn new(
        fields: impl IntoIterator<Item = (InfobarItem, InfobarValue)>,
        thread_id: Option<ThreadId>,
    ) -> Option<Self> {
        Self::new_with_colors(fields, thread_id, infobar_colors_enabled())
    }

    fn new_with_colors(
        fields: impl IntoIterator<Item = (InfobarItem, InfobarValue)>,
        thread_id: Option<ThreadId>,
        use_colors: bool,
    ) -> Option<Self> {
        let fields: Vec<_> = fields
            .into_iter()
            .filter(|(_, value)| match value {
                InfobarValue::Meter { percent, .. } => percent.is_finite(),
                InfobarValue::Text(text) => !InfobarValue::sanitize(text.clone()).trim().is_empty(),
            })
            .map(|(item, value)| {
                let full = status_line_from_segments(
                    [(item.accent_item(), value.full_text())],
                    use_colors,
                    thread_id,
                )
                .unwrap_or_default();
                let mut style = full
                    .spans
                    .first()
                    .map(|span| span.style)
                    .unwrap_or_default();
                if !use_colors {
                    style.fg = None;
                    style.bg = None;
                }
                let styled = |text| Line::from(Span::styled(text, style));
                let compact = styled(value.text(0, InfobarLabel::Full));
                let compact_width = line_width(&compact);
                InfobarField {
                    item,
                    compact,
                    compact_width,
                    meters: if matches!(value, InfobarValue::Meter { .. }) {
                        (Self::MIN_METER_CELLS..=Self::MAX_METER_CELLS)
                            .rev()
                            .map(|cells| styled(value.text(cells, InfobarLabel::Full)))
                            .collect()
                    } else {
                        Vec::new()
                    },
                    narrow: value.narrow_texts().into_iter().map(styled).collect(),
                }
            })
            .collect();
        let separator = if use_colors {
            Span::styled(" · ", secondary_text_style())
        } else {
            Span::raw(" · ")
        };
        let compact_width = fields
            .iter()
            .map(|field| field.compact_width)
            .sum::<usize>()
            + fields.len().saturating_sub(1) * crate::width::display_width(&separator.content);
        let mut meter_fields = fields
            .iter()
            .enumerate()
            .filter_map(|(index, field)| (!field.meters.is_empty()).then_some(index))
            .collect::<Vec<_>>();
        // Keep context's visual meter longest, then quotas in configured order.
        // This changes decoration only: every field stays in its selected position.
        meter_fields.sort_by_key(|index| {
            !matches!(
                fields[*index].item,
                InfobarItem::Status(StatusLineItem::ContextRemaining | StatusLineItem::ContextUsed)
            )
        });
        (!fields.is_empty()).then_some(Self {
            fields,
            separator,
            compact_width,
            meter_fields,
            layout: RefCell::default(),
        })
    }

    #[cfg(test)]
    pub(crate) fn full_line(&self) -> Line<'static> {
        self.join(
            self.fields
                .iter()
                .map(|field| field.meters.first().unwrap_or(&field.compact).clone()),
        )
    }

    fn join(&self, fields: impl IntoIterator<Item = Line<'static>>) -> Line<'static> {
        let mut spans = Vec::new();
        for field in fields {
            if !spans.is_empty() {
                spans.push(self.separator.clone());
            }
            spans.extend(field.spans);
        }
        Line::from(spans)
    }

    fn lines(&self, width: u16, max_rows: u16) -> Ref<'_, [Line<'static>]> {
        let max_rows = max_rows.min(Self::MAX_ROWS);
        if self.layout.borrow().width != Some(width) {
            let full = self.layout_lines(width, Self::MAX_ROWS);
            self.layout.replace(InfobarLayoutCache {
                width: Some(width),
                full,
                ..Default::default()
            });
        }
        let needs_constrained = {
            let cache = self.layout.borrow();
            cache.full.len() > usize::from(max_rows) && cache.constrained_rows != max_rows
        };
        if needs_constrained {
            let lines = self.layout_lines(width, max_rows);
            let mut cache = self.layout.borrow_mut();
            cache.constrained_rows = max_rows;
            cache.constrained = lines;
        }
        Ref::map(self.layout.borrow(), |cache| {
            if cache.full.len() <= usize::from(max_rows) {
                cache.full.as_slice()
            } else {
                cache.constrained.as_slice()
            }
        })
    }

    fn layout_lines(&self, width: u16, max_rows: u16) -> Vec<Line<'static>> {
        if width == 0 || max_rows == 0 {
            return Vec::new();
        }
        let width = usize::from(width);
        let max_rows = usize::from(max_rows.min(Self::MAX_ROWS));
        if self.compact_width <= width {
            let budget = width - self.compact_width;
            let minimum_cost = Self::MIN_METER_CELLS + crate::width::display_width(" []");
            let visible = self.meter_fields.len().min(budget / minimum_cost);
            let mut details = vec![None; self.fields.len()];
            if visible > 0 {
                // Grow one cell at a time once every meter fits. Below that
                // boundary, remove minimum-size meters individually without
                // growing the others as the terminal shrinks.
                let extra = if visible == self.meter_fields.len() {
                    budget - visible * minimum_cost
                } else {
                    0
                };
                let growth = (extra / visible).min(Self::MAX_METER_CELLS - Self::MIN_METER_CELLS);
                for (position, index) in self.meter_fields.iter().take(visible).enumerate() {
                    let cells =
                        (Self::MIN_METER_CELLS + growth + usize::from(position < extra % visible))
                            .min(Self::MAX_METER_CELLS);
                    details[*index] = Some(Self::MAX_METER_CELLS - cells);
                }
            }
            return vec![
                self.join(self.fields.iter().enumerate().map(|(index, field)| {
                    details[index]
                        .map(|detail| &field.meters[detail])
                        .unwrap_or(&field.compact)
                        .clone()
                })),
            ];
        }
        let mut selected = (0..self.fields.len()).collect::<Vec<_>>();
        let mut rows = self.pack_fields(width, &selected);
        while rows.len() > max_rows && selected.len() > 1 {
            // Only severe constraints remove fields. Retained fields keep their
            // configured order; context and quota values outlive metadata.
            let Some(remove) = selected
                .iter()
                .enumerate()
                .max_by_key(|(position, index)| {
                    let priority = match self.fields[**index].item {
                        InfobarItem::Status(
                            StatusLineItem::ContextRemaining | StatusLineItem::ContextUsed,
                        ) => 0,
                        InfobarItem::Status(
                            StatusLineItem::FiveHourLimit | StatusLineItem::WeeklyLimit,
                        ) => 1,
                        InfobarItem::BankedResets
                        | InfobarItem::PrimaryReset
                        | InfobarItem::SecondaryReset => 2,
                        _ => 3,
                    };
                    (priority, *position)
                })
                .map(|(position, _)| position)
            else {
                break;
            };
            selected.remove(remove);
            rows = self.pack_fields(width, &selected);
        }
        let hidden = self.fields.len() - selected.len();
        let mut lines = rows
            .into_iter()
            .map(|row| self.join(row))
            .collect::<Vec<_>>();
        if hidden > 0
            && let Some(last) = lines.last_mut()
        {
            for marker in [
                format!(" · +{hidden} more"),
                format!(" +{hidden}"),
                " …".into(),
            ] {
                if line_width(last) + crate::width::display_width(&marker) <= width {
                    last.spans.push(Span::raw(marker));
                    break;
                }
            }
        }
        lines
    }

    fn pack_fields(&self, width: usize, selected: &[usize]) -> Vec<Vec<Line<'static>>> {
        let mut rows = vec![Vec::new()];
        let mut used = 0;
        for index in selected {
            let field = &self.fields[*index];
            let line =
                if let Some(line) = field.narrow.iter().find(|line| line_width(line) <= width) {
                    line.clone()
                } else if matches!(
                    field.item,
                    InfobarItem::Status(
                        StatusLineItem::ContextRemaining
                            | StatusLineItem::ContextUsed
                            | StatusLineItem::FiveHourLimit
                            | StatusLineItem::WeeklyLimit
                    ) | InfobarItem::BankedResets
                        | InfobarItem::PrimaryReset
                        | InfobarItem::SecondaryReset
                ) {
                    // A complete metric cannot fit; a visible marker is safer than
                    // a truncated number or a missing left/used qualifier.
                    Line::from("…")
                } else {
                    truncate_line_with_ellipsis_if_overflow(field.compact.clone(), width)
                };
            let field_width = line_width(&line);
            if used > 0 && used + 3 + field_width > width {
                rows.push(Vec::new());
                used = 0;
            }
            if used > 0 {
                used += 3;
            }
            used += field_width;
            if let Some(row) = rows.last_mut() {
                row.push(line);
            }
        }
        rows
    }

    /// Leave at least one transcript row in addition to the reserved composer.
    pub(crate) fn height(&self, width: u16, available_height: u16) -> u16 {
        self.desired_height(width)
            .min(available_height.saturating_sub(1))
    }
}

impl Renderable for Infobar {
    fn desired_height(&self, width: u16) -> u16 {
        self.lines(width, Self::MAX_ROWS).len() as u16
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        for (offset, line) in self.lines(area.width, area.height).iter().enumerate() {
            Widget::render(
                line,
                Rect::new(area.x, area.y + offset as u16, area.width, 1),
                buf,
            );
        }
    }
}

pub(crate) struct InfobarSetupView {
    picker: MultiSelectPicker,
}

impl InfobarSetupView {
    pub(crate) fn new(
        selected: &[String],
        preview: BTreeMap<InfobarItem, String>,
        app_event_tx: AppEventSender,
        list_keymap: ListKeymap,
    ) -> Self {
        let mut seen = BTreeSet::new();
        let mut items = Vec::new();
        for item in selected
            .iter()
            .filter_map(|id| id.parse::<InfobarItem>().ok())
        {
            if seen.insert(item) {
                items.push(Self::select_item(item, true));
            }
        }
        for item in InfobarItem::iter() {
            if !seen.contains(&item) {
                items.push(Self::select_item(item, false));
            }
        }
        Self {
            picker: MultiSelectPicker::builder(
                "Configure Infobar".to_string(),
                Some(
                    "Select and order top bar items. Clear all items to hide the bar.".to_string(),
                ),
                app_event_tx,
            )
            .list_keymap(list_keymap)
            .items(items)
            .enable_ordering()
            .on_preview(move |items| {
                status_line_from_segments(
                    items.iter().filter(|item| item.enabled).filter_map(|item| {
                        let item = item.id.parse::<InfobarItem>().ok()?;
                        preview
                            .get(&item)
                            .map(|text| (item.accent_item(), text.clone()))
                    }),
                    infobar_colors_enabled(),
                    None,
                )
            })
            .on_confirm(|ids, tx| {
                tx.send(AppEvent::InfobarSetup {
                    items: ids.iter().filter_map(|id| id.parse().ok()).collect(),
                });
            })
            .build(),
        }
    }

    fn select_item(item: InfobarItem, enabled: bool) -> MultiSelectItem {
        MultiSelectItem {
            id: item.to_string(),
            name: item.to_string(),
            description: Some(item.description().to_string()),
            enabled,
            orderable: true,
            section_break_after: false,
        }
    }
}

impl BottomPaneView for InfobarSetupView {
    fn keymap_contexts(&self) -> crate::keymap::KeymapContextSet {
        crate::keymap::KeymapContextSet::new(crate::keymap::KeymapContext::List)
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        self.picker.handle_key_event(key_event);
    }

    fn is_complete(&self) -> bool {
        self.picker.complete
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.picker.close();
        CancellationEvent::Handled
    }
}

impl Renderable for InfobarSetupView {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.picker.render(area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.picker.desired_height(width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::RuntimeKeymap;
    use crossterm::event::KeyCode;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc::unbounded_channel;

    #[test]
    fn infobar_picker_reorders_and_confirms_without_spending_resets() {
        let (tx, mut rx) = unbounded_channel();
        let mut view = InfobarSetupView::new(
            &["model".into(), "banked-resets".into()],
            BTreeMap::new(),
            AppEventSender::new(tx),
            RuntimeKeymap::defaults().list,
        );
        view.handle_key_event(KeyCode::Right.into());
        view.handle_key_event(KeyCode::Enter.into());
        assert!(view.is_complete());
        match rx.try_recv().unwrap() {
            AppEvent::InfobarSetup { items } => assert_eq!(
                items,
                vec![
                    InfobarItem::BankedResets,
                    InfobarItem::Status(StatusLineItem::ModelName),
                ]
            ),
            event => panic!("unexpected event: {event:?}"),
        }
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn infobar_picker_can_hide_or_cancel() {
        for key in [KeyCode::Enter, KeyCode::Esc] {
            let (tx, mut rx) = unbounded_channel();
            let mut view = InfobarSetupView::new(
                &[],
                BTreeMap::new(),
                AppEventSender::new(tx),
                RuntimeKeymap::defaults().list,
            );
            view.handle_key_event(key.into());
            assert!(view.is_complete());
            if key == KeyCode::Enter {
                assert!(
                    matches!(rx.try_recv(), Ok(AppEvent::InfobarSetup { items }) if items.is_empty())
                );
            } else {
                assert!(rx.try_recv().is_err());
            }
        }
    }

    #[test]
    fn infobar_remaining_meter_clamps_and_rounds() {
        let meter = |label: &str, percent| {
            InfobarValue::Meter {
                label: label.into(),
                percent,
                suffix: "left",
                tokens: None,
            }
            .full_text()
        };
        assert_eq!(meter("5h", 74.6), "5h [████████░░] 75% left");
        assert_eq!(meter("context", -1.0), "context [░░░░░░░░░░] 0% left");
        assert_eq!(meter("weekly", 200.0), "weekly [██████████] 100% left");
        assert_eq!(meter("5h", f64::NAN), "5h unavailable");
        assert!(
            Infobar::new(
                [(
                    InfobarItem::Status(StatusLineItem::FiveHourLimit),
                    InfobarValue::Meter {
                        label: "5h".into(),
                        percent: f64::INFINITY,
                        suffix: "left",
                        tokens: None
                    },
                )],
                None
            )
            .is_none()
        );
    }

    fn example_infobar() -> Infobar {
        Infobar::new(
            [
                (
                    InfobarItem::Status(StatusLineItem::ModelWithReasoning),
                    InfobarValue::Text("gpt-test high".into()),
                ),
                (
                    InfobarItem::Status(StatusLineItem::ContextRemaining),
                    InfobarValue::Meter {
                        label: "Context".into(),
                        percent: 64.0,
                        suffix: "left",
                        tokens: Some("100K/256K".into()),
                    },
                ),
                (
                    InfobarItem::Status(StatusLineItem::FiveHourLimit),
                    InfobarValue::Meter {
                        label: "5h".into(),
                        percent: 75.0,
                        suffix: "left",
                        tokens: None,
                    },
                ),
                (
                    InfobarItem::Status(StatusLineItem::WeeklyLimit),
                    InfobarValue::Meter {
                        label: "weekly".into(),
                        percent: 40.0,
                        suffix: "left",
                        tokens: None,
                    },
                ),
                (
                    InfobarItem::BankedResets,
                    InfobarValue::Text("Resets 3".into()),
                ),
            ],
            None,
        )
        .unwrap()
    }

    #[test]
    fn infobar_responsive_layout_preserves_counts_percentages_and_order() {
        let bar = example_infobar();
        let mut snapshots = Vec::new();
        for width in [
            160, 120, 100, 99, 98, 95, 94, 90, 89, 85, 84, 80, 40, 20, 160,
        ] {
            let lines = bar.lines(width, Infobar::MAX_ROWS);
            let text = lines
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                lines
                    .iter()
                    .all(|line| line_width(line) <= usize::from(width))
            );
            assert_eq!(bar.desired_height(width), lines.len() as u16);
            let mut tail = text.as_str();
            for field in [
                "gpt-test high",
                "100K/256K",
                "64% left",
                "5h",
                "75% left",
                "weekly",
                "40% left",
                "Resets 3",
            ] {
                tail = tail
                    .split_once(field)
                    .unwrap_or_else(|| panic!("missing {field}: {text}"))
                    .1;
            }
            snapshots.push(format!("{width} columns\n{text}"));
        }
        insta::assert_snapshot!("responsive_infobar", snapshots.join("\n\n"));
    }

    #[test]
    fn infobar_short_layout_prioritizes_context_and_quota_over_metadata() {
        let bar = example_infobar();
        assert_eq!(
            bar.lines(40, 1)[0].to_string(),
            "Context 100K/256K 64% left · 5h 75% left"
        );
        assert_eq!(bar.lines(20, 1)[0].to_string(), "100K/256K 64% left …");
        assert_eq!(bar.height(20, 1), 0);
        assert_eq!(bar.height(20, 2), 1);
        assert_eq!(bar.height(20, 10), 5);
        assert_eq!(bar.height(0, 10), 0);
        let context = Infobar::new(
            [(
                InfobarItem::Status(StatusLineItem::ContextRemaining),
                InfobarValue::Meter {
                    label: "Context".into(),
                    percent: 64.0,
                    suffix: "left",
                    tokens: Some("100K/256K".into()),
                },
            )],
            None,
        )
        .unwrap();
        assert_eq!(context.lines(20, 1)[0].to_string(), "100K/256K 64% left");
    }

    #[test]
    fn infobar_shrinks_meters_then_uses_compact_and_wrapped_text() {
        let bar = example_infobar();
        let mut meter_sizes = BTreeSet::new();
        let mut heights = BTreeSet::new();
        for width in (1..=180).rev().chain(1..=180) {
            let lines = bar.lines(width, Infobar::MAX_ROWS);
            let text = lines
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n");
            assert!(!lines.is_empty());
            assert!(lines.len() <= usize::from(Infobar::MAX_ROWS));
            assert!(
                lines
                    .iter()
                    .all(|line| !line.to_string().is_empty() && !line.to_string().ends_with('·'))
            );
            if let Some((_, meter)) = text.split_once('[') {
                assert_eq!(lines.len(), 1);
                assert!(
                    (1..=3).contains(&text.matches('[').count()),
                    "{width} columns: {text}"
                );
                meter_sizes.insert(crate::width::display_width(
                    meter.split_once(']').unwrap().0,
                ));
            } else {
                heights.insert(lines.len());
            }
            assert!(
                lines
                    .iter()
                    .all(|line| line_width(line) <= usize::from(width))
            );
        }
        assert_eq!(
            meter_sizes,
            (Infobar::MIN_METER_CELLS..=Infobar::MAX_METER_CELLS).collect()
        );
        assert!(heights.contains(&1) && heights.contains(&2) && heights.contains(&5));
        let text = bar
            .lines(20, Infobar::MAX_ROWS)
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("5h 75% left"), "{text}");
        assert!(text.contains("weekly 40% left"), "{text}");
        assert!(text.contains("100K/256K 64% left"), "{text}");
        // Medium widths keep complete text groups on one row.
        let text = bar.lines(bar.compact_width as u16, 1)[0].to_string();
        assert!(!text.contains('['), "{text}");
        for value in ["100K/256K", "64% left", "75% left", "40% left", "Resets 3"] {
            assert!(text.contains(value), "{text}");
        }
    }

    #[test]
    fn infobar_intermediate_widths_change_one_meter_at_a_time() {
        let bar = example_infobar();
        let compact_width = bar.compact_width as u16;
        let mut previous_sizes = vec![0; bar.fields.len()];
        let mut expanding = Vec::new();
        for width in compact_width..=compact_width + 40 {
            let lines = bar.lines(width, 5);
            assert_eq!(lines.len(), 1);
            let text = lines[0].to_string();
            assert!(line_width(&lines[0]) <= usize::from(width));
            let groups = text.split(" · ").collect::<Vec<_>>();
            assert_eq!(groups.len(), bar.fields.len());
            let sizes = groups
                .iter()
                .map(|group| {
                    group
                        .split_once('[')
                        .and_then(|(_, meter)| meter.split_once(']'))
                        .map_or(0, |(meter, _)| crate::width::display_width(meter))
                })
                .collect::<Vec<_>>();
            for (index, (old, new)) in previous_sizes.iter().zip(&sizes).enumerate() {
                // Growing the shell never makes an existing meter shrink.
                assert!(new >= old, "{width} columns, field {index}: {text}");
                assert!(new - old <= 1 || (*old == 0 && *new == 2));
            }
            let active = sizes.iter().filter(|size| **size > 0).count();
            let budget = usize::from(width - compact_width);
            assert_eq!(active, (budget / 5).min(3));
            if active > 0 {
                assert!(sizes[1] > 0, "context meter must survive longest: {text}");
            }
            for value in ["100K/256K", "64% left", "75% left", "40% left", "Resets 3"] {
                assert!(text.contains(value), "{text}");
            }
            previous_sizes = sizes;
            expanding.push(text);
        }
        // Reversing direction uses exactly the same layout, without hysteresis
        // or a delayed resize timer that could leave an overflowing frame.
        for (width, expected) in (compact_width..=compact_width + 40).zip(expanding).rev() {
            assert_eq!(bar.lines(width, 5)[0].to_string(), expected);
        }
    }

    #[test]
    fn infobar_reuses_layout_between_measurement_and_streaming_redraws() {
        let bar = example_infobar();
        for width in [160, 120, 100, 95, 90, 80, 40, 20] {
            let lines = bar.lines(width, 5);
            let area = Rect::new(0, 0, width, lines.len() as u16);
            // Keeping the cached lines borrowed also ensures these paths do
            // not rebuild or mutate the layout on an unchanged viewport.
            for _ in 0..10 {
                assert_eq!(bar.desired_height(width), area.height);
                assert_eq!(bar.height(width, 30), area.height);
                let mut buffer = Buffer::empty(area);
                bar.render(area, &mut buffer);
                assert_eq!(bar.lines(width, area.height).as_ptr(), lines.as_ptr());
            }
        }
    }

    #[test]
    fn infobar_cached_layout_reflows_after_width_and_row_budget_changes() {
        let bar = example_infobar();
        for width in [160, 95, 94, 85, 84, 80, 40, 20, 1, 0, 20, 40, 80, 160] {
            for rows in [5, 2, 1, 0, 1, 2, 5] {
                let actual = bar
                    .lines(width, rows)
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>();
                let fresh = example_infobar()
                    .lines(width, rows)
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>();
                assert_eq!(actual, fresh, "{width} columns, {rows} rows");
                assert!(actual.len() <= usize::from(rows));
            }
        }
    }

    #[test]
    fn infobar_preserves_semantic_groups_at_extreme_widths() {
        let bar = example_infobar();
        assert!(
            bar.lines(13, 5)
                .iter()
                .any(|line| line.to_string() == "Ctx 64% left")
        );
        assert!(
            bar.lines(13, 5)
                .iter()
                .any(|line| line.to_string() == "wk 40% left")
        );
        for width in 1..=10 {
            let lines = bar.lines(width, 1);
            assert_eq!(lines.len(), 1);
            let text = lines[0].to_string();
            assert!(text.starts_with('…') && !text.contains('%'), "{text}");
        }
        for (item, value) in [
            (InfobarItem::BankedResets, "Resets 123"),
            (InfobarItem::PrimaryReset, "5h resets 12:34 PM"),
            (InfobarItem::SecondaryReset, "weekly resets Oct 12"),
        ] {
            let bar = Infobar::new([(item, InfobarValue::Text(value.into()))], None).unwrap();
            for width in 1..=30 {
                let text = bar.lines(width, 1)[0].to_string();
                assert!(text == value || text == "…", "{width}: {text}");
            }
        }
    }

    #[test]
    fn infobar_color_disabled_keeps_values_and_meter_glyphs() {
        let value = || InfobarValue::Meter {
            label: "Context".into(),
            percent: 100.0,
            suffix: "left",
            tokens: Some("0/256K".into()),
        };
        let fields = || {
            [
                (
                    InfobarItem::Status(StatusLineItem::ContextRemaining),
                    value(),
                ),
                (
                    InfobarItem::BankedResets,
                    InfobarValue::Text("Resets 0".into()),
                ),
            ]
        };
        let plain = Infobar::new_with_colors(fields(), None, false).unwrap();
        let colored = Infobar::new_with_colors(fields(), None, true).unwrap();
        for width in [160, 40, 20, 13] {
            let lines = plain.lines(width, 5);
            assert_eq!(
                lines.iter().map(ToString::to_string).collect::<Vec<_>>(),
                colored
                    .lines(width, 5)
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            );
            assert!(
                lines
                    .iter()
                    .flat_map(|line| &line.spans)
                    .all(|span| span.style.fg.is_none() && span.style.bg.is_none())
            );
        }
        assert!(
            plain
                .full_line()
                .to_string()
                .contains("[██████████] 100% left")
        );
    }

    #[test]
    fn infobar_clears_stale_cells_when_values_contract_from_wrapped_to_one_row() {
        let area = Rect::new(0, 0, 40, 5);
        let mut buf = Buffer::empty(area);
        example_infobar().render(area, &mut buf);
        assert_ne!(buf[(0, 1)].symbol(), " ");
        let shorter = Infobar::new(
            [(
                InfobarItem::BankedResets,
                InfobarValue::Text("Resets 0".into()),
            )],
            None,
        )
        .unwrap();
        shorter.render(area, &mut buf);
        let mut fresh = Buffer::empty(area);
        shorter.render(area, &mut fresh);
        assert_eq!(buf, fresh);
    }

    #[test]
    fn infobar_render_stays_within_its_area_at_small_widths_and_heights() {
        let bar = example_infobar();
        for width in [0, 1, 8, 20, 40, 80, 160] {
            for height in [0, 1, 3, 5] {
                let mut buf = Buffer::empty(Rect::new(0, 0, width + 4, height + 4));
                bar.render(Rect::new(2, 2, width, height), &mut buf);
                for y in 0..height + 4 {
                    for x in 0..width + 4 {
                        if x < 2 || x >= width + 2 || y < 2 || y >= height + 2 {
                            assert_eq!(buf[(x, y)].symbol(), " ");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn infobar_wraps_and_truncates_using_terminal_cells() {
        let bar = Infobar::new(
            [
                (
                    InfobarItem::Status(StatusLineItem::WorkspaceHeadline),
                    InfobarValue::Text("界界界界".into()),
                ),
                (
                    InfobarItem::BankedResets,
                    InfobarValue::Text("Resets 3".into()),
                ),
            ],
            None,
        )
        .unwrap();
        assert_eq!(
            bar.lines(8, 5)
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["界界界界", "Resets 3"]
        );
        assert_eq!(bar.lines(7, 5)[0].to_string(), "界界界…");
    }

    #[test]
    fn infobar_handles_long_unicode_model_and_workspace_at_every_width() {
        let bar = Infobar::new(
            [
                (
                    InfobarItem::Status(StatusLineItem::ModelWithReasoning),
                    InfobarValue::Text(format!("{} high", "模型🦀e\u{301}".repeat(80))),
                ),
                (
                    InfobarItem::Status(StatusLineItem::CurrentDir),
                    InfobarValue::Text(format!("/工作/{}", "👩‍💻目录".repeat(80))),
                ),
                (
                    InfobarItem::Status(StatusLineItem::ContextRemaining),
                    InfobarValue::Meter {
                        label: "Context".into(),
                        percent: 0.0,
                        suffix: "left",
                        tokens: Some("256K/256K".into()),
                    },
                ),
            ],
            None,
        )
        .unwrap();
        for width in 1..=240 {
            for rows in 1..=5 {
                let lines = bar.lines(width, rows);
                assert!(!lines.is_empty() && lines.len() <= usize::from(rows));
                assert!(
                    lines
                        .iter()
                        .all(|line| line_width(line) <= usize::from(width))
                );
                if width >= 20 && rows == 1 {
                    assert!(lines[0].to_string().contains("256K/256K 0% left"));
                }
            }
        }
    }

    #[test]
    fn infobar_accepts_footer_aliases_and_reset_fields() {
        assert_eq!(
            "model-name".parse::<InfobarItem>(),
            Ok(InfobarItem::Status(StatusLineItem::ModelName))
        );
        assert_eq!(
            "banked-resets".parse::<InfobarItem>(),
            Ok(InfobarItem::BankedResets)
        );
        assert!("unknown".parse::<InfobarItem>().is_err());
    }
}
