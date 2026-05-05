use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

use chrono::{DateTime, Datelike, Local, Utc};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use uuid::Uuid;

use crate::snapshots::{self, SnapPopupState, SnapViewerData, load_snapshot};
use crate::types::Task;
use crate::ui::board::project_to_color;
use crate::ui::tui::{RowKind, build_rows_from};

// ─── Shared viewer row renderer ───────────────────────────────────────────────

enum ViewerEntry {
    Separator(String, Color),
    Row(usize),
}

fn build_viewer_entries(rows: &[crate::ui::tui::TuiRow], tasks: &[Task]) -> Vec<ViewerEntry> {
    let tasks_by_id: HashMap<Uuid, &Task> = tasks.iter().map(|t| (t.id, t)).collect();
    let mut entries: Vec<ViewerEntry> = Vec::new();
    let mut cur_proj: Option<String> = None;
    for (i, row) in rows.iter().enumerate() {
        if row.depth == 0 {
            let proj = tasks_by_id.get(&row.id).map(|t| t.project.as_str()).unwrap_or("");
            if cur_proj.as_deref() != Some(proj) {
                let color = project_to_color(proj);
                let label = if proj.is_empty() { "INBOX".to_string() } else { proj.to_string() };
                entries.push(ViewerEntry::Separator(label, color));
                cur_proj = Some(proj.to_string());
            }
        }
        entries.push(ViewerEntry::Row(i));
    }
    entries
}

/// Renders a read-only snapshot tree. Mutably borrows viewer to sync scroll_offset
/// back to its true clamped value each frame, which prevents phantom-offset accumulation.
fn draw_viewer_rows(frame: &mut Frame, viewer: &mut SnapViewerData, area: Rect) {
    if area.height == 0 {
        return;
    }
    let rows = build_rows_from(&viewer.tasks, &viewer.projects, &viewer.collapsed);
    let entries = build_viewer_entries(&rows, &viewer.tasks);

    // Per-depth sibling counters, same logic as the live tree view.
    let mut depth_counters: Vec<usize> = Vec::new();
    let num_labels: Vec<String> = rows.iter().map(|row| {
        let d = row.depth;
        if d < depth_counters.len() {
            depth_counters.truncate(d + 1);
            depth_counters[d] += 1;
        } else {
            depth_counters.push(1);
        }
        depth_counters.last().map(|n| n.to_string()).unwrap_or_default()
    }).collect();

    let area_h = area.height as usize;
    let max_scroll = entries.len().saturating_sub(area_h);
    viewer.scroll_offset = viewer.scroll_offset.min(max_scroll);
    let scroll = viewer.scroll_offset;

    let bg = Color::Indexed(234);
    let num_style = Style::default().fg(Color::Indexed(240)).bg(bg);
    let mut y = area.y;
    for entry in entries.iter().skip(scroll).take(area_h) {
        match entry {
            ViewerEntry::Separator(name, color) => {
                let pill = format!(" {} ", name);
                let fill_width =
                    (area.width as usize).saturating_sub(pill.chars().count() + 1);
                let fill = "─".repeat(fill_width);
                let line = Line::from(vec![
                    Span::styled(
                        pill,
                        Style::default()
                            .fg(Color::Indexed(234))
                            .bg(*color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" {}", fill),
                        Style::default().fg(Color::Indexed(238)),
                    ),
                ]);
                frame.render_widget(
                    Paragraph::new(line).style(Style::default().bg(bg)),
                    Rect { x: area.x, y, width: area.width, height: 1 },
                );
            }
            ViewerEntry::Row(idx) => {
                let row = &rows[*idx];
                let num_str = format!("{} ", num_labels[*idx]);
                let title_style = match row.kind {
                    RowKind::Todo => Style::default().fg(Color::Indexed(252)).bg(bg),
                    RowKind::Doing => Style::default()
                        .fg(Color::Indexed(214))
                        .bg(bg)
                        .add_modifier(Modifier::BOLD),
                    RowKind::Done => Style::default()
                        .fg(Color::Indexed(240))
                        .bg(bg)
                        .add_modifier(Modifier::CROSSED_OUT),
                };
                let meta_style = Style::default().fg(Color::Indexed(238)).bg(bg);
                let collapse_indicator = if row.is_collapsed { "▸ " } else { "" };
                let prefix_width = row.display_prefix.chars().count()
                    + num_str.chars().count()
                    + collapse_indicator.chars().count();
                let title_width = (area.width as usize).saturating_sub(prefix_width);
                let title_text: String = row.title.chars().take(title_width).collect();

                let mut spans: Vec<Span> = Vec::new();
                if !row.display_prefix.is_empty() {
                    spans.push(Span::styled(row.display_prefix.clone(), meta_style));
                }
                spans.push(Span::styled(num_str, num_style));
                if !collapse_indicator.is_empty() {
                    spans.push(Span::styled(
                        collapse_indicator,
                        Style::default().fg(Color::Indexed(214)).bg(bg),
                    ));
                }
                spans.push(Span::styled(title_text, title_style));

                frame.render_widget(
                    Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
                    Rect { x: area.x, y, width: area.width, height: 1 },
                );
            }
        }
        y += 1;
    }
}

