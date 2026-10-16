use super::*;
use ratatui::{
    text::Line,
    widgets::{Paragraph, Widget},
};

fn workspace_selection_background(palette: &Palette) -> ratatui::style::Color {
    if palette.selection_bg == ratatui::style::Color::Reset {
        palette.active_row_bg
    } else {
        palette.selection_bg
    }
}

pub(in crate::client::shell) fn workspace_active_background(
    palette: &Palette,
    navigating: bool,
) -> ratatui::style::Color {
    // The fallback cursor shares the active-row color; only fill the cursor while navigating.
    if navigating && palette.selection_bg == ratatui::style::Color::Reset {
        palette.sidebar_bg
    } else {
        palette.active_row_bg
    }
}

pub(in crate::client::shell) fn collapsed_sidebar_sections(
    area: Rect,
) -> (Rect, Option<u16>, Rect) {
    let content = Rect::new(area.x, area.y, area.width.saturating_sub(1), area.height);
    if content.is_empty() {
        return (Rect::default(), None, Rect::default());
    }
    if content.height < 7 {
        return (content, None, Rect::default());
    }
    let workspace_height = content.height.div_ceil(2);
    let divider_y = content.y + workspace_height;
    let detail_height = content.height.saturating_sub(workspace_height + 1);
    (
        Rect::new(content.x, content.y, content.width, workspace_height),
        Some(divider_y),
        Rect::new(content.x, divider_y + 1, content.width, detail_height),
    )
}

pub(crate) fn render_collapsed_sidebar(
    buffer: &mut Buffer,
    area: Rect,
    snapshot: &ClientShellSnapshot,
    config: &ClientShellConfig,
    selected_workspace_id: Option<&str>,
    hits: &mut ShellHitMap,
) {
    let palette = &config.palette;
    let selection_background = workspace_selection_background(palette);
    let active_background = workspace_active_background(palette, selected_workspace_id.is_some());
    render_sidebar_background(buffer, area, palette);
    let (workspace_area, divider_y, detail_area) = collapsed_sidebar_sections(area);
    for (index, workspace) in snapshot
        .workspaces
        .iter()
        .take(workspace_area.height as usize)
        .enumerate()
    {
        let rect = Rect::new(
            workspace_area.x,
            workspace_area.y + index as u16,
            workspace_area.width,
            1,
        );
        let selected = selected_workspace_id == Some(workspace.workspace_id.as_str());
        if selected {
            buffer.set_style(rect, Style::default().bg(selection_background));
        } else if workspace.focused {
            buffer.set_style(rect, Style::default().bg(active_background));
        }
        let number_style = if selected {
            Style::default()
                .fg(palette.overlay1)
                .bg(selection_background)
        } else if workspace.focused {
            Style::default().fg(palette.text).bg(active_background)
        } else {
            Style::default().fg(palette.overlay0)
        };
        put_text(
            buffer,
            rect.x,
            rect.y,
            rect.width.min(2),
            &format!("{:<2}", index + 1),
            number_style,
        );
        let status = workspace.agent_status;
        put_text(
            buffer,
            rect.x.saturating_add(2),
            rect.y,
            rect.width.saturating_sub(2),
            status_icon(status, config.status_indicators),
            Style::default().fg(status_color(status, palette)),
        );
        hits.workspaces.push(WorkspaceHit {
            rect,
            endpoint_id: ClientEndpointId::Local,
            workspace_id: workspace.workspace_id.clone(),
            indented: false,
            group_toggle: None,
        });
    }

    if let Some(divider_y) = divider_y {
        put_text(
            buffer,
            workspace_area.x,
            divider_y,
            workspace_area.width,
            &"─".repeat(workspace_area.width as usize),
            Style::default().fg(palette.surface_dim),
        );
    }

    let detail_content = Rect::new(
        detail_area.x,
        detail_area.y,
        detail_area.width,
        detail_area.height.saturating_sub(1),
    );
    for (index, pane_id) in super::ordered_agent_pane_ids(snapshot, config.agent_panel_sort)
        .into_iter()
        .take(detail_content.height as usize)
        .enumerate()
    {
        let Some(agent) = snapshot
            .agents
            .iter()
            .find(|agent| agent.pane_id == pane_id)
        else {
            continue;
        };
        let rect = Rect::new(
            detail_content.x,
            detail_content.y + index as u16,
            detail_content.width,
            1,
        );
        if agent.focused {
            buffer.set_style(rect, Style::default().bg(palette.active_row_bg));
        }
        put_text(
            buffer,
            rect.x,
            rect.y,
            rect.width.min(2),
            &format!("{:<2}", index + 1),
            Style::default().fg(if agent.focused {
                palette.text
            } else {
                palette.overlay0
            }),
        );
        put_text(
            buffer,
            rect.x.saturating_add(2),
            rect.y,
            rect.width.saturating_sub(2),
            status_icon(agent.agent_status, config.status_indicators),
            Style::default().fg(status_color(agent.agent_status, palette)),
        );
        hits.agents.push((rect, pane_id));
    }
    hits.sidebar_toggle = if area.is_empty() || workspace_area.width == 0 {
        Rect::default()
    } else {
        Rect::new(
            workspace_area.x + workspace_area.width / 2,
            area.bottom().saturating_sub(1),
            1,
            1,
        )
    };
    put_text(
        buffer,
        hits.sidebar_toggle.x,
        hits.sidebar_toggle.y,
        hits.sidebar_toggle.width,
        "»",
        if super::super::global_menu::global_menu_attention(snapshot) {
            Style::default()
                .fg(palette.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette.overlay0)
        },
    );
}

