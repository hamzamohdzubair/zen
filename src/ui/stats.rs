use chrono::Utc;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::types::{Status, Task};
use super::done::{completed_at, elapsed_to_done, format_duration, format_relative};

pub struct StatsApp {
    pub tasks: Vec<Task>,
}

impl StatsApp {
    pub fn new(tasks: Vec<Task>) -> Self {
        Self { tasks }
    }
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

pub fn draw_stats(frame: &mut Frame, app: &mut StatsApp) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(9),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let todo_count = app.tasks.iter().filter(|t| t.status == Status::Todo).count();
    let doing_count = app.tasks.iter().filter(|t| t.status == Status::Doing).count();
    let done_tasks: Vec<&Task> = app.tasks.iter().filter(|t| t.status == Status::Done).collect();
    let done_count = done_tasks.len();

    let elapsed: Vec<i64> = done_tasks.iter().filter_map(|t| elapsed_to_done(t)).collect();
    let (avg, median, fastest, slowest) = compute_stats(&elapsed);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" STATS ", Style::default().fg(Color::Black).bg(Color::Indexed(74)).add_modifier(Modifier::BOLD)),
            Span::styled("    q: quit", Style::default().fg(Color::Indexed(240))),
        ])),
        chunks[0],
    );

    frame.render_widget(Paragraph::new(""), chunks[1]);

    let sep = Span::styled("  │  ", Style::default().fg(Color::Indexed(240)));
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" Todo: {} ", todo_count), Style::default().fg(Color::Indexed(252)).add_modifier(Modifier::BOLD)),
            sep.clone(),
            Span::styled(format!(" Doing: {} ", doing_count), Style::default().fg(Color::Indexed(214)).add_modifier(Modifier::BOLD)),
            sep.clone(),
            Span::styled(format!(" Done: {} ", done_count), Style::default().fg(Color::Indexed(108)).add_modifier(Modifier::BOLD)),
        ])),
        chunks[2],
    );

    let fmt = |v: Option<i64>| v.map(format_duration).unwrap_or_else(|| "—".into());
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" Avg: {} ", fmt(avg)), Style::default().fg(Color::Indexed(252))),
            sep.clone(),
            Span::styled(format!(" Median: {} ", fmt(median)), Style::default().fg(Color::Indexed(252))),
            sep.clone(),
            Span::styled(format!(" Fastest: {} ", fmt(fastest)), Style::default().fg(Color::Indexed(108))),
            sep.clone(),
            Span::styled(format!(" Slowest: {} ", fmt(slowest)), Style::default().fg(Color::Indexed(174))),
        ])),
        chunks[3],
    );

    frame.render_widget(
        Paragraph::new(Span::styled(" Completions — last 8 weeks", Style::default().fg(Color::Indexed(244)))),
        chunks[4],
    );
    draw_weekly_chart(frame, &done_tasks, chunks[5]);

    frame.render_widget(
        Paragraph::new(Span::styled(" Recent completions", Style::default().fg(Color::Indexed(244)))),
        chunks[6],
    );

    let mut sorted_done = done_tasks.clone();
    sorted_done.sort_by(|a, b| completed_at(b).cmp(&completed_at(a)));
    let recent_lines: Vec<Line> = sorted_done.iter()
        .take(chunks[7].height as usize)
        .map(|t| Line::from(vec![
            Span::styled(format!(" {} ", t.title), Style::default().fg(Color::Indexed(252))),
            Span::raw("  "),
            Span::styled(elapsed_to_done(t).map(format_duration).unwrap_or_else(|| "—".into()), Style::default().fg(Color::Indexed(246))),
            Span::raw("   "),
            Span::styled(format_relative(completed_at(t)), Style::default().fg(Color::Indexed(244))),
        ]))
        .collect();
    frame.render_widget(Paragraph::new(recent_lines), chunks[7]);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" STATS ", Style::default().fg(Color::Black).bg(Color::Indexed(74))),
            Span::styled("│", Style::default().fg(Color::Indexed(240))),
            Span::styled(" q quit", Style::default().fg(Color::Indexed(244))),
        ])),
        chunks[8],
    );
}

fn draw_weekly_chart(frame: &mut Frame, done_tasks: &[&Task], area: Rect) {
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
            Span::styled("█".repeat(bar_len), Style::default().fg(Color::Indexed(74))),
            Span::styled(format!(" {}", count), Style::default().fg(Color::Indexed(252))),
        ])
    }).collect();

    frame.render_widget(Paragraph::new(lines), area);
}
