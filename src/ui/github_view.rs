use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table},
};

use crate::app::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let github = &app.github_data;

    // 1. If username is not configured
    if app.config.github.username.trim().is_empty() {
        render_unconfigured(f, area);
        return;
    }

    // 2. If an error occurred (e.g. rate limit or network down)
    if let Some(err) = &github.error_message {
        render_error(f, err, area);
        return;
    }

    // 3. Normal view: split top (profile + repos) and bottom (recent activity feed)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Profile card & repos
            Constraint::Min(0),     // Activity feed
        ])
        .split(area);

    let top_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(chunks[0]);

    render_profile_card(f, app, top_cols[0]);
    render_top_repos(f, app, top_cols[1]);
    render_activity_feed(f, app, chunks[1]);
}

fn render_unconfigured(f: &mut Frame, area: Rect) {
    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  🐙 GitHub Integration Not Configured",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Connect your GitHub account to view public events, commits, and repository stats.",
            Style::default().fg(Color::Gray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  How to configure:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  1. In ", Style::default().fg(Color::Gray)),
            Span::styled("config.toml", Style::default().fg(Color::White)),
            Span::styled(":", Style::default().fg(Color::Gray)),
        ]),
        Line::from(Span::styled(
            "     [github]",
            Style::default().fg(Color::Green),
        )),
        Line::from(Span::styled(
            "     username = \"your-username\"",
            Style::default().fg(Color::Green),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  2. (Optional) Set personal access token in env: ",
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                "export GITHUB_TOKEN=ghp_...",
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Press [r] to refresh after updating configuration.",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" 🐙 GitHub Activity ");
    f.render_widget(Paragraph::new(text).block(block), area);
}

fn render_error(f: &mut Frame, err: &str, area: Rect) {
    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  ⚠️ Failed to Load GitHub Data",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("  Error: {err}"),
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Tip: Check your internet connection or verify your GITHUB_TOKEN rate limits.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "  Press [r] to retry connecting to GitHub.",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" 🐙 GitHub Activity (Error) ");
    f.render_widget(Paragraph::new(text).block(block), area);
}

fn render_profile_card(f: &mut Frame, app: &App, area: Rect) {
    let github = &app.github_data;
    let lines = if let Some(user) = &github.user {
        let display_name = user.name.as_deref().unwrap_or(&user.login);
        vec![
            Line::from(vec![
                Span::styled(
                    format!("  {} ", display_name),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("(@{})", user.login),
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
            Line::from(vec![Span::styled(
                format!("  {}", user.bio.as_deref().unwrap_or("No bio provided")),
                Style::default().fg(Color::Gray),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  📦 Repos: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:<6}", user.public_repos),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("👥 Followers: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}", user.followers),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                format!("  🌐 {}", user.html_url),
                Style::default().fg(Color::Blue),
            )]),
        ]
    } else {
        vec![Line::from("  Loading user profile...")]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" 👤 User Profile ");
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_top_repos(f: &mut Frame, app: &App, area: Rect) {
    let github = &app.github_data;

    let items: Vec<ListItem> = github
        .repos
        .iter()
        .take(5)
        .map(|r| {
            let lang = r.language.as_deref().unwrap_or("Text");
            let desc = r.description.as_deref().unwrap_or("");
            let desc_truncated = if desc.len() > 30 {
                format!("{}…", &desc[..29])
            } else {
                desc.to_string()
            };

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("  {:<18}", r.name),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{:<10}", lang), Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("⭐ {:<4}", r.stars),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    format!("🍴 {:<4}", r.forks),
                    Style::default().fg(Color::Green),
                ),
                Span::styled(desc_truncated, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" 📂 Recent Repositories "),
    );
    f.render_widget(list, area);
}

fn render_activity_feed(f: &mut Frame, app: &App, area: Rect) {
    let github = &app.github_data;

    if github.events.is_empty() {
        let p = Paragraph::new("  No recent public events recorded in the last 30 days.").block(
            Block::default()
                .borders(Borders::ALL)
                .title(" ⚡ Recent GitHub Events "),
        );
        f.render_widget(p, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from("Type"),
        Cell::from("Repository"),
        Cell::from("Time"),
        Cell::from("Summary"),
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
    .height(1);

    let rows: Vec<Row> = github
        .events
        .iter()
        .map(|ev| {
            Row::new(vec![
                Cell::from(Span::styled(ev.icon(), Style::default().fg(Color::Cyan))),
                Cell::from(Span::styled(
                    &ev.repo_name,
                    Style::default().fg(Color::White),
                )),
                Cell::from(Span::styled(
                    ev.time_ago(),
                    Style::default().fg(Color::DarkGray),
                )),
                Cell::from(Span::styled(&ev.summary, Style::default().fg(Color::Gray))),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(12),
        Constraint::Length(24),
        Constraint::Length(12),
        Constraint::Min(24),
    ];

    let table = Table::new(rows, widths).header(header).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" ⚡ Public Activity Feed (Past 30 Days) ")
            .title_bottom(" [r] Refresh Data "),
    );

    f.render_widget(table, area);
}