pub(crate) fn render_sidebar(
    buffer: &mut Buffer,
    area: Rect,
    snapshot: &ClientShellSnapshot,
    config: &ClientShellConfig,
    state: &mut ShellRenderState<'_>,
    hits: &mut ShellHitMap,
) {
    let palette = &config.palette;
    render_sidebar_background(buffer, area, palette);
    hits.sidebar_divider = if area.is_empty() {
        Rect::default()
    } else {
        Rect::new(area.right().saturating_sub(1), area.y, 1, area.height)
    };
    let sections = SidebarSectionLayout::compute(area, config, state);
    let workspace_area = sections.spaces;
    let accounts_area = sections.accounts;
    let detail_area = sections.agents;
    hits.sidebar_split_region = sections.split_region;
    hits.sidebar_split_gap = sections.accounts.height;
    hits.sidebar_section_divider = sections.section_divider;
    put_text(
        buffer,
        workspace_area.x,
        workspace_area.y,
        workspace_area.width,
        " spaces",
        Style::default()
            .fg(palette.overlay0)
            .add_modifier(Modifier::BOLD),
    );
    render_section_fold_marker(
        buffer,
        workspace_area,
        workspace_area.y,
        display_width(" spaces"),
        sections.spaces_collapsed,
        palette,
    );
    if config.mouse_capture && workspace_area.height > 0 {
        hits.section_headers.push((
            Rect::new(workspace_area.x, workspace_area.y, workspace_area.width, 1),
            SidebarSection::Spaces,
        ));
    }

    let entries = if sections.spaces_collapsed {
        Vec::new()
    } else {
        workspace_entries(snapshot, state.collapsed_groups)
    };
    let body = Rect::new(
        workspace_area.x,
        workspace_area.y.saturating_add(WORKSPACE_HEADER_ROWS),
        workspace_area.width,
        workspace_area
            .height
            .saturating_sub(WORKSPACE_HEADER_ROWS + 1),
    );
    hits.workspace_body = body;
    let row_heights = entries
        .iter()
        .map(|entry| {
            snapshot
                .workspaces
                .get(entry.index)
                .map(|workspace| {
                    workspace_rows(
                        workspace,
                        displayed_workspace_status(snapshot, workspace, state.collapsed_groups),
                        entry.indented,
                        workspace_tab_count(snapshot, &workspace.workspace_id),
                        &config.spaces,
                    )
                    .len()
                    .max(1)
                    .min(u16::MAX as usize) as u16
                })
                .unwrap_or(1)
        })
        .collect::<Vec<_>>();
    let gaps = entries
        .iter()
        .enumerate()
        .map(|(index, _)| {
            entries
                .get(index + 1)
                .map_or(0, |next| u16::from(!next.indented) * config.spaces.row_gap)
        })
        .collect::<Vec<_>>();
    let mut metrics = super::scroll::list_scroll_metrics(
        &row_heights,
        &gaps,
        body.height,
        *state.workspace_scroll,
    );
    if !body.is_empty() && std::mem::take(state.reveal_focused_workspace) {
        if let Some(target) = entries
            .iter()
            .position(|entry| snapshot.workspaces[entry.index].focused)
        {
            *state.workspace_scroll = super::scroll::list_scroll_start_to_reveal(
                &row_heights,
                &gaps,
                body.height,
                *state.workspace_scroll,
                target,
            );
            metrics = super::scroll::list_scroll_metrics(
                &row_heights,
                &gaps,
                body.height,
                *state.workspace_scroll,
            );
        }
    }
    hits.workspace_max_scroll = metrics.max_offset_from_bottom;
    hits.workspace_scroll_metrics = Some(metrics);
    *state.workspace_scroll = metrics
        .max_offset_from_bottom
        .saturating_sub(metrics.offset_from_bottom);
    let show_scrollbar = metrics.max_offset_from_bottom > 0 && body.width > 1;
    let content_width = body.width.saturating_sub(u16::from(show_scrollbar));
    let mut y = body.y;
    for (entry_position, entry) in entries.iter().enumerate().skip(*state.workspace_scroll) {
        let Some(workspace) = snapshot.workspaces.get(entry.index) else {
            continue;
        };
        let status = displayed_workspace_status(snapshot, workspace, state.collapsed_groups);
        let rows = workspace_rows(
            workspace,
            status,
            entry.indented,
            workspace_tab_count(snapshot, &workspace.workspace_id),
            &config.spaces,
        );
        let row_height = (rows.len().max(1).min(u16::MAX as usize) as u16).min(body.height);
        if y.saturating_add(row_height) > body.bottom() {
            break;
        }
        let rect = Rect::new(body.x, y, content_width, row_height);
        let selected = state.selected_workspace_id.is_some_and(|target| {
            target.matches(state.active_endpoint_id, &workspace.workspace_id)
        });
        let dragged = state.dragged_workspace_id == Some(workspace.workspace_id.as_str());
        if selected {
            buffer.set_style(rect, Style::default().bg(palette.selection_bg));
        } else if dragged {
            buffer.set_style(rect, Style::default().bg(palette.surface1));
        } else if workspace.focused {
            buffer.set_style(rect, Style::default().bg(palette.active_row_bg));
        }
        render_workspace_rows(
            buffer,
            rect,
            status,
            config.status_indicators,
            entry,
            rows,
            workspace.focused,
            selected,
            state.selected_workspace_id.is_some(),
            dragged,
            palette,
        );
        let group_toggle = render_parent_group_toggle(
            buffer,
            rect,
            snapshot,
            entry.index,
            state.collapsed_groups,
            palette,
        );
        hits.workspaces.push(WorkspaceHit {
            rect,
            endpoint_id: ClientEndpointId::Local,
            workspace_id: workspace.workspace_id.clone(),
            indented: entry.indented,
            group_toggle,
        });
        let gap = entries
            .get(entry_position + 1)
            .map_or(0, |next| u16::from(!next.indented) * config.spaces.row_gap);
        y = y.saturating_add(row_height + gap);
    }

    if show_scrollbar {
        let track = Rect::new(body.right().saturating_sub(1), body.y, 1, body.height);
        hits.workspace_scrollbar = track;
        super::scroll::render_list_scrollbar(buffer, track, metrics, palette);
    }

    if let Some(row) = state.workspace_drop_indicator_row.filter(|row| {
        *row >= workspace_area.y.saturating_add(1)
            && *row < workspace_area.bottom().saturating_sub(1)
    }) {
        put_text(
            buffer,
            body.x,
            row,
            body.width,
            &"─".repeat(body.width as usize),
            Style::default().fg(palette.accent),
        );
    }

    let footer_y = workspace_area.bottom().saturating_sub(1);
    if config.mouse_capture {
        hits.new_workspace = Rect::new(
            workspace_area.x,
            footer_y,
            5.min(workspace_area.width),
            u16::from(workspace_area.height > 0),
        );
        put_text(
            buffer,
            workspace_area.x,
            footer_y,
            workspace_area.width,
            " new",
            Style::default().fg(palette.overlay0),
        );
        let attention = super::super::global_menu::global_menu_attention(snapshot);
        let launcher_width = if attention { 8 } else { 6 }.min(workspace_area.width);
        hits.global_launcher = Rect::new(
            workspace_area.right().saturating_sub(launcher_width),
            footer_y,
            launcher_width,
            1,
        );
        if attention {
            let start_x = workspace_area.right().saturating_sub(6);
            put_text(
                buffer,
                start_x,
                footer_y,
                2,
                "● ",
                Style::default()
                    .fg(palette.accent)
                    .add_modifier(Modifier::BOLD),
            );
            put_text(
                buffer,
                start_x.saturating_add(2),
                footer_y,
                4,
                "menu",
                Style::default().fg(palette.overlay0),
            );
        } else {
            put_right_text(
                buffer,
                workspace_area,
                footer_y,
                "menu",
                Style::default().fg(palette.overlay0),
            );
        }
    }

    if accounts_area.height > 0 {
        render_accounts_section(
            buffer,
            accounts_area,
            config,
            state.accounts,
            state.account_scroll,
            sections.accounts_collapsed,
            hits,
        );
    }

    let agent_panel_area = if sections.agents_collapsed {
        Rect::new(
            detail_area.x,
            detail_area.y,
            detail_area.width,
            detail_area.height.min(FOLDED_SECTION_ROWS),
        )
    } else {
        detail_area
    };
    super::render_agent_panel(
        buffer,
        agent_panel_area,
        snapshot,
        config,
        state.agent_scroll,
        hits,
    );
    if detail_area.height >= 2 {
        let header_y = detail_area.y.saturating_add(1);
        render_section_fold_marker(
            buffer,
            detail_area,
            header_y,
            display_width(" agents"),
            sections.agents_collapsed,
            palette,
        );
        if config.mouse_capture {
            let reserved = hits.agent_sort_toggle.width.saturating_add(1);
            hits.section_headers.push((
                Rect::new(
                    detail_area.x,
                    header_y,
                    detail_area.width.saturating_sub(reserved),
                    1,
                ),
                SidebarSection::Agents,
            ));
        }
    }

    hits.sidebar_toggle = Rect::new(
        area.right().saturating_sub(2),
        area.bottom().saturating_sub(1),
        u16::from(area.width > 1),
        u16::from(area.height > 0),
    );
    put_text(
        buffer,
        hits.sidebar_toggle.x,
        hits.sidebar_toggle.y,
        hits.sidebar_toggle.width,
        "«",
        Style::default().fg(palette.overlay0),
    );
}

