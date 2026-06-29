//! Terminal setup and teardown.

use std::io::{self, Stdout};

use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{Terminal, backend::CrosstermBackend};

/// Concrete terminal type used throughout the app.
pub type Term = Terminal<CrosstermBackend<Stdout>>;

/// Owns the terminal handle; restores raw mode and alternate screen on drop.
///
/// A panic hook is installed at construction time so the terminal is also
/// restored if the thread panics.
pub struct TerminalHandle {
    pub terminal: Term,
}

impl TerminalHandle {
    /// Enter raw mode and the alternate screen, then install a panic hook
    /// that tears down the terminal before printing the panic message.
    pub fn new() -> Result<Self> {
        // Install the hook before enabling raw mode so any failure during
        // setup is also cleaned up properly.
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
            original_hook(info);
        }));

        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let terminal = Terminal::new(CrosstermBackend::new(stdout))?;

        Ok(Self { terminal })
    }
}

impl Drop for TerminalHandle {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
    }
}
