use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

use crate::model::project::ProjectInfo;
use crate::ui::widgets::heatmap::ActivityHeatmap;

pub fn render(f: &mut Frame, project: &ProjectInfo, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header (Name, path, branch, type)
            Constraint::Length(9),  // Code Stats & Git Stats (Side-by-side)
            Constraint::Length(10), // Heatmap
            Constraint::Min(0),     // Recent Commits
        ])
        .split(area);

    render_header(f, project, chunks[0]);
    render_stats_row(f, project, chunks[1]);
    render_heatmap_row(f, project, chunks[2]);
    render_commits_table(f, project, chunks[3]);
}

fn render_header(f: &mut Frame, project: &ProjectInfo, area: Rect) {
    let branch_info = project
        .git_stats
        .as_ref()
        .and_then(|g| g.current_branch.as_deref())
        .map(|b| format!("  🌿 Branch: {b}"))
        .unwrap_or_default();

    let ignored_info = if project.ignored { "  [IGNORED]" } else { "" };

    let title_line = Line::from(vec![
        Span::styled(
            format!(" {} {} ", project.project_type.icon(), project.name),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("({})", project.project_type),
            Style::default().fg(Color::Cyan),
        ),
        Span::styled(&branch_info, Style::default().fg(Color::Green)),
        Span::styled(
            ignored_info,
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  📁 {}", project.path.display()),
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" 🔍 Project Detail — Press [Esc / Backspace] to Return ");
    let paragraph = Paragraph::new(title_line).block(block);
    f.render_widget(paragraph, area);
}

fn render_stats_row(f: &mut Frame, project: &ProjectInfo, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left: Code stats & language breakdown
    let code_lines = if let Some(stats) = &project.code_stats {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("  Total LOC:    ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format_number(stats.code_lines),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("   Files: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{}", stats.file_count),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled("   Comments: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format_number(stats.comment_lines),
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  Languages:",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
        ];

        let lang_spans: Vec<Span> = stats
            .languages
            .iter()
            .take(6)
            .map(|l| {
                Span::styled(
                    format!("  {} ({} LOC) ", l.name, format_number(l.code_lines)),
                    Style::default().fg(Color::White),
                )
            })
            .collect();

        lines.push(Line::from(lang_spans));
        lines
    } else {
        vec![Line::from(Span::styled(
            "  No code statistics available",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let code_block = Block::default()
        .borders(Borders::ALL)
        .title(" 📊 Code Overview ");
    f.render_widget(Paragraph::new(code_lines).block(code_block), cols[0]);

    // Right: Git stats & top contributors
    let git_lines = if let Some(git) = &project.git_stats {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("  Total Commits: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{}", git.total_commits),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("   Last 30 Days: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{}", git.commits_last_30_days),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  Top Contributors:",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )),
        ];

        for (i, c) in git.top_contributors.iter().take(3).enumerate() {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("   {}. ", i + 1),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(format!("{:<20}", c.name), Style::default().fg(Color::White)),
                Span::styled(
                    format!("{} commits", c.commit_count),
                    Style::default().fg(Color::Green),
                ),
            ]));
        }
        lines
    } else {
        vec![Line::from(Span::styled(
            "  Not a git repository",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let git_block = Block::default()
        .borders(Borders::ALL)
        .title(" 🌿 Git Overview ");
    f.render_widget(Paragraph::new(git_lines).block(git_block), cols[1]);
}

fn render_heatmap_row(f: &mut Frame, project: &ProjectInfo, area: Rect) {
    if let Some(git) = &project.git_stats {
        let heatmap = ActivityHeatmap::new(&git.daily_activity)
            .title(" 📅 Project Commit Heatmap (Last 52 Weeks) ");
        f.render_widget(heatmap, area);
    } else {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" 📅 Commit Heatmap ");
        let p = Paragraph::new("  No git repository detected for heatmap").block(block);
        f.render_widget(p, area);
    }
}

fn render_commits_table(f: &mut Frame, project: &ProjectInfo, area: Rect) {
    let Some(git) = &project.git_stats else {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" 📋 Recent Commits ");
        let p = Paragraph::new("  No git history available").block(block);
        f.render_widget(p, area);
        return;
    };

    let header = Row::new(vec![
        Cell::from("Hash"),
        Cell::from("Author"),
        Cell::from("Age"),
        Cell::from("Message"),
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
    .height(1);

    let rows: Vec<Row> = git
        .recent_commits
        .iter()
        .map(|c| {
            Row::new(vec![
                Cell::from(Span::styled(&c.hash, Style::default().fg(Color::Cyan))),
                Cell::from(Span::styled(&c.author, Style::default().fg(Color::White))),
                Cell::from(Span::styled(
                    c.time_ago(),
                    Style::default().fg(Color::DarkGray),
                )),
                Cell::from(Span::styled(&c.message, Style::default().fg(Color::Gray))),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(9),
        Constraint::Length(20),
        Constraint::Length(12),
        Constraint::Min(20),
    ];

    let table = Table::new(rows, widths).header(header).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" 📋 Recent Commits "),
    );

    f.render_widget(table, area);
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
