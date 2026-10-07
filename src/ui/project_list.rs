use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
};

use crate::app::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    if let Some(err) = &app.scan_error {
        render_message(
            f,
            area,
            " 📁 Projects ",
            vec![
                Line::from(Span::styled(
                    "Scan failed",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(err.as_str()),
                Line::from(""),
                Line::from("Check general.scan_directory in config, then press [r] to retry."),
            ],
        );
        return;
    }

    let visible = app.visible_projects();
    if visible.is_empty() {
        let scan_dir = app.config.resolved_scan_directory();
        render_message(
            f,
            area,
            " 📁 Projects ",
            vec![
                Line::from(Span::styled(
                    "No projects found",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(format!("Scanned: {}", scan_dir.display())),
                Line::from(
                    "No known project markers (Cargo.toml, package.json, go.mod, …) were found.",
                ),
                Line::from(""),
                Line::from(
                    "Try a different directory or depth, then press [r] to rescan. Toggle ignored with [i].",
                ),
            ],
        );
        return;
    }

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

fn render_message(f: &mut Frame, area: Rect, title: &str, lines: Vec<Line<'_>>) {
    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: true });
    f.render_widget(paragraph, area);
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
