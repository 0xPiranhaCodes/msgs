use crate::{
    domain::ChatEntry,
    tui::model::{Mode, Model, Overlay, Pane},
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

const ACCENT: Color = Color::Cyan;
const MUTED: Color = Color::DarkGray;

pub fn render_app(frame: &mut Frame, model: &Model) {
    let area = frame.area();
    if area.width < 40 || area.height < 8 {
        frame.render_widget(
            Paragraph::new("Terminal too small — resize to at least 40×8")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Yellow)),
            area,
        );
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);
    render_header(frame, rows[0], model);

    let show_peers = model.peers_visible && area.width >= 72;
    let conversation_area = if show_peers {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(24), Constraint::Min(30)])
            .split(rows[1]);
        render_peers(frame, columns[0], model);
        columns[1]
    } else {
        rows[1]
    };
    render_conversation(frame, conversation_area, model);
    render_composer(frame, rows[2], model);
    render_status(frame, rows[3], model);
    if model.overlay == Some(Overlay::Help) {
        render_help(frame, area);
    }
}

fn render_header(frame: &mut Frame, area: Rect, model: &Model) {
    let title = Line::from(vec![
        Span::styled(
            " msgs ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled("encrypted network mesh", Style::default().fg(MUTED)),
        Span::raw("  "),
        Span::styled(
            format!("● {} online", model.peers.len() + 1),
            Style::default().fg(Color::Green),
        ),
    ]);
    frame.render_widget(Paragraph::new(title), area);
}

fn render_peers(frame: &mut Frame, area: Rect, model: &Model) {
    let active = model.pane == Pane::Peers;
    let border = if active { ACCENT } else { MUTED };
    let mut peers = vec![ListItem::new(Line::from(vec![
        Span::styled("● ", Style::default().fg(Color::Green)),
        Span::styled(
            format!("{} (you)", model.local_identity.name),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ]))];
    peers.extend(model.peers.values().map(|identity| {
        ListItem::new(Line::from(vec![
            Span::styled("● ", Style::default().fg(Color::Green)),
            Span::raw(identity.name.clone()),
        ]))
    }));
    frame.render_widget(
        List::new(peers).block(
            Block::default()
                .title(" PEERS ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border)),
        ),
        area,
    );
}

fn render_conversation(frame: &mut Frame, area: Rect, model: &Model) {
    let mut lines = Vec::new();
    let mut previous_sender = None;
    for entry in &model.entries {
        match entry {
            ChatEntry::Message {
                id,
                identity,
                text,
                timestamp,
            } => {
                let same_sender = previous_sender == Some(*id);
                if !same_sender {
                    if !lines.is_empty() {
                        lines.push(Line::default());
                    }
                    lines.push(Line::from(vec![
                        Span::styled(identity.name.clone(), sender_style(id.0[0])),
                        Span::styled(
                            format!(" <{}>  {}", identity.email, clock(*timestamp)),
                            Style::default().fg(MUTED),
                        ),
                    ]));
                } else {
                    lines.push(Line::from(Span::styled(
                        clock(*timestamp),
                        Style::default().fg(MUTED),
                    )));
                }
                lines.push(Line::from(text.clone()));
                previous_sender = Some(*id);
            }
            ChatEntry::Presence {
                identity,
                joined,
                timestamp,
            } => {
                lines.push(
                    Line::from(format!(
                        "── {} {} · {} ──",
                        identity.name,
                        if *joined { "joined" } else { "left" },
                        clock(*timestamp)
                    ))
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(MUTED)),
                );
                previous_sender = None;
            }
            ChatEntry::Status { text, timestamp } => {
                lines.push(
                    Line::from(format!("{} · {}", text, clock(*timestamp)))
                        .style(Style::default().fg(MUTED)),
                );
                previous_sender = None;
            }
        }
    }
    if lines.is_empty() {
        lines.push(
            Line::from("No messages yet. Say hello.")
                .alignment(Alignment::Center)
                .style(Style::default().fg(MUTED)),
        );
    }
    let inner_height = area.height.saturating_sub(2) as usize;
    let maximum_scroll = lines.len().saturating_sub(inner_height);
    let scroll = maximum_scroll.saturating_sub(model.scroll.min(maximum_scroll));
    let border = if model.pane == Pane::Conversation {
        ACCENT
    } else {
        MUTED
    };
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" ROOM ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border)),
            )
            .wrap(Wrap { trim: false })
            .scroll((scroll as u16, 0)),
        area,
    );
}

