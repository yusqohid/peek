use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub fn render(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  👀 peek",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  v0.3.0 — Friendly Developer Project Analyzer",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Navigation",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        key_line("1", "Dashboard tab"),
        key_line("2", "Projects tab"),
        key_line("3", "GitHub Activity tab"),
        key_line("4 / ?", "Help tab"),
        key_line("Tab / Shift+Tab", "Next / Previous tab"),
        Line::from(""),
        Line::from(Span::styled(
            "  Project List & Detail",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        key_line("Enter", "Open selected project detail view"),
        key_line("Esc / Backspace", "Return to project list from detail"),
        key_line("j / ↓", "Move down"),
        key_line("k / ↑", "Move up"),
        key_line("g / Home", "Go to top"),
        key_line("G / End", "Go to bottom"),
        key_line("s", "Cycle sort order (Name → LOC → Commits → Recent)"),
        key_line("i", "Toggle ignored projects visibility"),
        Line::from(""),
        Line::from(Span::styled(
            "  General",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        key_line("r", "Rescan projects and refresh GitHub data"),
        key_line("q / Ctrl+C", "Quit"),
        Line::from(""),
        Line::from(Span::styled(
            "  Configuration",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Copy config.example.toml → config.toml and edit to customize.",
            Style::default().fg(Color::Gray),
        )),
        Line::from(Span::styled(
            "  Set `[github] username` and `GITHUB_TOKEN` to enable GitHub activity.",
            Style::default().fg(Color::Gray),
        )),
        Line::from(Span::styled(
            "  Add project names to `ignored_projects` to hide experiments.",
            Style::default().fg(Color::Gray),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" ❓ Help & Keybindings ");
    let paragraph = Paragraph::new(help_text).block(block);
    f.render_widget(paragraph, area);
}

fn key_line(key: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("  {:<20}", key),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(desc.to_string(), Style::default().fg(Color::Gray)),
    ])
}