pub(crate) fn workspace_entries(
    snapshot: &ClientShellSnapshot,
    collapsed_groups: &HashSet<String>,
) -> Vec<WorkspaceEntry> {
    let mut members = HashMap::<&str, Vec<usize>>::new();
    for (index, workspace) in snapshot.workspaces.iter().enumerate() {
        if let Some(worktree) = &workspace.worktree {
            members.entry(&worktree.key).or_default().push(index);
        }
    }
    let grouped = members
        .iter()
        .filter(|(_, indices)| {
            indices.iter().any(|index| {
                snapshot.workspaces[*index]
                    .worktree
                    .as_ref()
                    .is_some_and(|worktree| worktree.is_linked_worktree)
            }) && indices.iter().any(|index| {
                snapshot.workspaces[*index]
                    .worktree
                    .as_ref()
                    .is_some_and(|worktree| !worktree.is_linked_worktree)
            })
        })
        .map(|(key, _)| *key)
        .collect::<HashSet<_>>();
    let mut emitted = HashSet::<&str>::new();
    let mut entries = Vec::new();
    for (index, workspace) in snapshot.workspaces.iter().enumerate() {
        let Some(worktree) = workspace
            .worktree
            .as_ref()
            .filter(|worktree| grouped.contains(worktree.key.as_str()))
        else {
            entries.push(WorkspaceEntry {
                index,
                indented: false,
                last_child: false,
            });
            continue;
        };
        if !emitted.insert(&worktree.key) {
            continue;
        }
        let Some(group_members) = members.get(worktree.key.as_str()) else {
            continue;
        };
        for parent in group_members.iter().copied().filter(|member| {
            snapshot.workspaces[*member]
                .worktree
                .as_ref()
                .is_some_and(|worktree| !worktree.is_linked_worktree)
        }) {
            entries.push(WorkspaceEntry {
                index: parent,
                indented: false,
                last_child: false,
            });
        }
        if collapsed_groups.contains(&worktree.key) {
            if let Some(active) = group_members.iter().copied().find(|member| {
                let workspace = &snapshot.workspaces[*member];
                workspace.focused
                    && workspace
                        .worktree
                        .as_ref()
                        .is_some_and(|worktree| worktree.is_linked_worktree)
            }) {
                entries.push(WorkspaceEntry {
                    index: active,
                    indented: true,
                    last_child: true,
                });
            }
            continue;
        }
        let children = group_members
            .iter()
            .copied()
            .filter(|member| {
                snapshot.workspaces[*member]
                    .worktree
                    .as_ref()
                    .is_some_and(|worktree| worktree.is_linked_worktree)
            })
            .collect::<Vec<_>>();
        for (child_index, child) in children.iter().enumerate() {
            entries.push(WorkspaceEntry {
                index: *child,
                indented: true,
                last_child: child_index + 1 == children.len(),
            });
        }
    }
    entries
}

