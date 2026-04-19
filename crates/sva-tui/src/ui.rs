use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::ExplorerState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaveDisplay {
    NoSignal,
    Missing {
        signal: String,
        time: i64,
    },
    Error {
        signal: String,
        time: i64,
        message: String,
    },
    Value {
        signal: String,
        time: i64,
        raw_bits: String,
        pretty_hex: Option<String>,
    },
}

pub fn draw(
    frame: &mut Frame<'_>,
    state: &mut ExplorerState,
    full_signal: bool,
    wave: Option<&WaveDisplay>,
) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(area);

    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    draw_tree(frame, panes[0], state, full_signal);
    draw_right(frame, panes[1], state, full_signal, wave);
    draw_footer(frame, chunks[1], state, full_signal);
}

fn draw_tree(frame: &mut Frame<'_>, area: Rect, state: &mut ExplorerState, full_signal: bool) {
    let rows = state.visible_rows();
    let selected = state.selected_index();
    let height = area.height.saturating_sub(2).max(1) as usize;
    let top = selected.saturating_sub(height.saturating_sub(1));

    let items = rows
        .iter()
        .enumerate()
        .skip(top)
        .take(height)
        .map(|(index, row)| {
            let style = if index == selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            ListItem::new(row.render(state.index(), full_signal)).style(style)
        })
        .collect::<Vec<_>>();

    let list = List::new(items).block(Block::default().title("Current tree").borders(Borders::ALL));
    frame.render_widget(list, area);
}

fn draw_right(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut ExplorerState,
    _full_signal: bool,
    wave: Option<&WaveDisplay>,
) {
    let right_chunks = if wave.is_some() {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(7)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1)])
            .split(area)
    };

    draw_code(frame, right_chunks[0], state);
    if let Some(wave) = wave {
        draw_wave(frame, right_chunks[1], wave);
    }
}

fn draw_code(frame: &mut Frame<'_>, area: Rect, state: &mut ExplorerState) {
    let selected_row = state.selected_row();
    let visible_lines = area.height.saturating_sub(2).max(1) as usize;
    state.clamp_code_scroll(visible_lines);

    let snippet_lines = state.index().code_snippet_lines(&selected_row.entry);
    let highlight_indices = state
        .index()
        .code_snippet_highlight_indices(&selected_row.entry);
    let code_scroll = state.code_scroll();
    let code_lines = snippet_lines
        .iter()
        .enumerate()
        .skip(code_scroll)
        .take(visible_lines)
        .map(|(index, line)| {
            if highlight_indices.contains(&index) {
                Line::from(Span::styled(
                    line.clone(),
                    Style::default().fg(Color::Green),
                ))
            } else {
                Line::from(line.clone())
            }
        })
        .collect::<Vec<_>>();

    let end = (code_scroll + visible_lines).min(snippet_lines.len());
    let title = format!(
        "Code context lines {}-{}/{}",
        code_scroll + 1,
        end.max(code_scroll + 1),
        snippet_lines.len()
    );
    let paragraph = Paragraph::new(code_lines)
        .block(Block::default().title(title).borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn draw_wave(frame: &mut Frame<'_>, area: Rect, wave: &WaveDisplay) {
    let lines = match wave {
        WaveDisplay::NoSignal => vec![Line::from("No selected signal for this row.")],
        WaveDisplay::Missing { signal, time } => vec![
            Line::from(format!("signal: {signal}")),
            Line::from(format!("time: {time}")),
            Line::from("value: <not found in VCD>"),
        ],
        WaveDisplay::Error {
            signal,
            time,
            message,
        } => vec![
            Line::from(format!("signal: {signal}")),
            Line::from(format!("time: {time}")),
            Line::from(format!("error: {message}")),
        ],
        WaveDisplay::Value {
            signal,
            time,
            raw_bits,
            pretty_hex,
        } => vec![
            Line::from(format!("signal: {signal}")),
            Line::from(format!("time: {time}")),
            Line::from(format!("raw bits: {raw_bits}")),
            Line::from(format!(
                "hex: {}",
                pretty_hex.as_deref().unwrap_or("<not binary>")
            )),
        ],
    };
    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title("Selected signal value")
            .borders(Borders::ALL),
    );
    frame.render_widget(paragraph, area);
}

fn draw_footer(frame: &mut Frame<'_>, area: Rect, state: &mut ExplorerState, full_signal: bool) {
    let selected_row = state.selected_row();
    let status = if selected_row.already_shown {
        "already shown"
    } else if state.can_expand(&selected_row) {
        "expandable"
    } else if state.index().has_children(&selected_row.entry) {
        "expanded"
    } else {
        "leaf"
    };
    let selected = selected_row.render(state.index(), full_signal);
    let text = vec![
        Line::from(format!("[{status}] {selected}")),
        Line::from("q quit | r reset | Enter/Right expand | Left/Backspace collapse or parent | PgUp/PgDn code"),
    ];
    let paragraph = Paragraph::new(text)
        .style(Style::default().add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::TOP));
    frame.render_widget(paragraph, area);
}
