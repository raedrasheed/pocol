//! The passphrase prompt of the PoCol command-line tools (`pocol`, `pocol-key`, `pocol-sign`): one line from
//! stdin, never a command-line value. Node-local: no consensus rule reads it.
use super::backup::{read_passphrase, PassphraseInput};
use std::sync::atomic::{AtomicBool, Ordering};
use zeroize::Zeroizing;

/// One passphrase line from stdin without its line ending ([`read_passphrase`]). When stdin is a terminal the
/// prompt goes to stderr and echo is off while the line is typed; a terminal whose echo cannot be switched off is
/// refused. `cancelled` (the caller's Ctrl-C flag) cancels the prompt. The Windows wallet writes the line through a
/// pipe instead.
pub fn passphrase(prompt: &str, cancelled: &dyn Fn() -> bool) -> Result<Zeroizing<Vec<u8>>, String> {
    let echo = Echo::off();
    let from = match (echo.terminal, echo.saved.is_some()) {
        (false, _) => PassphraseInput::Pipe,
        (true, true) => PassphraseInput::Hidden,
        (true, false) => PassphraseInput::Visible,
    };
    if from == PassphraseInput::Hidden {
        eprint!("{prompt}: ");
        // The SIGINT handler can run on any thread, so the blocked read would not see Ctrl-C: wait for the typed
        // line with poll, checking the stop flag. A terminal in canonical mode is readable once Enter is pressed.
        #[cfg(unix)]
        while !cancelled() {
            let mut fd = libc::pollfd { fd: 0, events: libc::POLLIN, revents: 0 };
            if unsafe { libc::poll(&mut fd, 1, 200) } != 0 {
                break;
            }
        }
    }
    let read = read_passphrase(&mut std::io::stdin().lock(), from, cancelled);
    if from == PassphraseInput::Hidden {
        eprintln!();
    }
    drop(echo);
    read
}

/// [`passphrase`] for a tool that has no Ctrl-C handler of its own (`pocol-key`, `pocol-sign`): while a terminal
/// prompt is open, SIGINT and SIGTERM (Ctrl-C, close and shutdown on Windows) cancel it instead of ending the process
/// with terminal echo still off. The previous handlers are restored afterwards, so the rest of the tool stops as
/// before. With stdin not a terminal (a pipe) echo is never touched and no handler is installed, so a signal ends a
/// tool that waits on its pipe at once, as before (a caught signal would only restart the blocked pipe read).
pub fn ask(prompt: &str) -> Result<Zeroizing<Vec<u8>>, String> {
    use std::io::IsTerminal;
    let _interrupts = std::io::stdin().is_terminal().then(Interrupts::catch);
    passphrase(prompt, &|| INTERRUPTED.load(Ordering::Relaxed))
}

static INTERRUPTED: AtomicBool = AtomicBool::new(false);
#[cfg(unix)]
extern "C" fn interrupt(_: i32) {
    INTERRUPTED.store(true, Ordering::Relaxed);
}
#[cfg(windows)]
unsafe extern "system" fn console(event: u32) -> i32 {
    if event == 0 || event == 1 || event == 2 {
        INTERRUPTED.store(true, Ordering::Relaxed);
        1
    } else {
        0
    }
}

/// [`ask`]'s handlers, installed until drop.
struct Interrupts {
    #[cfg(unix)]
    previous: [libc::sighandler_t; 2],
}
impl Interrupts {
    #[cfg(unix)]
    fn catch() -> Self {
        unsafe {
            let handler = interrupt as *const () as libc::sighandler_t;
            Self { previous: [libc::signal(libc::SIGINT, handler), libc::signal(libc::SIGTERM, handler)] }
        }
    }
    #[cfg(windows)]
    fn catch() -> Self {
        unsafe { windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(console), 1) };
        Self {}
    }
    #[cfg(not(any(unix, windows)))]
    fn catch() -> Self {
        Self {}
    }
}
impl Drop for Interrupts {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::signal(libc::SIGINT, self.previous[0]);
            libc::signal(libc::SIGTERM, self.previous[1]);
        }
        #[cfg(windows)]
        unsafe {
            windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(console), 0);
        }
    }
}

/// Terminal echo switched off until drop (restored even on an error path). On Unix, Ctrl-C cancels the prompt at
/// once; on Windows the console read restarts after Ctrl-C, and the line typed after it is discarded as a cancel.
struct Echo {
    terminal: bool,
    #[cfg(unix)]
    saved: Option<libc::termios>,
    #[cfg(windows)]
    saved: Option<(windows_sys::Win32::Foundation::HANDLE, windows_sys::Win32::System::Console::CONSOLE_MODE)>,
    #[cfg(not(any(unix, windows)))]
    saved: Option<()>,
}
impl Echo {
    #[cfg(unix)]
    fn off() -> Self {
        unsafe {
            if libc::isatty(0) != 1 {
                return Self { terminal: false, saved: None };
            }
            let mut mode: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(0, &mut mode) != 0 {
                return Self { terminal: true, saved: None };
            }
            let mut quiet = mode;
            quiet.c_lflag &= !libc::ECHO;
            let saved = (libc::tcsetattr(0, libc::TCSANOW, &quiet) == 0).then_some(mode);
            Self { terminal: true, saved }
        }
    }
    #[cfg(windows)]
    fn off() -> Self {
        use windows_sys::Win32::System::Console::{GetConsoleMode, GetStdHandle, SetConsoleMode, CONSOLE_MODE, ENABLE_ECHO_INPUT, STD_INPUT_HANDLE};
        unsafe {
            let handle = GetStdHandle(STD_INPUT_HANDLE);
            let mut mode: CONSOLE_MODE = 0;
            if GetConsoleMode(handle, &mut mode) == 0 {
                return Self { terminal: false, saved: None };
            }
            let saved = (SetConsoleMode(handle, mode & !ENABLE_ECHO_INPUT) != 0).then_some((handle, mode));
            Self { terminal: true, saved }
        }
    }
    #[cfg(not(any(unix, windows)))]
    fn off() -> Self {
        Self { terminal: false, saved: None }
    }
}
impl Drop for Echo {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(mode) = self.saved.take() {
            unsafe { libc::tcsetattr(0, libc::TCSANOW, &mode) };
        }
        #[cfg(windows)]
        if let Some((handle, mode)) = self.saved.take() {
            unsafe { windows_sys::Win32::System::Console::SetConsoleMode(handle, mode) };
        }
    }
}
