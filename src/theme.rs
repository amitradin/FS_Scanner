use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType},
};

pub const ACCENT: Color = Color::Cyan;
pub const OK: Color = Color::Green;
pub const ERR: Color = Color::Red;
pub const MUTED: Color = Color::DarkGray;

/// Standard rounded panel with a padded, bold title
pub fn panel(title: &str) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(MUTED))
        .title(Line::from(format!(" {title} ")).bold())
}

/// Panel with a colored border (focus / valid / invalid)
pub fn focused_panel(title: &str, color: Color) -> Block<'_> {
    panel(title).border_style(Style::new().fg(color))
}

/// Borderd box for the footer, without a title
pub fn footer_panel() -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(MUTED))
}

/// Selected-row style for all lists
pub fn highlight() -> Style {
    Style::new().fg(Color::Black).bg(ACCENT).bold()
}

/// Colored title row shown at the top of every screen
pub fn title_bar(text: &str) -> Line<'static> {
    Line::from(
        Span::from(format!("  {text}  "))
            .fg(Color::Black)
            .bg(ACCENT)
            .bold(),
    )
    .centered()
}

/// Renders [("q","quit"),("j","down")] as colored key chips
pub fn key_hints(pairs: &[(&str, &str)]) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, (key, label)) in pairs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        spans.push(
            Span::from(format!(" {key} "))
                .fg(Color::Black)
                .bg(ACCENT)
                .bold(),
        );
        spans.push(Span::from(format!(" {label}")).fg(MUTED));
    }
    Line::from(spans).centered()
}