// ─── In-TUI popup ─────────────────────────────────────────────────────────────

pub fn draw_snap_popup(frame: &mut Frame, popup: &mut SnapPopupState) {
    let area = centered_rect(60, 80, frame.area());
    frame.render_widget(Clear, area);

    if let Some(ref mut viewer) = popup.viewer {
        draw_popup_viewer(frame, viewer, area);
    } else {
        draw_popup_list(frame, popup, area);
    }
}

fn draw_popup_list(frame: &mut Frame, popup: &SnapPopupState, area: Rect) {
    let block = Block::default()
        .title(" SNAPSHOTS  — Enter to open · Esc to close ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(Color::Indexed(235)));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if popup.entries.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    "  No snapshots yet. Press ",
                    Style::default().fg(Color::Indexed(244)),
                ),
                Span::styled(
                    "S",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " in tree view to save one.",
                    Style::default().fg(Color::Indexed(244)),
                ),
            ]))
            .style(Style::default().bg(Color::Indexed(235))),
            inner,
        );
        return;
    }

    let mut lines: Vec<Line> = vec![Line::from("")];
    for (idx, entry) in popup.entries.iter().enumerate() {
        let is_sel = idx == popup.cursor;
        let bg = if is_sel { Color::Indexed(238) } else { Color::Indexed(235) };
        lines.push(Line::from(vec![
            Span::styled(
                if is_sel { "  ▶ " } else { "    " },
                Style::default().fg(Color::Indexed(214)).bg(bg),
            ),
            Span::styled(
                entry.label.clone(),
                if is_sel {
                    Style::default()
                        .fg(Color::Indexed(252))
                        .bg(bg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Indexed(244)).bg(bg)
                },
            ),
        ]));
    }

    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(Color::Indexed(235))),
        inner,
    );
}

fn draw_popup_viewer(frame: &mut Frame, viewer: &mut SnapViewerData, area: Rect) {
    let title = format!(" {}  — j/k scroll · q/Esc back ", viewer.label);
    let block = Block::default()
        .title(title.as_str())
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(Color::Indexed(234)));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    draw_viewer_rows(frame, viewer, inner);
}

// ─── Standalone zen snaps browser ────────────────────────────────────────────

#[derive(Hash, PartialEq, Eq, Clone, Debug)]
pub enum NodeKey {
    Year(i32),
    Month(i32, u32),
    Day(i32, u32, u32),
}

#[derive(Clone, Debug)]
pub enum BrowserNode {
    Year(i32),
    Month(i32, u32),
    Day(i32, u32, u32),
    Snap { taken_at: DateTime<Utc>, path: PathBuf },
}

pub struct BrowserItem {
    pub node: BrowserNode,
    pub depth: usize,
    pub expanded: bool,
}

