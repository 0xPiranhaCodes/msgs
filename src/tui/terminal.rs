use crossterm::{
    cursor::{Hide, Show},
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, stdout};

pub struct TerminalGuard {
    active: bool,
}

impl TerminalGuard {
    pub fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        if let Err(error) = execute!(stdout(), EnterAlternateScreen, EnableMouseCapture, Hide) {
            let _ = disable_raw_mode();
            return Err(error);
        }
        Ok(Self { active: true })
    }

    pub fn restore(&mut self) -> io::Result<()> {
        restore_once(&mut self.active, force_restore)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = force_restore();
        previous(info);
    }));
}

fn force_restore() -> io::Result<()> {
    let screen = execute!(stdout(), Show, DisableMouseCapture, LeaveAlternateScreen);
    let raw = disable_raw_mode();
    screen.and(raw)
}

fn restore_once<F>(active: &mut bool, restore: F) -> io::Result<()>
where
    F: FnOnce() -> io::Result<()>,
{
    if !*active {
        return Ok(());
    }
    *active = false;
    restore()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restoration_runs_once() {
        let mut active = true;
        let mut calls = 0;
        restore_once(&mut active, || {
            calls += 1;
            Ok(())
        })
        .unwrap();
        restore_once(&mut active, || {
            calls += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(calls, 1);
        assert!(!active);
    }

    #[test]
    fn restoration_marks_inactive_even_on_error() {
        let mut active = true;
        assert!(restore_once(&mut active, || {
            Err(std::io::Error::other("restore failed"))
        })
        .is_err());
        assert!(!active);
    }
}