fn parent_group_key(snapshot: &ClientShellSnapshot, index: usize) -> Option<String> {
    let workspace = snapshot.workspaces.get(index)?;
    let worktree = workspace.worktree.as_ref()?;
    if worktree.is_linked_worktree {
        return None;
    }
    snapshot
        .workspaces
        .iter()
        .any(|candidate| {
            candidate.worktree.as_ref().is_some_and(|candidate| {
                candidate.key == worktree.key && candidate.is_linked_worktree
            })
        })
        .then(|| worktree.key.clone())
}

pub(in crate::client::shell) fn workspace_close_is_group(
    snapshot: &ClientShellSnapshot,
    workspace: &ClientShellWorkspace,
) -> bool {
    let Some(worktree) = workspace
        .worktree
        .as_ref()
        .filter(|worktree| !worktree.is_linked_worktree)
    else {
        return false;
    };
    let mut has_child = false;
    for member in &snapshot.workspaces {
        if member.workspace_id == workspace.workspace_id {
            continue;
        }
        if let Some(candidate) = member
            .worktree
            .as_ref()
            .filter(|candidate| candidate.key == worktree.key)
        {
            if !candidate.is_linked_worktree {
                return false;
            }
            has_child = true;
        }
    }
    has_child
}

pub(in crate::client::shell) fn render_parent_group_toggle(
    buffer: &mut Buffer,
    workspace_rect: Rect,
    snapshot: &ClientShellSnapshot,
    workspace_index: usize,
    collapsed_groups: &HashSet<String>,
    palette: &Palette,
) -> Option<(Rect, String)> {
    let key = parent_group_key(snapshot, workspace_index)?;
    let toggle = Rect::new(
        workspace_rect.right().saturating_sub(1),
        workspace_rect.y,
        1,
        1,
    );
    put_text(
        buffer,
        toggle.x,
        toggle.y,
        toggle.width,
        if collapsed_groups.contains(&key) {
            "▸"
        } else {
            "▾"
        },
        Style::default().fg(palette.accent),
    );
    Some((toggle, key))
}

