pub mod dashboard;
pub mod github_view;
pub mod help;
pub mod project_detail;
pub mod project_list;
pub mod widgets;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Tabs},
};

use crate::app::{ActiveTab, App};

/// Render the entire UI for a single frame.
pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Tab bar
            Constraint::Min(0),    // Main content
            Constraint::Length(1), // Status bar
        ])
        .split(f.area());

    // ── Tab bar ──
    let tab_titles: Vec<Line> = ActiveTab::ALL
        .iter()
        .map(|t| Line::from(t.label()))
        .collect();

    let tabs = Tabs::new(tab_titles)
        .block(Block::default().borders(Borders::ALL).title(" 👀 peek "))
        .select(app.active_tab.index())
        .style(Style::default().fg(Color::Gray))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .divider("│");

    f.render_widget(tabs, chunks[0]);

    // ── Main content ──
    match app.active_tab {
        ActiveTab::Dashboard => dashboard::render(f, app, chunks[1]),
        ActiveTab::Projects => {
            if let Some(idx) = app.detail_project {
                let visible = app.visible_projects();
                if let Some(project) = visible.get(idx) {
                    project_detail::render(f, project, chunks[1]);
                } else {
                    project_list::render(f, app, chunks[1]);
                }
            } else {
                project_list::render(f, app, chunks[1]);
            }
        }
        ActiveTab::GitHub => github_view::render(f, app, chunks[1]),
        ActiveTab::Help => help::render(f, chunks[1]),
    }

    // ── Status bar ──
    let status = Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(&app.status_message, Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("  Sort: {}  ", app.sort_order.label()),
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    f.render_widget(ratatui::widgets::Paragraph::new(status), chunks[2]);
}
