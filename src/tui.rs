use crate::{model, safe, store::Store, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    widgets::{Block, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use std::{io, path::Path, sync::mpsc, time::Duration};

struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
    }
}

fn render_history(db: &Store, id: Option<&str>) -> Result<String> {
    let Some(id) = id else {
        return Ok("Press Ctrl+N to start a session.".into());
    };
    let mut lines = Vec::new();
    for (kind, body) in db.preview(id)? {
        lines.push(format!("{kind}  {}", safe(&body)));
    }
    if let Some(t) = db.pending(id)? {
        lines.push(format!("Pending turn {t}"));
    }
    Ok(lines.join("\n\n"))
}

pub fn open(model: Option<String>, port: u16) -> Result<()> {
    let mut db = Store::open()?;
    let mut rows = db.list()?;
    let mut selected = 0usize;
    let mut draft = String::new();
    let mut notice = "Ctrl+N new · Tab switch · Enter send · Esc quit".to_string();
    let mut history = render_history(&db, rows.first().and_then(|r| r["id"].as_str()))?;
    let (send, receive) = mpsc::channel::<std::result::Result<(), String>>();
    let mut busy = false;
    let mut redraw = true;
    enable_raw_mode()?;
    let _restore = Restore;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut term = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    loop {
        if busy {
            if let Ok(result) = receive.try_recv() {
                busy = false;
                notice = match result {
                    Ok(()) => "Reply saved · Enter send · Tab switch · Esc quit".into(),
                    Err(e) => e,
                };
                history = render_history(&db, rows.get(selected).and_then(|r| r["id"].as_str()))?;
                redraw = true;
            }
        }
        if redraw {
            term.draw(|f| {
                let [content, input, status] = Layout::vertical([
                    Constraint::Min(3),
                    Constraint::Length(3),
                    Constraint::Length(1),
                ])
                .areas(f.area());
                let [side, chat] =
                    Layout::horizontal([Constraint::Percentage(27), Constraint::Percentage(73)])
                        .areas(content);
                let items = rows
                    .iter()
                    .map(|r| ListItem::new(safe(r["name"].as_str().unwrap_or(""))));
                let list = List::new(items)
                    .block(Block::bordered().title(" StainlessSteel "))
                    .highlight_symbol("> ")
                    .highlight_style(Style::default().add_modifier(Modifier::BOLD));
                let mut state = ListState::default().with_selected(Some(selected));
                f.render_stateful_widget(list, side, &mut state);
                let transcript = Paragraph::new(history.as_str())
                    .wrap(Wrap { trim: false })
                    .block(Block::bordered().title(" Transcript "));
                let line_count = history
                    .lines()
                    .map(|l| {
                        l.chars()
                            .count()
                            .max(1)
                            .div_ceil(chat.width.saturating_sub(2).max(1) as usize)
                    })
                    .sum::<usize>();
                f.render_widget(
                    transcript.scroll((
                        line_count
                            .saturating_sub(chat.height.saturating_sub(2) as usize)
                            .min(65535) as u16,
                        0,
                    )),
                    chat,
                );
                f.render_widget(
                    Paragraph::new(draft.as_str()).block(Block::bordered().title(" Message ")),
                    input,
                );
                f.render_widget(Paragraph::new(notice.as_str()), status);
            })?;
            redraw = false;
        }
        if busy && !event::poll(Duration::from_millis(120))? {
            continue;
        }
        let ev = event::read()?;
        let Event::Key(k) = ev else {
            redraw = matches!(ev, Event::Resize(..));
            continue;
        };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        if k.code == KeyCode::Esc
            || (k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL))
        {
            break;
        }
        if busy {
            continue;
        }
        match k.code {
            KeyCode::Char('n') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                db.new_session(&format!("Session {}", rows.len() + 1), Path::new("."))?;
                rows = db.list()?;
                selected = 0;
                draft.clear();
            }
            KeyCode::Tab if !rows.is_empty() => {
                selected = (selected + 1) % rows.len();
                draft.clear();
            }
            KeyCode::Enter if !draft.trim().is_empty() => {
                let Some(model) = model.clone() else {
                    notice = "Set STAINLESS_MODEL or start with --model NAME to send.".into();
                    redraw = true;
                    continue;
                };
                if rows.is_empty() {
                    db.new_session("Session 1", Path::new("."))?;
                    rows = db.list()?;
                }
                let id = rows[selected]["id"].as_str().unwrap().to_owned();
                let prompt = std::mem::take(&mut draft);
                let send = send.clone();
                busy = true;
                notice = "Waiting for the local model…".into();
                std::thread::spawn(move || {
                    let result = (|| -> Result<()> {
                        model::run(&mut Store::open()?, &id, &model, port, &prompt, |_| Ok(()))?;
                        Ok(())
                    })();
                    let _ = send.send(result.map_err(|e| e.to_string()));
                });
            }
            KeyCode::Char(c)
                if !c.is_control() && draft.len() + c.len_utf8() <= model::MAX_INPUT =>
            {
                draft.push(c);
                redraw = true;
                continue;
            }
            KeyCode::Backspace => {
                draft.pop();
                redraw = true;
                continue;
            }
            _ => continue,
        }
        history = render_history(&db, rows.get(selected).and_then(|r| r["id"].as_str()))?;
        redraw = true;
    }
    Ok(())
}
