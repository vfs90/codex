//! Infobar state derived from the same runtime values as the footer and `/usage`.

use super::*;
use crate::infobar::DEFAULT_INFOBAR_ITEMS;
use crate::infobar::Infobar;
use crate::infobar::InfobarItem;
use crate::infobar::InfobarSetupView;
use crate::infobar::InfobarValue;

impl ChatWidget {
    pub(super) fn infobar_items_with_invalids(&self) -> (Vec<InfobarItem>, Vec<String>) {
        super::status_surfaces::parse_items_with_invalids(
            self.local_settings.tui.infobar.clone().unwrap_or_default(),
        )
    }

    pub(super) fn open_infobar_setup(&mut self) {
        let selected = self.local_settings.tui.infobar.clone().unwrap_or_else(|| {
            DEFAULT_INFOBAR_ITEMS
                .iter()
                .map(ToString::to_string)
                .collect()
        });
        let preview = InfobarItem::iter()
            .filter_map(|item| {
                self.infobar_value(item)
                    .map(|value| (item, value.full_text()))
            })
            .collect();
        self.bottom_pane.show_view(Box::new(InfobarSetupView::new(
            &selected,
            preview,
            self.app_event_tx.clone(),
            self.bottom_pane.list_keymap(),
        )));
    }

    pub(crate) fn setup_infobar(&mut self, items: Vec<InfobarItem>) {
        self.local_settings.tui.infobar = Some(items.iter().map(ToString::to_string).collect());
        self.refresh_status_surfaces();
        self.request_redraw();
    }

    pub(crate) fn set_infobar_reset_count(&mut self, count: Option<i64>) {
        self.available_rate_limit_reset_credits = count;
    }

    pub(super) fn refresh_infobar(&mut self, items: &[InfobarItem], invalids: &[String]) {
        if self.thread_id.is_some() && !invalids.is_empty() && !self.infobar_invalid_items_warned {
            self.infobar_invalid_items_warned = true;
            self.on_warning(format!(
                "Ignored invalid infobar items: {}.",
                proper_join(invalids)
            ));
        }
        let segments = items
            .iter()
            .filter_map(|item| self.infobar_value(*item).map(|value| (*item, value)))
            .collect::<Vec<_>>();
        self.infobar = Infobar::new(segments, self.thread_id);
    }

