pub mod model;
pub mod terminal;
pub mod view;

use crate::{
    domain::{NetworkEvent, UserAction},
    net::mesh::MeshHandle,
    tui::{
        model::{InputEvent, Model},
        terminal::TerminalGuard,
        view::render_app,
    },
};
use anyhow::{Context, Result};
use crossterm::event::{Event, EventStream, KeyEventKind, MouseEventKind};
use futures_util::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::stdout;
use tokio::sync::broadcast;

pub async fn run(mut model: Model, mesh: MeshHandle) -> Result<()> {
    let mut guard = TerminalGuard::enter().context("enter terminal UI")?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend).context("create terminal")?;
    terminal.clear().context("clear terminal")?;
    let mut inputs = EventStream::new();
    let mut network = mesh.subscribe();

    loop {
        terminal
            .draw(|frame| render_app(frame, &model))
            .context("render terminal UI")?;
        let event = tokio::select! {
            input = inputs.next() => match input {
                Some(Ok(Event::Key(key))) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                    Some(InputEvent::Key(key))
                }
                Some(Ok(Event::Mouse(mouse))) => match mouse.kind {
                    MouseEventKind::ScrollUp => Some(InputEvent::Scroll(3)),
                    MouseEventKind::ScrollDown => Some(InputEvent::Scroll(-3)),
                    _ => None,
                },
                Some(Ok(_)) => None,
                Some(Err(error)) => return Err(error).context("read terminal input"),
                None => return Ok(()),
            },
            received = network.recv() => match received {
                Ok(event) => Some(InputEvent::Network(event)),
                Err(broadcast::error::RecvError::Lagged(count)) => Some(InputEvent::Network(
                    NetworkEvent::Status(format!("{count} UI events skipped"))
                )),
                Err(broadcast::error::RecvError::Closed) => return Ok(()),
            },
            _ = tokio::signal::ctrl_c() => {
                model.should_quit = true;
                Some(InputEvent::Network(NetworkEvent::Status("shutting down".into())))
            }
        };
        if let Some(event) = event {
            for action in model.update(event) {
                let quitting = action == UserAction::Quit;
                mesh.send(action).await?;
                if quitting {
                    model.should_quit = true;
                }
            }
        }
        if model.should_quit {
            break;
        }
    }
    mesh.shutdown();
    guard.restore().context("restore terminal")?;
    Ok(())
}
