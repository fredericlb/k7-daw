//! frames.rs — where the host protocol's frames go: the real stdout, a pipe of their own inside an app (its
//! stdout is shared), a channel for a host in this process, or nowhere under `--config`. The one writer of
//! stdout: one stray line there and the app parses garbage (`apps/k7-daw-engine/k7-supervisor/docs/hostProtocol.md`).

use std::fs::File;
use std::io::Write;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;

enum Out {
    Stdout,
    File(File),
    Channel(UnboundedSender<String>),
    Silent,
}

/// A frame sink, cloned into whatever emits; each frame is one line, written whole.
#[derive(Clone)]
pub struct Frames(Arc<Mutex<Out>>);

impl Frames {
    pub fn stdout() -> Frames {
        Frames(Arc::new(Mutex::new(Out::Stdout)))
    }
    /// Frames to a file or pipe the app reads, as `k7_supervisor_run` is handed.
    pub fn file(f: File) -> Frames {
        Frames(Arc::new(Mutex::new(Out::File(f))))
    }
    /// Each frame's line, newline excluded, to a receiver in this process.
    pub fn channel(tx: UnboundedSender<String>) -> Frames {
        Frames(Arc::new(Mutex::new(Out::Channel(tx))))
    }
    /// `--config`: no host reads stdout.
    pub fn silent() -> Frames {
        Frames(Arc::new(Mutex::new(Out::Silent)))
    }

    /// One frame, compact; a failed write is the app gone, and the frame is dropped.
    pub fn emit(&self, frame: &Value) {
        self.emit_line(&frame.to_string());
    }

    /// A frame already serialized, as the snapshot's comparison has it.
    pub fn emit_line(&self, text: &str) {
        let mut out = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let line = format!("{text}\n");
        match &mut *out {
            Out::Stdout => {
                let mut o = std::io::stdout().lock();
                let _ = o.write_all(line.as_bytes()).and_then(|_| o.flush());
            }
            Out::File(f) => {
                let _ = f.write_all(line.as_bytes());
            }
            Out::Channel(tx) => {
                let _ = tx.send(text.to_string());
            }
            Out::Silent => {}
        }
    }
}
