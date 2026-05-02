use chrono::{DateTime, Local, Utc};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};
use uuid::Uuid;

use crate::types::{Status, Task};
use super::board::project_to_color;
use super::{pill_span, slot_key_char, unc_pill_span};

#[derive(Clone, Copy, PartialEq)]
pub enum SortBy {
    Date,
    Duration,
    Project,
}

pub struct DoneApp {
    pub tasks: Vec<Task>,
    pub projects: [Option<String>; 10],
    pub active_slots: [bool; 10],
    pub show_unc: bool,
    pub cursor: usize,
    pub sort_by: SortBy,
    pub table_state: TableState,
}

impl DoneApp {
    pub fn new(tasks: Vec<Task>, projects: [Option<String>; 10]) -> Self {
        let mut state = TableState::default();
        state.select(Some(0));
        Self {
            tasks,
            projects,
            active_slots: [true; 10],
            show_unc: true,
            cursor: 0,
            sort_by: SortBy::Date,
            table_state: state,
        }
    }

    pub fn slot_for_project(&self, project: &str) -> Option<usize> {
        if project.is_empty() { return None; }
        self.projects.iter().position(|p| p.as_deref() == Some(project))
    }

    pub fn is_unc(&self, task: &Task) -> bool {
        task.project.is_empty() || self.slot_for_project(&task.project).is_none()
    }

    fn visible(&self, task: &Task) -> bool {
        if task.status != Status::Done { return false; }
        if self.is_unc(task) { self.show_unc } else { self.active_slots[self.slot_for_project(&task.project).unwrap()] }
    }

    pub fn done_tasks(&self) -> Vec<&Task> {
        let mut tasks: Vec<&Task> = self.tasks.iter().filter(|t| self.visible(t)).collect();
        match self.sort_by {
            SortBy::Date => tasks.sort_by(|a, b| completed_at(b).cmp(&completed_at(a))),
            SortBy::Duration => tasks.sort_by(|a, b| elapsed_to_done(b).cmp(&elapsed_to_done(a))),
            SortBy::Project => tasks.sort_by(|a, b| a.project.cmp(&b.project).then(a.title.cmp(&b.title))),
        }
        tasks
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 { self.cursor -= 1; }
        self.table_state.select(Some(self.cursor));
    }

    pub fn move_down(&mut self) {
        let max = self.done_tasks().len().saturating_sub(1);
        if self.cursor < max { self.cursor += 1; }
        self.table_state.select(Some(self.cursor));
    }

    pub fn toggle_slot(&mut self, slot: usize) {
        if self.projects[slot].is_some() {
            self.active_slots[slot] = !self.active_slots[slot];
        }
        self.clamp();
    }

    pub fn toggle_unc(&mut self) {
        self.show_unc = !self.show_unc;
        self.clamp();
    }

    pub fn cycle_sort(&mut self) {
        self.sort_by = match self.sort_by {
            SortBy::Date => SortBy::Duration,
            SortBy::Duration => SortBy::Project,
            SortBy::Project => SortBy::Date,
        };
        self.clamp();
    }

    fn clamp(&mut self) {
        let len = self.done_tasks().len();
        if len == 0 { self.cursor = 0; } else if self.cursor >= len { self.cursor = len - 1; }
        self.table_state.select(Some(self.cursor));
    }
}

pub fn completed_at(task: &Task) -> Option<DateTime<Utc>> {
    task.transitions.iter().filter(|t| t.to == Status::Done).last().map(|t| t.at)
}

pub fn elapsed_to_done(task: &Task) -> Option<i64> {
    completed_at(task).map(|end| (end - task.created_at).num_seconds().max(0))
}

pub fn format_duration(secs: i64) -> String {
    if secs <= 0 { return "—".into(); }
    if secs < 60 { return format!("{}s", secs); }
    if secs < 3600 { return format!("{}m", secs / 60); }
    if secs < 86400 {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        return if m == 0 { format!("{}h", h) } else { format!("{}h {}m", h, m) };
    }
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    if h == 0 { format!("{}d", d) } else { format!("{}d {}h", d, h) }
}

pub fn format_relative(dt: Option<DateTime<Utc>>) -> String {
    let Some(t) = dt else { return "—".into(); };
    let secs = (Utc::now() - t).num_seconds();
    if secs < 60 { return "just now".into(); }
    if secs < 3600 { return format!("{}m ago", secs / 60); }
    if secs < 86400 { return format!("{}h ago", secs / 3600); }
    if secs < 7 * 86400 { return format!("{}d ago", secs / 86400); }
    t.with_timezone(&Local).format("%b %d").to_string()
}

struct RowData {
    project: String,
    parent_title: Option<String>,
    title: String,
    elapsed: Option<i64>,
    completed: Option<DateTime<Utc>>,
}