pub(in crate::client::shell) fn displayed_workspace_status(
    snapshot: &ClientShellSnapshot,
    workspace: &ClientShellWorkspace,
    collapsed_groups: &HashSet<String>,
) -> crate::api::schema::AgentStatus {
    let Some(worktree) = workspace
        .worktree
        .as_ref()
        .filter(|worktree| !worktree.is_linked_worktree)
    else {
        return workspace.agent_status;
    };
    if !collapsed_groups.contains(&worktree.key) {
        return workspace.agent_status;
    }
    snapshot
        .workspaces
        .iter()
        .filter(|candidate| {
            candidate.worktree.as_ref().is_some_and(|member| {
                member.key == worktree.key
                    && (member.is_linked_worktree
                        || candidate.workspace_id == workspace.workspace_id)
            })
        })
        .map(|candidate| candidate.agent_status)
        .max_by_key(|status| status_priority(*status))
        .unwrap_or(workspace.agent_status)
}

pub(in crate::client::shell) fn workspace_tab_count(
    snapshot: &ClientShellSnapshot,
    workspace_id: &str,
) -> usize {
    snapshot
        .tabs
        .iter()
        .filter(|tab| tab.workspace_id == workspace_id)
        .count()
}

pub(in crate::client::shell) fn workspace_rows(
    workspace: &ClientShellWorkspace,
    status: crate::api::schema::AgentStatus,
    indented: bool,
    tab_count: usize,
    config: &SpacesSidebarConfig,
) -> Vec<Vec<crate::ui::ResolvedToken>> {
    let label = if indented && !workspace.custom_label {
        workspace
            .branch
            .as_deref()
            .and_then(|branch| branch.strip_prefix("worktree/").or(Some(branch)))
            .unwrap_or(&workspace.label)
    } else {
        &workspace.label
    };
    let token_values = workspace.tokens.iter().cloned().collect::<HashMap<_, _>>();
    crate::ui::sidebar_space_rows(
        config,
        crate::ui::SpaceTokenContext {
            workspace: label,
            branch: workspace.branch.as_deref(),
            state_text: status_text(status),
            ahead_behind: workspace.git_ahead_behind,
            tab_count,
            tokens: &token_values,
            suppress_git_details: indented,
        },
    )
}

