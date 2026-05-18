use crate::TerminalEvent;
use crossterm::event::KeyEventKind;

pub const CMD_RESIZE: u8 = 0x04;

fn convert_data_to_key_event(data: &[u8]) -> Option<crossterm::event::KeyEvent> {
    let key = match data {
        b"\x1b\x5b\x41" => crossterm::event::KeyCode::Up,
        b"\x1b\x5b\x42" => crossterm::event::KeyCode::Down,
        b"\x1b\x5b\x43" => crossterm::event::KeyCode::Right,
        b"\x1b\x5b\x44" => crossterm::event::KeyCode::Left,
        b"\x03" | b"\x1b" => crossterm::event::KeyCode::Esc,
        b"\x0d" => crossterm::event::KeyCode::Enter,
        b"\x7f" => crossterm::event::KeyCode::Backspace,
        b"\x1b[3~" => crossterm::event::KeyCode::Delete,
        b"\x09" => crossterm::event::KeyCode::Tab,
        x if x.len() == 1 => crossterm::event::KeyCode::Char(data[0] as char),
        _ => return None,
    };
    Some(crossterm::event::KeyEvent::new(
        key,
        crossterm::event::KeyModifiers::empty(),
    ))
}

pub fn convert_data_to_terminal_event(data: &[u8]) -> Option<TerminalEvent> {
    if let Some(&[cols, rows]) = data.strip_prefix(&[CMD_RESIZE]) {
        return Some(TerminalEvent::Resize(cols as u16, rows as u16));
    }
    let key = convert_data_to_key_event(data)?;
    if key.kind != KeyEventKind::Press {
        return None;
    }
    Some(TerminalEvent::Key(key))
}
