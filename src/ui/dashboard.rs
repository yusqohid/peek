use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Bar, BarChart, BarGroup, Block, Borders, List, ListItem, Paragraph},
};

use crate::app::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(0)])
        .split(chunks[0]);

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    render_overview(f, app, left_chunks[0]);
    render_language_chart(f, app, left_chunks[1]);
    render_top_projects(f, app, right_chunks[0]);
    render_recent_projects(f, app, right_chunks[1]);
}

/// Top-left: aggregate numbers.
fn render_overview(f: &mut Frame, app: &App, area: Rect) {
    let active_count = app.projects.iter().filter(|p| !p.ignored).count();
    let ignored_count = app.projects.iter().filter(|p| p.ignored).count();

    let text = vec![
        Line::from(vec![
            Span::styled("  Total Projects:  ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{active_count}"),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            if ignored_count > 0 {
                Span::styled(
                    format!("  ({ignored_count} ignored)"),
                    Style::default().fg(Color::DarkGray),
                )
            } else {
                Span::raw("")
            },
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Total LOC:       ", Style::default().fg(Color::Gray)),
            Span::styled(
                format_number(app.total_loc()),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Total Files:     ", Style::default().fg(Color::Gray)),
            Span::styled(
                format_number(app.total_files()),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Languages:       ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", app.unique_languages()),
                Style::default().fg(Color::Magenta),
            ),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" 📊 Overview ");
    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(paragraph, area);
}

/// Bottom-left: bar chart of languages by LOC.
fn render_language_chart(f: &mut Frame, app: &App, area: Rect) {
    // Aggregate language stats across all non-ignored projects.
    let mut lang_map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for project in app.projects.iter().filter(|p| !p.ignored) {
        if let Some(stats) = &project.code_stats {
            for lang in &stats.languages {
                *lang_map.entry(lang.name.clone()).or_default() += lang.code_lines;
            }
        }
    }

    let mut langs: Vec<(String, usize)> = lang_map.into_iter().collect();
    langs.sort_by_key(|a| std::cmp::Reverse(a.1));
    langs.truncate(8); // Top 8 languages

    let colors = [
        Color::Cyan,
        Color::Green,
        Color::Yellow,
        Color::Magenta,
        Color::Red,
        Color::Blue,
        Color::LightCyan,
        Color::LightGreen,
    ];

    let bars: Vec<Bar> = langs
        .iter()
        .enumerate()
        .map(|(i, (name, loc))| {
            let color = colors[i % colors.len()];
            Bar::default()
                .value(*loc as u64)
                .label(Line::from(truncate_str(name, 10)))
                .style(Style::default().fg(color))
        })
        .collect();

    let chart = BarChart::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" 🔤 Languages (LOC) "),
        )
        .data(BarGroup::default().bars(&bars))
        .bar_width(7)
        .bar_gap(1)
        .max(langs.first().map_or(1, |(_, v)| *v as u64));

    f.render_widget(chart, area);
}

/// Top-right: projects ranked by LOC.
fn render_top_projects(f: &mut Frame, app: &App, area: Rect) {
    let mut sorted: Vec<&_> = app.projects.iter().filter(|p| !p.ignored).collect();
    sorted.sort_by_key(|a| std::cmp::Reverse(a.total_loc()));
    sorted.truncate(10);

    let max_loc = sorted.first().map_or(1, |p| p.total_loc().max(1));

    let items: Vec<ListItem> = sorted
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let bar_len = (p.total_loc() as f64 / max_loc as f64 * 20.0) as usize;
            let bar: String = "█".repeat(bar_len);
            let pad: String = " ".repeat(20_usize.saturating_sub(bar_len));
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("  {:>2}. ", i + 1),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(format!("{:<16}", p.name), Style::default().fg(Color::White)),
                Span::styled(bar, Style::default().fg(Color::Cyan)),
                Span::raw(pad),
                Span::styled(
                    format!(" {}", format_number(p.total_loc())),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" 🔥 Top Projects (by LOC) "),
    );
    f.render_widget(list, area);
}

/// Bottom-right: most recently modified projects.
fn render_recent_projects(f: &mut Frame, app: &App, area: Rect) {
    let mut sorted: Vec<&_> = app.projects.iter().filter(|p| !p.ignored).collect();
    sorted.sort_by_key(|a| std::cmp::Reverse(a.last_modified));
    sorted.truncate(10);

    let items: Vec<ListItem> = sorted
        .iter()
        .map(|p| {
            ListItem::new(Line::from(vec![
                Span::styled(format!("  {} ", p.project_type.icon()), Style::default()),
                Span::styled(format!("{:<20}", p.name), Style::default().fg(Color::White)),
                Span::styled(
                    p.last_activity_display(),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" 🕐 Recent Activity "),
    );
    f.render_widget(list, area);
}

// ── helpers ──

fn format_number(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}
