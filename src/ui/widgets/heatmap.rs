use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Widget},
};

/// ASCII/Unicode activity heatmap widget (similar to GitHub's contribution graph).
pub struct ActivityHeatmap<'a> {
    data: &'a [usize],
    title: &'a str,
    block: Option<Block<'a>>,
}

impl<'a> ActivityHeatmap<'a> {
    pub fn new(data: &'a [usize]) -> Self {
        Self {
            data,
            title: " 📅 Commit Activity (Last 52 Weeks) ",
            block: None,
        }
    }

    pub fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }

    #[allow(dead_code)]
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }
}

impl<'a> Widget for ActivityHeatmap<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = self
            .block
            .unwrap_or_else(|| Block::default().borders(Borders::ALL).title(self.title));

        let inner_area = block.inner(area);
        block.render(area, buf);

        if inner_area.width < 10 || inner_area.height < 4 {
            return;
        }

        let day_labels = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
        let label_width = 4; // "Mon "
        let available_cols = (inner_area.width as usize).saturating_sub(label_width);
        let num_weeks = 52.min(available_cols);

        // Data is 364 days (52 weeks * 7 days).
        // If data is smaller, slice appropriately.
        let total_days = self.data.len();
        let total_commits: usize = self.data.iter().sum();

        // Render each day of week (7 rows)
        for (day_idx, label) in day_labels.iter().enumerate() {
            let row_y = inner_area.y + day_idx as u16;
            if row_y >= inner_area.y + inner_area.height.saturating_sub(1) {
                break;
            }

            // Only show Mon, Wed, Fri labels to reduce visual clutter like GitHub
            let label_span = if day_idx == 0 || day_idx == 2 || day_idx == 4 {
                Span::styled(format!("{label} "), Style::default().fg(Color::DarkGray))
            } else {
                Span::styled("    ", Style::default())
            };

            buf.set_span(inner_area.x, row_y, &label_span, label_width as u16);

            // Render columns (weeks)
            for week_col in 0..num_weeks {
                let cell_x = inner_area.x + label_width as u16 + week_col as u16;
                if cell_x >= inner_area.x + inner_area.width {
                    break;
                }

                // Calculate index in data: (start of window) + week * 7 + day
                // Start from the most recent `num_weeks`
                let offset = (52 - num_weeks) * 7;
                let data_idx = offset + week_col * 7 + day_idx;

                let count = if data_idx < total_days {
                    self.data[data_idx]
                } else {
                    0
                };

                let (ch, style) = match count {
                    0 => ('░', Style::default().fg(Color::Rgb(50, 50, 50))),
                    1..=2 => ('▒', Style::default().fg(Color::Rgb(14, 100, 50))),
                    3..=5 => ('▓', Style::default().fg(Color::Rgb(38, 166, 65))),
                    _ => (
                        '█',
                        Style::default()
                            .fg(Color::Rgb(57, 211, 83))
                            .add_modifier(Modifier::BOLD),
                    ),
                };

                if let Some(cell) = buf.cell_mut((cell_x, row_y)) {
                    cell.set_char(ch).set_style(style);
                }
            }
        }

        // Render legend at the bottom row if space permits
        let legend_y = inner_area.y + inner_area.height.saturating_sub(1);
        if legend_y >= inner_area.y + 7 {
            let legend_line = Line::from(vec![
                Span::styled(
                    format!("Total: {total_commits} commits  "),
                    Style::default().fg(Color::Gray),
                ),
                Span::styled("Less ", Style::default().fg(Color::DarkGray)),
                Span::styled("░", Style::default().fg(Color::Rgb(50, 50, 50))),
                Span::styled(" ", Style::default()),
                Span::styled("▒", Style::default().fg(Color::Rgb(14, 100, 50))),
                Span::styled(" ", Style::default()),
                Span::styled("▓", Style::default().fg(Color::Rgb(38, 166, 65))),
                Span::styled(" ", Style::default()),
                Span::styled("█", Style::default().fg(Color::Rgb(57, 211, 83))),
                Span::styled(" More", Style::default().fg(Color::DarkGray)),
            ]);
            buf.set_line(inner_area.x, legend_y, &legend_line, inner_area.width);
        }
    }
}
