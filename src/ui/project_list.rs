use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
};

use crate::app::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let visible = app.visible_projects();

    let header = Row::new(vec![
        Cell::from(" #"),
        Cell::from("Project"),
        Cell::from("Type"),
        Cell::from("LOC"),
        Cell::from("Commits"),
        Cell::from("Files"),
        Cell::from("Primary Lang"),
        Cell::from("Last Activity"),
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
    .height(1);

    let rows: Vec<Row> = visible
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let loc = p
                .code_stats
                .as_ref()
                .map_or("-".to_string(), |s| format_number(s.code_lines));
            let commits = p
                .git_stats
                .as_ref()
                .map_or("-".to_string(), |g| format_number(g.total_commits));
            let files = p
                .code_stats
                .as_ref()
                .map_or("-".to_string(), |s| s.file_count.to_string());
            let primary = p
                .code_stats
                .as_ref()
                .map_or("-".to_string(), |s| s.primary_language().to_string());

            let style = if p.ignored {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };

            let name_display = if p.ignored {
                format!("{} (ignored)", p.name)
            } else {
                p.name.clone()
            };

            Row::new(vec![
                Cell::from(format!(" {}", i + 1)),
                Cell::from(name_display),
                Cell::from(p.project_type.icon()),
                Cell::from(loc),
                Cell::from(commits),
                Cell::from(files),
                Cell::from(primary),
                Cell::from(p.last_activity_display()),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        ratatui::layout::Constraint::Length(4),
        ratatui::layout::Constraint::Min(16),
        ratatui::layout::Constraint::Length(6),
        ratatui::layout::Constraint::Length(9),
        ratatui::layout::Constraint::Length(9),
        ratatui::layout::Constraint::Length(7),
        ratatui::layout::Constraint::Length(14),
        ratatui::layout::Constraint::Length(14),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" 📁 Projects ")
                .title_bottom(
                    " [Enter] Detail  [s] Sort  [i] Ignored  [j/k] Navigate  [r] Refresh ",
                ),
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut state = TableState::default();
    state.select(Some(app.selected_project));

    f.render_stateful_widget(table, area, &mut state);
}

fn format_number(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