pub struct SnapsApp {
    pub all_snaps: Vec<(DateTime<Utc>, PathBuf)>,
    pub expanded: HashSet<NodeKey>,
    pub items: Vec<BrowserItem>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub viewer: Option<SnapViewerData>,
    pub status_message: Option<String>,
}

impl SnapsApp {
    pub fn new() -> Self {
        let all_snaps = snapshots::list_snapshots();
        let mut app = SnapsApp {
            all_snaps,
            expanded: HashSet::new(),
            items: Vec::new(),
            cursor: 0,
            scroll_offset: 0,
            viewer: None,
            status_message: None,
        };
        app.auto_expand_latest();
        app.rebuild_items();
        app
    }

    fn auto_expand_latest(&mut self) {
        if let Some((dt, _)) = self.all_snaps.last() {
            let local = dt.with_timezone(&Local);
            let y = local.year();
            let m = local.month();
            let d = local.day();
            self.expanded.insert(NodeKey::Year(y));
            self.expanded.insert(NodeKey::Month(y, m));
            self.expanded.insert(NodeKey::Day(y, m, d));
        }
    }

    pub fn rebuild_items(&mut self) {
        self.items.clear();
        let mut by_year: BTreeMap<
            i32,
            BTreeMap<u32, BTreeMap<u32, Vec<(DateTime<Utc>, PathBuf)>>>,
        > = BTreeMap::new();
        for (dt, path) in &self.all_snaps {
            let local = dt.with_timezone(&Local);
            by_year
                .entry(local.year())
                .or_default()
                .entry(local.month())
                .or_default()
                .entry(local.day())
                .or_default()
                .push((*dt, path.clone()));
        }
        for (year, months) in by_year.iter().rev() {
            let year_exp = self.expanded.contains(&NodeKey::Year(*year));
            self.items.push(BrowserItem {
                node: BrowserNode::Year(*year),
                depth: 0,
                expanded: year_exp,
            });
            if !year_exp {
                continue;
            }
            for (month, days) in months.iter().rev() {
                let month_exp = self.expanded.contains(&NodeKey::Month(*year, *month));
                self.items.push(BrowserItem {
                    node: BrowserNode::Month(*year, *month),
                    depth: 1,
                    expanded: month_exp,
                });
                if !month_exp {
                    continue;
                }
                for (day, snaps) in days.iter().rev() {
                    let day_exp =
                        self.expanded.contains(&NodeKey::Day(*year, *month, *day));
                    self.items.push(BrowserItem {
                        node: BrowserNode::Day(*year, *month, *day),
                        depth: 2,
                        expanded: day_exp,
                    });
                    if !day_exp {
                        continue;
                    }
                    for (dt, path) in snaps.iter().rev() {
                        self.items.push(BrowserItem {
                            node: BrowserNode::Snap {
                                taken_at: *dt,
                                path: path.clone(),
                            },
                            depth: 3,
                            expanded: false,
                        });
                    }
                }
            }
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor + 1 < self.items.len() {
            self.cursor += 1;
        }
    }

    pub fn toggle_or_open(&mut self) {
        let node = match self.items.get(self.cursor) {
            Some(i) => i.node.clone(),
            None => return,
        };
        match node {
            BrowserNode::Year(y) => self.toggle_key(NodeKey::Year(y)),
            BrowserNode::Month(y, m) => self.toggle_key(NodeKey::Month(y, m)),
            BrowserNode::Day(y, m, d) => self.toggle_key(NodeKey::Day(y, m, d)),
            BrowserNode::Snap { taken_at, path } => self.open_viewer(&taken_at, &path),
        }
    }

    fn toggle_key(&mut self, key: NodeKey) {
        if self.expanded.contains(&key) {
            self.expanded.remove(&key);
        } else {
            self.expanded.insert(key);
        }
        self.rebuild_items();
        self.clamp_cursor();
    }

    pub fn collapse_current(&mut self) {
        let node = match self.items.get(self.cursor) {
            Some(i) => i.node.clone(),
            None => return,
        };
        let key_opt: Option<NodeKey> = match &node {
            BrowserNode::Year(y) if self.expanded.contains(&NodeKey::Year(*y)) => {
                Some(NodeKey::Year(*y))
            }
            BrowserNode::Month(y, m)
                if self.expanded.contains(&NodeKey::Month(*y, *m)) =>
            {
                Some(NodeKey::Month(*y, *m))
            }
            BrowserNode::Day(y, m, d)
                if self.expanded.contains(&NodeKey::Day(*y, *m, *d)) =>
            {
                Some(NodeKey::Day(*y, *m, *d))
            }
            BrowserNode::Snap { taken_at, .. } => {
                let local = taken_at.with_timezone(&Local);
                Some(NodeKey::Day(local.year(), local.month(), local.day()))
            }
            _ => None,
        };
        if let Some(key) = key_opt {
            let was_snap = matches!(node, BrowserNode::Snap { .. });
            self.expanded.remove(&key);
            self.rebuild_items();
            if was_snap {
                for i in (0..self.cursor.min(self.items.len())).rev() {
                    if matches!(&self.items[i].node, BrowserNode::Day(..)) {
                        self.cursor = i;
                        break;
                    }
                }
            }
            self.clamp_cursor();
        }
    }

    fn open_viewer(&mut self, taken_at: &DateTime<Utc>, path: &PathBuf) {
        if let Some(snap) = load_snapshot(path) {
            let collapsed: HashSet<Uuid> = snap.collapsed.iter().copied().collect();
            let label = taken_at
                .with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string();
            self.viewer = Some(SnapViewerData {
                tasks: snap.tasks,
                projects: snap.projects,
                collapsed,
                scroll_offset: 0,
                label,
            });
        } else {
            self.status_message = Some("Failed to load snapshot".to_string());
        }
    }

    pub fn close_viewer(&mut self) {
        self.viewer = None;
    }

    pub fn viewer_scroll_down(&mut self) {
        if let Some(ref mut v) = self.viewer {
            v.scroll_offset += 1;
        }
    }

    pub fn viewer_scroll_up(&mut self) {
        if let Some(ref mut v) = self.viewer {
            v.scroll_offset = v.scroll_offset.saturating_sub(1);
        }
    }

    fn clamp_cursor(&mut self) {
        if self.items.is_empty() {
            self.cursor = 0;
        } else if self.cursor >= self.items.len() {
            self.cursor = self.items.len() - 1;
        }
    }
}

pub fn compute_browser_scroll(cursor: usize, current: usize, height: usize) -> usize {
    const SCROLLOFF: usize = 3;
    if height == 0 {
        return 0;
    }
    let min_scroll = (cursor + SCROLLOFF + 1).saturating_sub(height);
    let max_scroll = cursor.saturating_sub(SCROLLOFF);
    if min_scroll > max_scroll {
        min_scroll
    } else {
        current.clamp(min_scroll, max_scroll)
    }
}

pub fn draw_snaps(frame: &mut Frame, app: &mut SnapsApp) {
    let area = frame.area();

    if app.viewer.is_some() {
        let viewer = app.viewer.as_mut().unwrap();
        draw_full_viewer(frame, viewer, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let count = app.all_snaps.len();
    let count_label = format!(
        "  {} snapshot{}",
        count,
        if count == 1 { "" } else { "s" }
    );
    let mut header_spans = vec![
        Span::styled(
            " SNAPS ",
            Style::default().fg(Color::Black).bg(Color::Indexed(33)),
        ),
        Span::styled(count_label, Style::default().fg(Color::Indexed(244))),
    ];
    if let Some(msg) = &app.status_message {
        header_spans.push(Span::styled(
            format!("  {}", msg),
            Style::default().fg(Color::Indexed(208)),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(header_spans)), chunks[0]);

    let area_height = chunks[1].height as usize;
    app.scroll_offset = compute_browser_scroll(app.cursor, app.scroll_offset, area_height);

    if app.items.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  No snapshots yet. Open zen tui and press S to save one.",
                Style::default().fg(Color::Indexed(244)),
            ))),
            chunks[1],
        );
    } else {
        let mut y = chunks[1].y;
        for (idx, item) in app
            .items
            .iter()
            .enumerate()
            .skip(app.scroll_offset)
            .take(area_height)
        {
            let is_sel = idx == app.cursor;
            let bg = if is_sel { Color::Indexed(238) } else { Color::Reset };

            let indent = "  ".repeat(item.depth);
            let (symbol, label, label_fg) = match &item.node {
                BrowserNode::Year(y) => {
                    let sym = if item.expanded { "▾ " } else { "▸ " };
                    (sym.to_string(), y.to_string(), Color::Indexed(214))
                }
                BrowserNode::Month(_, m) => {
                    let sym = if item.expanded { "▾ " } else { "▸ " };
                    let names = [
                        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep",
                        "Oct", "Nov", "Dec",
                    ];
                    let name = names
                        .get((*m as usize).wrapping_sub(1))
                        .copied()
                        .unwrap_or("???");
                    (sym.to_string(), format!("{:02} — {}", m, name), Color::Indexed(179))
                }
                BrowserNode::Day(_, _, d) => {
                    let sym = if item.expanded { "▾ " } else { "▸ " };
                    (sym.to_string(), format!("{:02}", d), Color::Indexed(252))
                }
                BrowserNode::Snap { taken_at, .. } => {
                    let time_str = taken_at
                        .with_timezone(&Local)
                        .format("%H:%M:%S")
                        .to_string();
                    ("  ".to_string(), time_str, Color::Indexed(248))
                }
            };

            let label_style = if is_sel {
                Style::default()
                    .fg(Color::Indexed(252))
                    .bg(bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(label_fg)
            };
            let sym_style = Style::default().fg(Color::Indexed(214)).bg(bg);

            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(format!("{}{}", indent, symbol), sym_style),
                    Span::styled(label, label_style),
                ]))
                .style(Style::default().bg(bg)),
                Rect { x: chunks[1].x, y, width: chunks[1].width, height: 1 },
            );
            y += 1;
        }
    }

    let sep = Span::styled("  │  ", Style::default().fg(Color::Indexed(240)));
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " SNAPS ",
                Style::default().fg(Color::Black).bg(Color::Indexed(33)),
            ),
            sep.clone(),
            Span::styled("j/k", Style::default().fg(Color::Yellow)),
            Span::styled(" navigate", Style::default().fg(Color::Indexed(244))),
            sep.clone(),
            Span::styled("l/Enter", Style::default().fg(Color::Yellow)),
            Span::styled(" expand/open", Style::default().fg(Color::Indexed(244))),
            sep.clone(),
            Span::styled("h", Style::default().fg(Color::Yellow)),
            Span::styled(" collapse", Style::default().fg(Color::Indexed(244))),
            sep.clone(),
            Span::styled("q", Style::default().fg(Color::Yellow)),
            Span::styled(" quit", Style::default().fg(Color::Indexed(244))),
        ])),
        chunks[2],
    );
}

fn draw_full_viewer(frame: &mut Frame, viewer: &mut SnapViewerData, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " SNAP ",
                Style::default().fg(Color::Black).bg(Color::Indexed(242)),
            ),
            Span::styled(
                format!("  {}  (read-only)", viewer.label),
                Style::default().fg(Color::Indexed(244)),
            ),
        ])),
        chunks[0],
    );

    draw_viewer_rows(frame, viewer, chunks[1]);

    let sep = Span::styled("  │  ", Style::default().fg(Color::Indexed(240)));
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " SNAP ",
                Style::default().fg(Color::Black).bg(Color::Indexed(242)),
            ),
            sep.clone(),
            Span::styled("j/k", Style::default().fg(Color::Yellow)),
            Span::styled(" scroll", Style::default().fg(Color::Indexed(244))),
            sep,
            Span::styled("q/Esc", Style::default().fg(Color::Yellow)),
            Span::styled(" back to browser", Style::default().fg(Color::Indexed(244))),
        ])),
        chunks[2],
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vert[1])[1]
}
