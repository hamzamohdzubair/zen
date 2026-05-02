use chrono::Utc;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};

use crate::types::{Status, Task};
use super::board::project_to_color;
use super::done::{completed_at, elapsed_to_done, format_duration, format_relative};

#[derive(Clone, PartialEq)]
pub enum StatsView {
    Overview,
    Project(usize), // slot index; usize::MAX = unclassified
}

pub struct ProjectStats {
    pub name: String,
    pub color: Color,
    pub slot: Option<usize>,
    pub done: usize,
    pub in_flight: usize,
    pub avg_secs: Option<i64>,
    pub median_secs: Option<i64>,
    pub fastest_secs: Option<i64>,
    pub slowest_secs: Option<i64>,
}

pub struct StatsApp {
    pub tasks: Vec<Task>,
    pub projects: [Option<String>; 10],
    pub view: StatsView,
    pub cursor: usize,
    pub table_state: TableState,
    pub rows: Vec<ProjectStats>,
}

impl StatsApp {
    pub fn new(tasks: Vec<Task>, projects: [Option<String>; 10]) -> Self {
        let rows = compute_rows(&tasks, &projects);
        let mut state = TableState::default();
        if !rows.is_empty() { state.select(Some(0)); }
        Self { tasks, projects, view: StatsView::Overview, cursor: 0, table_state: state, rows }
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 { self.cursor -= 1; }
        self.table_state.select(Some(self.cursor));
    }

    pub fn move_down(&mut self) {
        if self.cursor + 1 < self.rows.len() { self.cursor += 1; }
        self.table_state.select(Some(self.cursor));
    }

    pub fn zoom_in(&mut self) {
        if let Some(row) = self.rows.get(self.cursor) {
            self.view = StatsView::Project(row.slot.unwrap_or(usize::MAX));
        }
    }

    pub fn zoom_out(&mut self) {
        self.view = StatsView::Overview;
    }
}

fn named_projects(projects: &[Option<String>; 10]) -> Vec<&str> {
    projects.iter().filter_map(|p| p.as_deref()).collect()
}

fn is_unc(task: &Task, named: &[&str]) -> bool {
    task.project.is_empty() || !named.contains(&task.project.as_str())
}

fn compute_stats(elapsed_times: &[i64]) -> (Option<i64>, Option<i64>, Option<i64>, Option<i64>) {
    if elapsed_times.is_empty() { return (None, None, None, None); }
    let mut sorted = elapsed_times.to_vec();
    sorted.sort_unstable();
    let avg = Some(sorted.iter().sum::<i64>() / sorted.len() as i64);
    let median = Some(sorted[sorted.len() / 2]);
    let fastest = sorted.first().copied();
    let slowest = sorted.last().copied();
    (avg, median, fastest, slowest)
}

fn compute_rows(tasks: &[Task], projects: &[Option<String>; 10]) -> Vec<ProjectStats> {
    let mut rows = vec![];
    let named = named_projects(projects);

    for (slot, name_opt) in projects.iter().enumerate() {
        let Some(name) = name_opt else { continue; };
        let project_tasks: Vec<&Task> = tasks.iter().filter(|t| &t.project == name).collect();
        if project_tasks.is_empty() { continue; }

        let done_tasks: Vec<&Task> = project_tasks.iter().filter(|t| t.status == Status::Done).copied().collect();
        let in_flight = project_tasks.iter().filter(|t| matches!(t.status, Status::Todo | Status::Doing)).count();
        let elapsed: Vec<i64> = done_tasks.iter().filter_map(|t| elapsed_to_done(t)).collect();
        let (avg, median, fastest, slowest) = compute_stats(&elapsed);

        rows.push(ProjectStats {
            name: name.clone(),
            color: project_to_color(name),
            slot: Some(slot),
            done: done_tasks.len(),
            in_flight,
            avg_secs: avg,
            median_secs: median,
            fastest_secs: fastest,
            slowest_secs: slowest,
        });
    }

    // Unclassified
    let unc_tasks: Vec<&Task> = tasks.iter().filter(|t| is_unc(t, &named)).collect();
    if !unc_tasks.is_empty() {
        let done_tasks: Vec<&Task> = unc_tasks.iter().filter(|t| t.status == Status::Done).copied().collect();
        let in_flight = unc_tasks.iter().filter(|t| matches!(t.status, Status::Todo | Status::Doing)).count();
        let elapsed: Vec<i64> = done_tasks.iter().filter_map(|t| elapsed_to_done(t)).collect();
        let (avg, median, fastest, slowest) = compute_stats(&elapsed);
        rows.push(ProjectStats {
            name: "unclassified".into(),
            color: Color::Indexed(102),
            slot: None,
            done: done_tasks.len(),
            in_flight,
            avg_secs: avg,
            median_secs: median,
            fastest_secs: fastest,
            slowest_secs: slowest,
        });
    }

    rows
}

