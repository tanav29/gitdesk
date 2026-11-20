use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use rune::{Key, KeyEvent, Waker};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;

struct Replies(mpsc::Sender<Vec<u8>>);
impl vt100::Callbacks for Replies {
    fn unhandled_csi(
        &mut self,
        screen: &mut vt100::Screen,
        prefix: Option<u8>,
        _: Option<u8>,
        args: &[&[u16]],
        code: char,
    ) {
        let reply = match (prefix, code, args.first().and_then(|a| a.first()).copied()) {
            (None, 'n', Some(6)) => {
                let (r, c) = screen.cursor_position();
                format!("\x1b[{};{}R", r + 1, c + 1)
            }
            (None, 'n', Some(5)) => "\x1b[0n".into(),
            (None, 'c', None | Some(0)) => "\x1b[?1;2c".into(),
            _ => return,
        };
        let _ = self.0.send(reply.into_bytes());
    }
}

pub struct Terminal {
    pub title: String,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn ChildKiller + Send + Sync>,
    exit: mpsc::Receiver<String>,
    input: mpsc::Sender<Vec<u8>>,
    parser: Arc<Mutex<vt100::Parser<Replies>>>,
    dirty: Arc<AtomicBool>,
    pub ended: Option<String>,
}

impl Terminal {
    pub fn open(repo: &Path, wake: Waker) -> Result<Self, String> {
        #[cfg(windows)]
        let cmd =
            CommandBuilder::new(std::env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into()));
        #[cfg(not(windows))]
        let cmd =
            CommandBuilder::new(std::env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into()));
        Self::spawn(repo, wake, cmd, "Terminal".into())
    }

    pub fn command(
        repo: &Path,
        wake: Waker,
        program: &str,
        args: Vec<String>,
    ) -> Result<Self, String> {
        let mut cmd = CommandBuilder::new(program);
        let title = format!("{program} {}", args.first().map_or("", String::as_str));
        cmd.args(args);
        Self::spawn(repo, wake, cmd, title).map_err(|e| {
            format!("Could not start {program}. Make sure it is installed and on PATH. {e}")
        })
    }

    fn spawn(
        repo: &Path,
        wake: Waker,
        mut cmd: CommandBuilder,
        title: String,
    ) -> Result<Self, String> {
        cmd.cwd(repo);
        cmd.env("TERM", "xterm-256color");
        for key in crate::git::REPO_ENV {
            cmd.env_remove(key);
        }
        let size = PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = native_pty_system()
            .openpty(size)
            .map_err(|e| e.to_string())?;
        let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let mut writer = pair.master.take_writer().map_err(|e| e.to_string())?;
        let mut process = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
        let child = process.clone_killer();
        let (done, exit) = mpsc::channel();
        let on_exit = wake.clone();
        thread::spawn(move || {
            let status = process
                .wait()
                .map(|s| format!("Exit code {}", s.exit_code()))
                .unwrap_or_else(|e| e.to_string());
            let _ = done.send(status);
            on_exit();
        });
        drop(pair.slave);
        let (input, stream) = mpsc::channel::<Vec<u8>>();
        let parser = Arc::new(Mutex::new(vt100::Parser::new_with_callbacks(
            size.rows.try_into().unwrap(),
            size.cols.try_into().unwrap(),
            4000,
            Replies(input.clone()),
        )));
        let dirty = Arc::new(AtomicBool::new(true));
        thread::spawn(move || {
            for bytes in stream {
                if writer
                    .write_all(&bytes)
                    .and_then(|()| writer.flush())
                    .is_err()
                {
                    break;
                }
            }
        });
        let output = parser.clone();
        let changed = dirty.clone();
        thread::spawn(move || {
            let mut buf = [0; 16384];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                output.lock().unwrap().process(&buf[..n]);
                if !changed.swap(true, Ordering::AcqRel) {
                    wake();
                }
            }
            changed.store(true, Ordering::Release);
            wake();
        });
        Ok(Self {
            title,
            master: pair.master,
            child,
            exit,
            input,
            parser,
            dirty,
            ended: None,
        })
    }

    pub fn poll(&mut self) -> bool {
        if let Ok(status) = self.exit.try_recv() {
            self.ended = Some(status);
            return true;
        }
        self.dirty.swap(false, Ordering::AcqRel)
    }

    pub fn screen<T>(&self, f: impl FnOnce(&vt100::Screen) -> T) -> T {
        f(self.parser.lock().unwrap().screen())
    }

    pub fn resize(&self, rows: u16, cols: u16) {
        let mut parser = self.parser.lock().unwrap();
        let size = (
            rows.max(1).try_into().unwrap(),
            cols.max(1).try_into().unwrap(),
        );
        if parser.screen().size() != size
            && self
                .master
                .resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .is_ok()
        {
            parser.set_size(size.0, size.1);
        }
    }

    pub fn send(&self, bytes: Vec<u8>) {
        self.parser.lock().unwrap().screen_mut().set_scrollback(0);
        let _ = self.input.send(bytes);
    }

    pub fn paste(&self, value: &str) {
        let value = value.replace("\r\n", "\n");
        if self.screen(|s| s.bracketed_paste()) {
            self.send(format!("\x1b[200~{}\x1b[201~", value.replace('\x1b', "")).into_bytes());
        } else {
            self.send(value.replace('\n', "\r").into_bytes());
        }
    }

    pub fn key(&self, ev: &KeyEvent) -> bool {
        if ev.mods.shift && matches!(ev.key, Key::PageUp | Key::PageDown) {
            let mut parser = self.parser.lock().unwrap();
            let screen = parser.screen_mut();
            let step = usize::from(screen.size().0.get());
            let next = if ev.key == Key::PageUp {
                screen.scrollback().saturating_add(step)
            } else {
                screen.scrollback().saturating_sub(step)
            };
            screen.set_scrollback(next);
            return true;
        }
        let cursor = if self.screen(|s| s.application_cursor()) {
            "\x1bO"
        } else {
            "\x1b["
        };
        let bytes = match &ev.key {
            Key::Text(t) if ev.mods.ctrl && !ev.mods.alt && t.is_ascii() && t.len() == 1 => {
                vec![t.as_bytes()[0] & 31]
            }
            Key::Text(t) if ev.mods.alt && !ev.mods.ctrl => format!("\x1b{t}").into_bytes(),
            Key::Text(t) => t.as_bytes().to_vec(),
            Key::Command(c) if c.is_ascii() && !ev.mods.logo => vec![*c as u8 & 31],
            Key::Enter => vec![b'\r'],
            Key::Backspace => vec![127],
            Key::Escape => vec![27],
            Key::Tab if ev.mods.shift => b"\x1b[Z".to_vec(),
            Key::Tab => vec![9],
            Key::Up | Key::Down | Key::Right | Key::Left | Key::Home | Key::End => {
                let code = match ev.key {
                    Key::Up => 'A',
                    Key::Down => 'B',
                    Key::Right => 'C',
                    Key::Left => 'D',
                    Key::Home => 'H',
                    _ => 'F',
                };
                format!("{cursor}{code}").into_bytes()
            }
            Key::Insert => b"\x1b[2~".to_vec(),
            Key::Delete => b"\x1b[3~".to_vec(),
            Key::PageUp => b"\x1b[5~".to_vec(),
            Key::PageDown => b"\x1b[6~".to_vec(),
            Key::Function(n @ 1..=4) => vec![27, b'O', b'P' + n - 1],
            Key::Function(n @ 5..=12) => format!(
                "\x1b[{}~",
                [15, 17, 18, 19, 20, 21, 23, 24][usize::from(n - 5)]
            )
            .into_bytes(),
            _ => return false,
        };
        self.send(bytes);
        true
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        if self.ended.is_none() {
            let _ = self.child.kill();
        }
    }
}