    pub(super) fn infobar_value(&mut self, item: InfobarItem) -> Option<InfobarValue> {
        match item {
            InfobarItem::BankedResets => self
                .available_rate_limit_reset_credits
                .filter(|count| *count >= 0)
                .map(|count| InfobarValue::Text(format!("Resets {count}"))),
            InfobarItem::PrimaryReset | InfobarItem::SecondaryReset => {
                let secondary = item == InfobarItem::SecondaryReset;
                let display = self.rate_limit_snapshots_by_limit_id.get("codex")?;
                let window = if secondary {
                    display.secondary.as_ref()
                } else {
                    display.primary.as_ref()
                }?;
                let label = limit_label_for_window(window.window_minutes, secondary);
                window
                    .resets_at
                    .as_ref()
                    .map(|time| InfobarValue::Text(format!("{label} resets {time}")))
            }
            InfobarItem::Status(StatusLineItem::FiveHourLimit | StatusLineItem::WeeklyLimit) => {
                let secondary = item == InfobarItem::Status(StatusLineItem::WeeklyLimit);
                let display = self.rate_limit_snapshots_by_limit_id.get("codex")?;
                let (window, is_secondary) = if secondary {
                    super::status_surfaces::weekly_status_window(display)
                } else {
                    super::status_surfaces::five_hour_status_window(display)
                }?;
                let label = limit_label_for_window(window.window_minutes, is_secondary);
                Some(InfobarValue::Meter {
                    label,
                    percent: 100.0 - window.used_percent,
                    suffix: "left",
                    tokens: None,
                })
            }
            InfobarItem::Status(StatusLineItem::ContextRemaining | StatusLineItem::ContextUsed) => {
                let used = item == InfobarItem::Status(StatusLineItem::ContextUsed);
                // The footer has optimistic startup fallbacks. A measured
                // meter must have both usage and capacity, including real zero.
                if self.token_usage_pending {
                    return Some(InfobarValue::Text("Context pending".into()));
                }
                let (Some(info), Some(window)) =
                    (&self.token_info, self.status_line_context_window_size())
                else {
                    return Some(InfobarValue::Text("Context unknown".into()));
                };
                let count = info.last_token_usage.tokens_in_context_window();
                if window <= 0 || count < 0 {
                    return Some(InfobarValue::Text("Context unavailable".into()));
                }
                let remaining = info
                    .last_token_usage
                    .percent_of_context_window_remaining(window);
                let percent = if used { 100 - remaining } else { remaining };
                let tokens = Some(format!(
                    "{}/{}",
                    format_tokens_compact(count),
                    format_tokens_compact(window)
                ));
                Some(InfobarValue::Meter {
                    label: "Context".into(),
                    percent: percent as f64,
                    suffix: if used { "used" } else { "left" },
                    tokens,
                })
            }
            InfobarItem::Status(item) => self
                .status_line_value_for_item(item)
                .map(InfobarValue::Text),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
    use codex_app_server_protocol::RateLimitWindow;
    use pretty_assertions::assert_eq;

    fn text(chat: &ChatWidget) -> Option<String> {
        chat.infobar.as_ref().map(|bar| bar.full_line().to_string())
    }

    #[tokio::test]
    async fn infobar_omits_unknowns_preserves_order_and_updates_reset_count() {
        let (mut chat, _, _, _) = make_chatwidget_manual_with_sender().await;
        assert!(chat.infobar.is_none());
        chat.setup_infobar(vec![
            InfobarItem::BankedResets,
            InfobarItem::Status(StatusLineItem::CodexVersion),
        ]);
        assert_eq!(text(&chat), Some(CODEX_CLI_VERSION.into()));
        chat.set_infobar_reset_count(Some(3));
        chat.refresh_status_surfaces();
        assert_eq!(text(&chat), Some(format!("Resets 3 · {CODEX_CLI_VERSION}")));
        chat.set_infobar_reset_count(Some(0));
        chat.refresh_status_surfaces();
        assert!(text(&chat).unwrap().starts_with("Resets 0 · "));
        chat.set_infobar_reset_count(None);
        chat.refresh_status_surfaces();
        assert_eq!(text(&chat), Some(CODEX_CLI_VERSION.into()));
        chat.set_infobar_reset_count(Some(-1));
        chat.refresh_status_surfaces();
        assert_eq!(text(&chat), Some(CODEX_CLI_VERSION.into()));
        chat.setup_infobar(vec![]);
        assert!(chat.infobar.is_none());
    }

    #[tokio::test]
    async fn infobar_meters_follow_window_duration_and_reset_times() {
        let (mut chat, _, _, _) = make_chatwidget_manual_with_sender().await;
        chat.setup_infobar(vec![
            InfobarItem::Status(StatusLineItem::FiveHourLimit),
            InfobarItem::Status(StatusLineItem::WeeklyLimit),
            InfobarItem::PrimaryReset,
        ]);
        chat.on_rate_limit_snapshot(Some(RateLimitSnapshot {
            limit_id: None,
            limit_name: None,
            normal_model_slug: None,
            primary: Some(RateLimitWindow {
                used_percent: 25,
                window_duration_mins: Some(24 * 60),
                resets_at: Some(2_000_000_000),
            }),
            secondary: Some(RateLimitWindow {
                used_percent: 60,
                window_duration_mins: Some(7 * 24 * 60),
                resets_at: None,
            }),
            credits: None,
            individual_limit: None,
            plan_type: None,
            spend_control_reached: None,
            rate_limit_reached_type: None,
        }));
        let line = text(&chat).unwrap();
        assert!(
            line.starts_with(
                "daily [████████░░] 75% left · weekly [████░░░░░░] 40% left · daily resets "
            ),
            "{line}"
        );
        assert!(chat.infobar_value(InfobarItem::SecondaryReset).is_none());
    }

    #[tokio::test]
    async fn infobar_context_count_tracks_active_context_and_preserves_percent() {
        let (mut chat, _, _, _) = make_chatwidget_manual_with_sender().await;
        chat.setup_infobar(vec![InfobarItem::Status(StatusLineItem::ContextRemaining)]);
        chat.token_info = Some(TokenUsageInfo {
            total_token_usage: TokenUsage {
                total_tokens: 900_000,
                ..Default::default()
            },
            last_token_usage: TokenUsage {
                total_tokens: 100_000,
                ..Default::default()
            },
            model_context_window: Some(256_000),
        });
        chat.refresh_status_surfaces();
        assert_eq!(
            text(&chat),
            Some("Context 100K/256K [██████░░░░] 64% left".into())
        );
        chat.setup_infobar(vec![InfobarItem::Status(StatusLineItem::ContextUsed)]);
        assert_eq!(
            text(&chat),
            Some("Context 100K/256K [████░░░░░░] 36% used".into())
        );
        // Compaction replaces the active-context count, not the session total.
        chat.token_info
            .as_mut()
            .unwrap()
            .last_token_usage
            .total_tokens = 100;
        chat.refresh_status_surfaces();
        assert_eq!(
            text(&chat),
            Some("Context 100/256K [░░░░░░░░░░] 0% used".into())
        );
        chat.setup_infobar(vec![InfobarItem::Status(StatusLineItem::ContextRemaining)]);
        chat.token_info
            .as_mut()
            .unwrap()
            .last_token_usage
            .total_tokens = 300_000;
        chat.refresh_status_surfaces();
        assert_eq!(
            text(&chat),
            Some("Context 300K/256K [░░░░░░░░░░] 0% left".into())
        );
        chat.token_usage_pending = true;
        chat.refresh_status_surfaces();
        assert_eq!(text(&chat), Some("Context pending".into()));
    }

    #[tokio::test]
    async fn infobar_context_count_does_not_invent_unknown_capacity() {
        let (mut chat, _, _, _) = make_chatwidget_manual_with_sender().await;
        chat.config.model_context_window = None;
        chat.setup_infobar(vec![InfobarItem::Status(StatusLineItem::ContextRemaining)]);
        assert_eq!(text(&chat), Some("Context unknown".into()));
        chat.config.model_context_window = Some(1_000_000);
        chat.refresh_status_surfaces();
        assert_eq!(text(&chat), Some("Context unknown".into()));
        chat.set_token_info(Some(TokenUsageInfo {
            total_token_usage: TokenUsage::default(),
            last_token_usage: TokenUsage::default(),
            model_context_window: None,
        }));
        chat.refresh_status_surfaces();
        assert_eq!(
            text(&chat),
            Some("Context 0/1M [██████████] 100% left".into())
        );
        chat.config.model_context_window = Some(0);
        chat.refresh_status_surfaces();
        assert_eq!(text(&chat), Some("Context unavailable".into()));
        chat.config.model_context_window = Some(256_000);
        chat.token_info
            .as_mut()
            .unwrap()
            .last_token_usage
            .total_tokens = -1;
        chat.refresh_status_surfaces();
        assert_eq!(text(&chat), Some("Context unavailable".into()));
    }

    #[tokio::test]
    async fn infobar_account_change_and_explicit_clear_invalidate_quota_and_reset_data() {
        let (mut chat, _, _, _) = make_chatwidget_manual_with_sender().await;
        chat.setup_infobar(vec![
            InfobarItem::BankedResets,
            InfobarItem::Status(StatusLineItem::FiveHourLimit),
        ]);
        chat.set_infobar_reset_count(Some(3));
        chat.refresh_status_surfaces();
        assert_eq!(text(&chat), Some("Resets 3".into()));
        chat.on_rate_limit_snapshot(None);
        assert!(text(&chat).is_none());
        chat.set_infobar_reset_count(Some(0));
        chat.refresh_status_surfaces();
        assert_eq!(text(&chat), Some("Resets 0".into()));
        chat.update_account_state(None, None, false, false);
        assert!(text(&chat).is_none());
    }

    #[tokio::test]
    async fn infobar_workspace_headline_refresh_uses_shared_cache_and_interval() {
        let (mut chat, _, mut events, _) = make_chatwidget_manual_with_sender().await;
        chat.local_settings.tui.status_line = Some(vec![]);
        chat.has_codex_backend_auth = true;
        chat.setup_infobar(vec![InfobarItem::Status(StatusLineItem::WorkspaceHeadline)]);
        let request = |events: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>| {
            std::iter::from_fn(|| events.try_recv().ok()).find_map(|event| match event {
                AppEvent::RefreshStatusLineWorkspaceHeadline { request_id } => Some(request_id),
                _ => None,
            })
        };
        let first = request(&mut events).unwrap();
        assert!(chat.set_status_line_workspace_headline(
            first,
            Ok(
                crate::workspace_messages::WorkspaceHeadlineFetchResult::Available(Some(
                    "Workspace notice".into()
                ))
            )
        ));
        assert_eq!(text(&chat), Some("Workspace notice".into()));
        chat.refresh_status_line_if_workspace_headline_due();
        assert!(request(&mut events).is_none());
        chat.status_line_workspace_headline_last_requested_at =
            Some(Instant::now() - crate::workspace_messages::WORKSPACE_HEADLINE_REFRESH_INTERVAL);
        chat.refresh_status_line_if_workspace_headline_due();
        let second = request(&mut events).unwrap();
        assert!(chat.set_status_line_workspace_headline(second, Err("transient error".into())));
        assert_eq!(text(&chat), Some("Workspace notice".into()));
        assert!(!chat.set_status_line_workspace_headline(
            first,
            Ok(crate::workspace_messages::WorkspaceHeadlineFetchResult::FeatureDisabled)
        ));
        chat.update_account_state(None, None, false, false);
        assert!(text(&chat).is_none());
    }

    #[tokio::test]
    async fn infobar_sanitizes_multiline_fields_and_ignores_unknown_ids() {
        let (mut chat, _, _, _) = make_chatwidget_manual_with_sender().await;
        chat.local_settings.tui.infobar = Some(vec!["unknown".into(), "workspace-headline".into()]);
        chat.status_line_workspace_headline = Some("First\nSecond\tThird".into());
        // Keep this unit test focused on rendering; shared-state refresh would
        // schedule the normal workspace lookup first.
        let (items, invalids) = chat.infobar_items_with_invalids();
        chat.refresh_infobar(&items, &invalids);
        assert_eq!(text(&chat), Some("First Second Third".into()));
        assert_eq!(invalids, vec!["\"unknown\"".to_string()]);
    }
}