pub fn draw_stats(frame: &mut Frame, app: &mut StatsApp) {
    let area = frame.area();
    match app.view.clone() {
        StatsView::Overview => draw_overview(frame, app, area),
        StatsView::Project(key) => draw_project(frame, app, key, area),
    }
}

fn draw_overview(frame: &mut Frame, app: &mut StatsApp, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" STATS ", Style::default().fg(Color::Black).bg(Color::Indexed(74)).add_modifier(Modifier::BOLD)),
            Span::styled(" — Project Overview", Style::default().fg(Color::Indexed(250))),
            Span::styled("    Enter: zoom in  q: quit", Style::default().fg(Color::Indexed(240))),
        ])),
        chunks[0],
    );

    let dim = Style::default().fg(Color::Indexed(246)).add_modifier(Modifier::BOLD);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("   Project            ", dim),
            Span::styled("Done  ", dim),
            Span::styled("Active  ", dim),
            Span::styled("Avg time    ", dim),
            Span::styled("Median      ", dim),
            Span::styled("Fastest     ", dim),
            Span::styled("Slowest    ", dim),
        ])),
        chunks[1],
    );

    // Build rows
    let table_rows: Vec<Row> = app.rows.iter().map(|r| {
        Row::new(vec![
            Cell::from(format!(" {} ", r.name)).style(Style::default().fg(Color::Black).bg(r.color).add_modifier(Modifier::BOLD)),
            Cell::from(format!("{:>4}  ", r.done)).style(Style::default().fg(Color::Indexed(252))),
            Cell::from(format!("{:>4}    ", r.in_flight)).style(Style::default().fg(Color::Indexed(178))),
            Cell::from(r.avg_secs.map(format_duration).unwrap_or_else(|| "—".into())).style(Style::default().fg(Color::Indexed(252))),
            Cell::from(r.median_secs.map(format_duration).unwrap_or_else(|| "—".into())).style(Style::default().fg(Color::Indexed(252))),
            Cell::from(r.fastest_secs.map(format_duration).unwrap_or_else(|| "—".into())).style(Style::default().fg(Color::Indexed(108))),
            Cell::from(r.slowest_secs.map(format_duration).unwrap_or_else(|| "—".into())).style(Style::default().fg(Color::Indexed(174))),
        ])
    }).collect();

    // Totals row
    let all_elapsed: Vec<i64> = app.tasks.iter()
        .filter(|t| t.status == Status::Done)
        .filter_map(|t| elapsed_to_done(t))
        .collect();
    let total_done: usize = app.rows.iter().map(|r| r.done).sum();
    let total_active: usize = app.rows.iter().map(|r| r.in_flight).sum();
    let (total_avg, _, total_fast, total_slow) = compute_stats(&all_elapsed);

    let sep_style = Style::default().fg(Color::Indexed(238));
    let mut all_rows = table_rows;
    all_rows.push(Row::new(vec![
        Cell::from("─────────────────────").style(sep_style),
        Cell::from("──────").style(sep_style),
        Cell::from("────────").style(sep_style),
        Cell::from("────────────").style(sep_style),
        Cell::from("────────────").style(sep_style),
        Cell::from("────────────").style(sep_style),
        Cell::from("───────────").style(sep_style),
    ]));
    let bold252 = Style::default().fg(Color::Indexed(252)).add_modifier(Modifier::BOLD);
    all_rows.push(Row::new(vec![
        Cell::from(format!(" Total ({} projects)", app.rows.len())).style(bold252),
        Cell::from(format!("{:>4}  ", total_done)).style(bold252),
        Cell::from(format!("{:>4}    ", total_active)).style(Style::default().fg(Color::Indexed(178)).add_modifier(Modifier::BOLD)),
        Cell::from(total_avg.map(format_duration).unwrap_or_else(|| "—".into())).style(bold252),
        Cell::from("").style(Style::default()),
        Cell::from(total_fast.map(format_duration).unwrap_or_else(|| "—".into())).style(Style::default().fg(Color::Indexed(108)).add_modifier(Modifier::BOLD)),
        Cell::from(total_slow.map(format_duration).unwrap_or_else(|| "—".into())).style(Style::default().fg(Color::Indexed(174)).add_modifier(Modifier::BOLD)),
    ]));

    let widths = [
        Constraint::Length(22),
        Constraint::Length(6),
        Constraint::Length(8),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Min(10),
    ];
    let table = Table::new(all_rows, widths)
        .row_highlight_style(Style::default().bg(Color::Indexed(236)).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");
    frame.render_stateful_widget(table, chunks[2], &mut app.table_state);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" STATS ", Style::default().fg(Color::Black).bg(Color::Indexed(74))),
            Span::styled("│", Style::default().fg(Color::Indexed(240))),
            Span::styled(" j/k navigate ", Style::default().fg(Color::Indexed(244))),
            Span::styled("│", Style::default().fg(Color::Indexed(240))),
            Span::styled(" Enter zoom in ", Style::default().fg(Color::Indexed(244))),
            Span::styled("│", Style::default().fg(Color::Indexed(240))),
            Span::styled(" q quit", Style::default().fg(Color::Indexed(244))),
        ])),
        chunks[3],
    );
}