fn render_composer(frame: &mut Frame, area: Rect, model: &Model) {
    let title = if model.mode == Mode::Compose {
        " COMPOSE "
    } else {
        " MESSAGE "
    };
    let style = if model.mode == Mode::Compose {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(MUTED)
    };
    let text = if model.composer.is_empty() {
        Line::from(Span::styled("Type a message…", Style::default().fg(MUTED)))
    } else {
        Line::from(model.composer.clone())
    };
    frame.render_widget(
        Paragraph::new(text).block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(style),
        ),
        area,
    );
}

fn render_status(frame: &mut Frame, area: Rect, model: &Model) {
    let mode = if model.mode == Mode::Compose {
        " COMPOSE "
    } else {
        " NAV "
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                mode,
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(model.status.clone(), Style::default().fg(MUTED)),
            Span::styled("   ? help · p peers", Style::default().fg(MUTED)),
        ])),
        area,
    );
}

fn render_help(frame: &mut Frame, area: Rect) {
    let popup = centered(area, 54, 16);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from("Keyboard"),
            Line::from(""),
            Line::from("Enter  send                 Esc  navigation"),
            Line::from("i      compose              j/k  scroll"),
            Line::from("PgUp/PgDn  page             Tab  switch pane"),
            Line::from("p      toggle peers          ?   close help"),
            Line::from("q      quit in NAV       Ctrl+C  quit"),
            Line::from(""),
            Line::from("/who  list peers        /quit  exit"),
        ])
        .block(
            Block::default()
                .title(" HELP ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        ),
        popup,
    );
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width.min(area.width)) / 2,
        y: area.y + area.height.saturating_sub(height.min(area.height)) / 2,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

fn sender_style(seed: u8) -> Style {
    let color = match seed % 5 {
        0 => Color::Magenta,
        1 => Color::Cyan,
        2 => Color::Yellow,
        3 => Color::Green,
        _ => Color::Blue,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn clock(timestamp: u64) -> String {
    let seconds = timestamp % 86_400;
    format!("{:02}:{:02}", seconds / 3_600, (seconds % 3_600) / 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{Identity, InstanceId, NetworkEvent},
        tui::model::{InputEvent, Model},
    };
    use ratatui::{backend::TestBackend, style::Color, Terminal};

    fn render(width: u16, height: u16, model: &Model) -> ratatui::buffer::Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render_app(frame, model)).unwrap();
        terminal.backend().buffer().clone()
    }

    #[test]
    fn wide_layout_shows_peers_messages_status_and_composer() {
        let mut model = Model::new(Identity {
            name: "Ada".into(),
            email: "ada@example.com".into(),
        });
        model.update(InputEvent::Network(NetworkEvent::PeerJoined {
            id: InstanceId([2; 16]),
            identity: Identity {
                name: "Grace".into(),
                email: "grace@example.com".into(),
            },
        }));
        let buffer = render(100, 30, &model);
        let content = buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(content.contains("PEERS"));
        assert!(content.contains("Grace"));
        assert!(content.contains("COMPOSE"));
        assert!(content.contains("Type a message"));
        assert!(buffer.content.iter().all(|cell| cell.bg == Color::Reset));
    }

    #[test]
    fn narrow_layout_collapses_peers_and_tiny_layout_warns() {
        let model = Model::new(Identity {
            name: "Ada".into(),
            email: "ada@example.com".into(),
        });
        let narrow = render(60, 20, &model);
        let narrow_text = narrow
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(!narrow_text.contains("PEERS"));
        let tiny = render(30, 6, &model);
        let tiny_text = tiny
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(tiny_text.contains("Terminal too small"));
    }
}