pub(in crate::client::shell) fn render_workspace_rows(
    buffer: &mut Buffer,
    area: Rect,
    status: crate::api::schema::AgentStatus,
    indicators: crate::config::StatusIndicatorStyle,
    entry: &WorkspaceEntry,
    rows: Vec<Vec<crate::ui::ResolvedToken>>,
    focused: bool,
    selected: bool,
    navigating: bool,
    dragged: bool,
    palette: &Palette,
) {
    for (row_index, row) in rows.iter().enumerate() {
        let y = area.y + row_index as u16;
        if y >= area.bottom() {
            break;
        }
        let mut x = area.x;
        if entry.indented {
            let prefix = if row_index == 0 {
                if entry.last_child {
                    "   └─ "
                } else {
                    "   ├─ "
                }
            } else if entry.last_child {
                "        "
            } else {
                "   │    "
            };
            x = put_segment(
                buffer,
                x,
                y,
                area.right(),
                prefix,
                Style::default().fg(palette.overlay0),
            );
        } else if row_index == 0 {
            x = x.saturating_add(1);
        } else {
            x = x.saturating_add(3);
        }
        let highlighted = focused || dragged;
        let workspace_style = Style::default()
            .fg(if highlighted {
                palette.text
            } else {
                palette.subtext0
            })
            .add_modifier(if highlighted {
                Modifier::BOLD
            } else {
                Modifier::empty()
            });
        let secondary_style = Style::default().fg(if focused {
            palette.mauve
        } else {
            palette.overlay0
        });
        let spans = crate::ui::resolved_token_spans(
            row,
            (
                status_icon(status, indicators),
                Style::default().fg(status_color(status, palette)),
            ),
            Style::default().fg(status_color(status, palette)),
            workspace_style,
            secondary_style,
            Style::default().fg(palette.overlay1),
            palette,
            area.right().saturating_sub(2).saturating_sub(x) as usize,
        );
        Paragraph::new(Line::from(spans)).render(
            Rect::new(x, y, area.right().saturating_sub(2).saturating_sub(x), 1),
            buffer,
        );
    }

    let background = if selected {
        Some(workspace_selection_background(palette))
    } else if dragged {
        Some(palette.surface1)
    } else if focused {
        Some(workspace_active_background(palette, navigating))
    } else {
        None
    };
    if let Some(background) = background {
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                buffer[(x, y)].set_bg(background);
            }
        }
    }
}

/// Rows the accounts section spends on its divider and title.
const ACCOUNTS_HEADER_ROWS: u16 = 2;
/// Rows a folded spaces or agents section keeps (title, plus footer or divider).
const FOLDED_SECTION_ROWS: u16 = 2;
/// Below this many sidebar rows the accounts section stays hidden.
const ACCOUNTS_MIN_SIDEBAR_ROWS: u16 = 12;

/// Vertical split of the expanded desktop sidebar into spaces / accounts / agents.
pub(in crate::client::shell) struct SidebarSectionLayout {
    pub(in crate::client::shell) spaces: Rect,
    pub(in crate::client::shell) accounts: Rect,
    pub(in crate::client::shell) agents: Rect,
    /// Rows shared by the split ratio: the whole content column minus the accounts section.
    pub(in crate::client::shell) split_region: Rect,
    pub(in crate::client::shell) section_divider: Rect,
    pub(in crate::client::shell) spaces_collapsed: bool,
    pub(in crate::client::shell) accounts_collapsed: bool,
    pub(in crate::client::shell) agents_collapsed: bool,
}