fn draw_project(frame: &mut Frame, app: &StatsApp, key: usize, area: Rect) {
    let Some(row) = app.rows.iter().find(|r| r.slot.unwrap_or(usize::MAX) == key) else { return; };

    let named = named_projects(&app.projects);
    let done_tasks: Vec<&Task> = app.tasks.iter()
        .filter(|t| t.status == Status::Done)
        .filter(|t| {
            if key == usize::MAX { is_unc(t, &named) }
            else { app.projects[key].as_deref() == Some(&t.project) }
        })
        .collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // header
            Constraint::Length(1),  // summary
            Constraint::Length(1),  // spacer
            Constraint::Length(1),  // chart label
            Constraint::Length(9),  // weekly chart (8 bars + 1 padding)
            Constraint::Length(1),  // recent label
            Constraint::Min(0),     // recent tasks
            Constraint::Length(1),  // footer
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" {} ", row.name), Style::default().fg(Color::Black).bg(row.color).add_modifier(Modifier::BOLD)),
            Span::styled(" — Project Stats", Style::default().fg(Color::Indexed(250))),
            Span::styled("    Esc: back to overview  q: quit", Style::default().fg(Color::Indexed(240))),
        ])),
        chunks[0],
    );

    let mut elapsed_sorted: Vec<i64> = done_tasks.iter().filter_map(|t| elapsed_to_done(t)).collect();
    elapsed_sorted.sort_unstable();
    let (avg, median, _, _) = compute_stats(&elapsed_sorted);
    let sep = Span::styled(" │ ", Style::default().fg(Color::Indexed(240)));
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" Done: {} ", row.done), Style::default().fg(Color::Indexed(252)).add_modifier(Modifier::BOLD)),
            sep.clone(),
            Span::styled(format!("Active: {} ", row.in_flight), Style::default().fg(Color::Indexed(178))),
            sep.clone(),
            Span::styled(format!("Avg: {} ", avg.map(format_duration).unwrap_or_else(|| "—".into())), Style::default().fg(Color::Indexed(252))),
            sep.clone(),
            Span::styled(format!("Median: {} ", median.map(format_duration).unwrap_or_else(|| "—".into())), Style::default().fg(Color::Indexed(252))),
            sep.clone(),
            Span::styled(format!("Fastest: {} ", row.fastest_secs.map(format_duration).unwrap_or_else(|| "—".into())), Style::default().fg(Color::Indexed(108))),
            sep.clone(),
            Span::styled(format!("Slowest: {} ", row.slowest_secs.map(format_duration).unwrap_or_else(|| "—".into())), Style::default().fg(Color::Indexed(174))),
        ])),
        chunks[1],
    );

    frame.render_widget(
        Paragraph::new(Span::styled(" Completion rate — last 8 weeks", Style::default().fg(Color::Indexed(244)))),
        chunks[3],
    );
    draw_weekly_chart(frame, &done_tasks, chunks[4], row.color);

    frame.render_widget(
        Paragraph::new(Span::styled(" Recent completions", Style::default().fg(Color::Indexed(244)))),
        chunks[5],
    );

    let mut sorted_done = done_tasks.clone();
    sorted_done.sort_by(|a, b| completed_at(b).cmp(&completed_at(a)));
    let recent_lines: Vec<Line> = sorted_done.iter()
        .take(chunks[6].height as usize)
        .map(|t| Line::from(vec![
            Span::styled(format!(" {} ", t.title), Style::default().fg(Color::Black).bg(row.color)),
            Span::raw("  "),
            Span::styled(elapsed_to_done(t).map(format_duration).unwrap_or_else(|| "—".into()), Style::default().fg(Color::Indexed(246))),
            Span::raw("   "),
            Span::styled(format_relative(completed_at(t)), Style::default().fg(Color::Indexed(244))),
        ]))
        .collect();
    frame.render_widget(Paragraph::new(recent_lines), chunks[6]);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" {} ", row.name), Style::default().fg(Color::Black).bg(row.color)),
            Span::styled("│", Style::default().fg(Color::Indexed(240))),
            Span::styled(" Esc back to overview ", Style::default().fg(Color::Indexed(244))),
            Span::styled("│", Style::default().fg(Color::Indexed(240))),
            Span::styled(" q quit", Style::default().fg(Color::Indexed(244))),
        ])),
        chunks[7],
    );
}

