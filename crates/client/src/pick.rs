//! The system's own "open file" dialog, for choosing a picture.
//!
//! The dialog runs in a helper process on a worker thread (PowerShell on
//! Windows, AppleScript on macOS, zenity or kdialog on Linux), so the
//! launcher keeps drawing while it is open and no GUI library is needed.

use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, TryRecvError};

/// How a dialog ended.
pub enum Picked {
    File(PathBuf),
    /// Closed without choosing.
    Cancelled,
    /// No dialog could be shown here.
    Unavailable,
}

/// A dialog that is open, or has just closed.
pub struct Pick(Receiver<Picked>);

impl Pick {
    /// Open a dialog for one PNG file, titled `title`.
    pub fn png(title: &str) -> Pick {
        let (tx, rx) = channel();
        let title = title.to_string();
        std::thread::spawn(move || {
            let _ = tx.send(run(&title));
        });
        Pick(rx)
    }

    /// The answer once the dialog has closed.
    pub fn poll(&self) -> Option<Picked> {
        match self.0.try_recv() {
            Ok(p) => Some(p),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Picked::Unavailable),
        }
    }
}

/// Run a helper and read the path it prints; `None` when it would not run.
fn ask(mut c: Command) -> Option<Picked> {
    let out = c.output().ok()?;
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Some(if path.is_empty() { Picked::Cancelled } else { Picked::File(PathBuf::from(path)) })
}

#[cfg(windows)]
fn run(title: &str) -> Picked {
    use std::os::windows::process::CommandExt;
    // An invisible topmost form owns the dialog, so it opens over the game
    // rather than behind it.
    let script = format!(
        "[Console]::OutputEncoding = [Text.Encoding]::UTF8; \
         Add-Type -AssemblyName System.Windows.Forms; \
         $o = New-Object Windows.Forms.Form -Property @{{TopMost = $true; ShowInTaskbar = $false}}; \
         $d = New-Object Windows.Forms.OpenFileDialog -Property @{{Filter = 'PNG pictures (*.png)|*.png'; Title = '{}'}}; \
         if ($d.ShowDialog($o) -eq 'OK') {{ [Console]::Out.Write($d.FileName) }}",
        title.replace('\'', "''")
    );
    let mut c = Command::new("powershell");
    c.args(["-NoProfile", "-NonInteractive", "-STA", "-Command", &script]);
    c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    ask(c).unwrap_or(Picked::Unavailable)
}

#[cfg(target_os = "macos")]
fn run(title: &str) -> Picked {
    let prompt = title.replace('\\', "\\\\").replace('"', "\\\"");
    let mut c = Command::new("osascript");
    c.args(["-e", "activate", "-e", &format!("POSIX path of (choose file with prompt \"{prompt}\" of type {{\"public.png\"}})")]);
    ask(c).unwrap_or(Picked::Unavailable)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn run(title: &str) -> Picked {
    let mut zenity = Command::new("zenity");
    zenity.args(["--file-selection", "--title", title, "--file-filter=PNG pictures | *.png *.PNG"]);
    if let Some(p) = ask(zenity) {
        return p;
    }
    let mut kdialog = Command::new("kdialog");
    kdialog.args(["--title", title, "--getopenfilename", ".", "*.png *.PNG|PNG pictures"]);
    ask(kdialog).unwrap_or(Picked::Unavailable)
}