impl SidebarSectionLayout {
    pub(in crate::client::shell) fn compute(
        area: Rect,
        config: &ClientShellConfig,
        state: &ShellRenderState<'_>,
    ) -> Self {
        let content = Rect::new(area.x, area.y, area.width.saturating_sub(1), area.height);
        let spaces_collapsed = state.collapsed_sections.contains(&SidebarSection::Spaces);
        let accounts_collapsed = state.collapsed_sections.contains(&SidebarSection::Accounts);
        let agents_collapsed = state.collapsed_sections.contains(&SidebarSection::Agents);
        if content.is_empty() {
            return Self {
                spaces: Rect::default(),
                accounts: Rect::default(),
                agents: Rect::default(),
                split_region: Rect::default(),
                section_divider: Rect::default(),
                spaces_collapsed,
                accounts_collapsed,
                agents_collapsed,
            };
        }
        let accounts_enabled = config.accounts.enabled
            && !config.accounts.list_command.trim().is_empty()
            && content.height >= ACCOUNTS_MIN_SIDEBAR_ROWS;
        let accounts_height = if !accounts_enabled {
            0
        } else if accounts_collapsed {
            ACCOUNTS_HEADER_ROWS
        } else {
            let body = account_section_body_lines(state.accounts);
            let max_body = (content.height / 3).max(2);
            ACCOUNTS_HEADER_ROWS.saturating_add(body.min(max_body))
        };
        let remaining = content.height.saturating_sub(accounts_height);
        let folded = FOLDED_SECTION_ROWS.min(remaining);
        let (spaces_height, agents_height) = match (spaces_collapsed, agents_collapsed) {
            (true, _) => (folded, remaining.saturating_sub(folded)),
            (false, true) => (remaining.saturating_sub(folded), folded),
            (false, false) => {
                crate::ui::sidebar_section_heights(remaining, state.sidebar_section_split)
            }
        };
        let spaces = Rect::new(content.x, content.y, content.width, spaces_height);
        let accounts = Rect::new(content.x, spaces.bottom(), content.width, accounts_height);
        let agents = Rect::new(content.x, accounts.bottom(), content.width, agents_height);
        let section_divider = if !spaces_collapsed && !agents_collapsed && remaining >= 6 {
            Rect::new(content.x, agents.y, content.width, 1)
        } else {
            Rect::default()
        };
        Self {
            spaces,
            accounts,
            agents,
            split_region: Rect::new(content.x, content.y, content.width, remaining),
            section_divider,
            spaces_collapsed,
            accounts_collapsed,
            agents_collapsed,
        }
    }
}

/// Body rows the accounts list wants: two per account, or one for a message.
fn account_section_body_lines(accounts: &super::super::accounts::ClientAccountsState) -> u16 {
    if accounts.accounts.is_empty() {
        1
    } else {
        accounts
            .accounts
            .len()
            .saturating_mul(2)
            .min(u16::MAX as usize) as u16
    }
}

fn render_section_fold_marker(
    buffer: &mut Buffer,
    area: Rect,
    y: u16,
    title_width: u16,
    collapsed: bool,
    palette: &Palette,
) {
    let x = area.x.saturating_add(title_width).saturating_add(1);
    if x >= area.right() {
        return;
    }
    put_text(
        buffer,
        x,
        y,
        1,
        if collapsed { "▸" } else { "▾" },
        Style::default()
            .fg(palette.overlay0)
            .add_modifier(Modifier::DIM),
    );
}

fn render_accounts_section(
    buffer: &mut Buffer,
    area: Rect,
    config: &ClientShellConfig,
    accounts: &super::super::accounts::ClientAccountsState,
    account_scroll: &mut usize,
    collapsed: bool,
    hits: &mut ShellHitMap,
) {
    let palette = &config.palette;
    put_text(
        buffer,
        area.x,
        area.y,
        area.width,
        &"─".repeat(area.width as usize),
        Style::default().fg(palette.surface_dim),
    );
    if area.height < 2 {
        return;
    }
    let header_y = area.y.saturating_add(1);
    let title = format!(" {}", config.accounts.title.trim());
    put_text(
        buffer,
        area.x,
        header_y,
        area.width,
        &title,
        Style::default()
            .fg(palette.overlay0)
            .add_modifier(Modifier::BOLD),
    );
    render_section_fold_marker(
        buffer,
        area,
        header_y,
        display_width(&title),
        collapsed,
        palette,
    );
    let status = if accounts.busy {
        Some(("…", palette.accent))
    } else if accounts.error.is_some() {
        Some(("!", palette.red))
    } else {
        None
    };
    let mut reserved = 0;
    if let Some((glyph, color)) = status {
        put_right_text(
            buffer,
            area,
            header_y,
            glyph,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        );
        reserved = display_width(glyph).saturating_add(1);
    }
    if config.mouse_capture {
        hits.section_headers.push((
            Rect::new(area.x, header_y, area.width.saturating_sub(reserved), 1),
            SidebarSection::Accounts,
        ));
    }

    let body = Rect::new(
        area.x,
        area.y.saturating_add(ACCOUNTS_HEADER_ROWS),
        area.width,
        area.height.saturating_sub(ACCOUNTS_HEADER_ROWS),
    );
    hits.account_body = body;
    if collapsed || body.is_empty() {
        *account_scroll = 0;
        return;
    }
    if accounts.accounts.is_empty() {
        *account_scroll = 0;
        let (message, color) = match accounts.error.as_deref() {
            Some(error) => (format!(" ✖ {error}"), palette.red),
            None if accounts.fetched_at.is_none() => (" connecting…".to_string(), palette.overlay0),
            None => (" no accounts".to_string(), palette.overlay0),
        };
        put_text(
            buffer,
            body.x,
            body.y,
            body.width,
            &message,
            Style::default().fg(color).add_modifier(Modifier::DIM),
        );
        return;
    }

    let row_heights = vec![2u16; accounts.accounts.len()];
    let gaps = vec![0u16; accounts.accounts.len()];
    let metrics =
        super::scroll::list_scroll_metrics(&row_heights, &gaps, body.height, *account_scroll);
    hits.account_max_scroll = metrics.max_offset_from_bottom;
    *account_scroll = metrics
        .max_offset_from_bottom
        .saturating_sub(metrics.offset_from_bottom);
    let show_scrollbar = metrics.max_offset_from_bottom > 0 && body.width > 1;
    let content_width = body.width.saturating_sub(u16::from(show_scrollbar));
    let mut y = body.y;
    for (index, account) in accounts.accounts.iter().enumerate().skip(*account_scroll) {
        let height = 2u16.min(body.height);
        if y.saturating_add(height) > body.bottom() {
            break;
        }
        let rect = Rect::new(body.x, y, content_width, height);
        render_account_row(buffer, rect, account, palette);
        hits.accounts.push((rect, index));
        y = y.saturating_add(height);
    }
    if show_scrollbar {
        let track = Rect::new(body.right().saturating_sub(1), body.y, 1, body.height);
        super::scroll::render_list_scrollbar(buffer, track, metrics, palette);
    }
}