fn parent_title(tasks: &[Task], parent_id: Option<Uuid>) -> Option<String> {
    let pid = parent_id?;
    tasks.iter().find(|t| t.id == pid).map(|p| {
        if p.title.chars().count() > 22 {
            format!("{}…", p.title.chars().take(21).collect::<String>())
        } else {
            p.title.clone()
        }
    })
}

pub fn draw_done(frame: &mut Frame, app: &mut DoneApp) {
    let area = frame.area();

    // Collect all display data while tasks is borrowed
    let (total, unc_count, project_counts, rows): (usize, usize, Vec<usize>, Vec<RowData>) = {
        let done = app.done_tasks();
        let total = done.len();
        let unc_count = app.tasks.iter().filter(|t| t.status == Status::Done && app.is_unc(t)).count();
        let project_counts: Vec<usize> = (0..10).map(|slot| {
            app.projects[slot].as_ref().map_or(0, |name| {
                app.tasks.iter().filter(|t| t.status == Status::Done && &t.project == name).count()
            })
        }).collect();
        let rows: Vec<RowData> = done.iter().map(|t| RowData {
            project: t.project.clone(),
            parent_title: parent_title(&app.tasks, t.parent_id),
            title: t.title.clone(),
            elapsed: elapsed_to_done(t),
            completed: completed_at(t),
        }).collect();
        (total, unc_count, project_counts, rows)
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    // Header
    let sort_label = match app.sort_by {
        SortBy::Date => "date",
        SortBy::Duration => "duration",
        SortBy::Project => "project",
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" DONE ({}) ", total), Style::default().fg(Color::Black).bg(Color::Indexed(242)).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  sort: {} [s to cycle]", sort_label), Style::default().fg(Color::Indexed(244))),
        ])),
        chunks[0],
    );

    // Pill row
    let sep = Span::styled("│", Style::default().fg(Color::Indexed(240)));
    let mut pill_spans: Vec<Span<'static>> = vec![];
    let has_unc = app.tasks.iter().any(|t| t.status == Status::Done && app.is_unc(t));
    if has_unc {
        pill_spans.push(unc_pill_span(unc_count, app.show_unc));
        pill_spans.push(sep.clone());
    }
    for slot in 0..10 {
        if let Some(name) = &app.projects[slot] {
            let color = project_to_color(name);
            pill_spans.push(pill_span(slot_key_char(slot), name, project_counts[slot], app.active_slots[slot], color));
            pill_spans.push(sep.clone());
        }
    }
    frame.render_widget(Paragraph::new(Line::from(pill_spans)), chunks[1]);

    // Table
    let table_rows: Vec<Row> = rows.iter().map(|r| {
        let color = if r.project.is_empty() { Color::Indexed(102) } else { project_to_color(&r.project) };
        let title_text = match &r.parent_title {
            Some(p) => format!("{}: {}", p, r.title),
            None => r.title.clone(),
        };
        Row::new(vec![
            Cell::from(format!(" {} ", title_text)).style(Style::default().fg(Color::Black).bg(color)),
            Cell::from(r.elapsed.map(format_duration).unwrap_or_else(|| "—".into()))
                .style(Style::default().fg(Color::Indexed(250))),
            Cell::from(format_relative(r.completed))
                .style(Style::default().fg(Color::Indexed(244))),
        ])
    }).collect();

    let widths = [Constraint::Min(20), Constraint::Length(10), Constraint::Length(12)];
    let table = Table::new(table_rows, widths)
        .row_highlight_style(Style::default().add_modifier(Modifier::BOLD).bg(Color::Indexed(237)))
        .highlight_symbol("> ");
    frame.render_stateful_widget(table, chunks[2], &mut app.table_state);

    // Footer
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" DONE ", Style::default().fg(Color::Black).bg(Color::Indexed(242))),
            Span::styled("│", Style::default().fg(Color::Indexed(240))),
            Span::styled(" j/k scroll ", Style::default().fg(Color::Indexed(244))),
            Span::styled("│", Style::default().fg(Color::Indexed(240))),
            Span::styled(" 1-9/0 filter ", Style::default().fg(Color::Indexed(244))),
            Span::styled("│", Style::default().fg(Color::Indexed(240))),
            Span::styled(" ` unc ", Style::default().fg(Color::Indexed(244))),
            Span::styled("│", Style::default().fg(Color::Indexed(240))),
            Span::styled(" s sort ", Style::default().fg(Color::Indexed(244))),
            Span::styled("│", Style::default().fg(Color::Indexed(240))),
            Span::styled(" q quit", Style::default().fg(Color::Indexed(244))),
        ])),
        chunks[3],
    );
}