fn draw_weekly_chart(frame: &mut Frame, done_tasks: &[&Task], area: Rect, color: Color) {
    let now = Utc::now();
    const WEEKS: usize = 8;
    let mut counts = [0usize; WEEKS];

    for task in done_tasks {
        if let Some(c) = completed_at(task) {
            let weeks_ago = (now - c).num_weeks() as usize;
            if weeks_ago < WEEKS {
                counts[WEEKS - 1 - weeks_ago] += 1;
            }
        }
    }

    let max = *counts.iter().max().unwrap_or(&1);
    let bar_area = area.width.saturating_sub(22) as usize;

    let lines: Vec<Line> = counts.iter().enumerate().map(|(i, &count)| {
        let weeks_ago = WEEKS - 1 - i;
        let label = match weeks_ago {
            0 => "this week".to_string(),
            1 => "last week".to_string(),
            n => format!("{} weeks ago", n),
        };
        let bar_len = if max > 0 { (count * bar_area) / max } else { 0 };
        Line::from(vec![
            Span::styled(format!("{:>13}  ", label), Style::default().fg(Color::Indexed(244))),
            Span::styled("█".repeat(bar_len), Style::default().fg(color)),
            Span::styled(format!(" {}", count), Style::default().fg(Color::Indexed(252))),
        ])
    }).collect();

    frame.render_widget(Paragraph::new(lines), area);
}