fn usage_color(pct: Option<u8>, palette: &Palette) -> ratatui::style::Color {
    match pct {
        Some(pct) if pct >= 85 => palette.red,
        Some(pct) if pct >= 60 => palette.peach,
        Some(_) => palette.teal,
        None => palette.overlay0,
    }
}

fn render_account_row(
    buffer: &mut Buffer,
    rect: Rect,
    account: &super::super::accounts::ClientAccount,
    palette: &Palette,
) {
    if account.active {
        buffer.set_style(rect, Style::default().bg(palette.active_row_bg));
    }
    let right = rect.right();
    let (icon, icon_color) = if account.disabled {
        ("×", palette.red)
    } else if account.active {
        ("◆", palette.accent)
    } else {
        ("◇", palette.overlay0)
    };
    let mut x = put_segment(
        buffer,
        rect.x.saturating_add(1),
        rect.y,
        right,
        icon,
        Style::default().fg(icon_color).add_modifier(Modifier::BOLD),
    );
    let label_style = if account.active {
        Style::default()
            .fg(palette.text)
            .add_modifier(Modifier::BOLD)
    } else if account.disabled {
        Style::default()
            .fg(palette.overlay0)
            .add_modifier(Modifier::DIM)
    } else {
        Style::default()
            .fg(palette.subtext0)
            .add_modifier(Modifier::BOLD)
    };
    x = put_segment(
        buffer,
        x.saturating_add(1),
        rect.y,
        right,
        account.label(),
        label_style,
    );
    if account.disabled {
        put_segment(
            buffer,
            x.saturating_add(1),
            rect.y,
            right,
            "offline",
            Style::default().fg(palette.red).add_modifier(Modifier::DIM),
        );
    }
    if rect.height < 2 {
        return;
    }
    let y = rect.y.saturating_add(1);
    let dim = Style::default().fg(palette.overlay0);
    let value = Style::default().fg(palette.subtext0);
    let pct_text = |pct: Option<u8>| match pct {
        Some(pct) => format!("{pct:>3}%"),
        None => "  --".to_string(),
    };
    let mut x = put_segment(buffer, rect.x.saturating_add(3), y, right, "5h ", dim);
    x = put_segment(
        buffer,
        x,
        y,
        right,
        &super::super::accounts::usage_bar(account.five_hour_pct),
        Style::default().fg(usage_color(account.five_hour_pct, palette)),
    );
    x = put_segment(buffer, x, y, right, &pct_text(account.five_hour_pct), value);
    x = put_segment(buffer, x, y, right, "  7d ", dim);
    put_segment(
        buffer,
        x,
        y,
        right,
        pct_text(account.seven_day_pct).trim_start(),
        Style::default().fg(usage_color(account.seven_day_pct, palette)),
    );
}
