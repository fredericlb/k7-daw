//! slots part — the machine's frozen chains (`docs/slots.md`): the rows and the enabled one on the snapshot,
//! the commands `slot.*`, the same verbs for a server's tunnel jobs (`Handle`), the enabled slot sent to the
//! engine at every start and followed in a sessions poll of its own. What a reader gets wrong: the part never
//! stops the engine; a slot "playing" is only one a poll saw loaded, and one seen gone after that means a
//! project took the engine over, so the memory of it is dropped.

mod apply;
mod ops;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde_json::Value;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{MissedTickBehavior, interval};

use crate::core::{Ctx, Event, PartSpec};
use crate::engine_client::EngineClient;
use crate::paths::Paths;
use crate::protocol::{Command, Session, SlotRow, UNIT_SEP};
use crate::slots::{Installed, Readable};

/// What a caller (a command, a tunnel job) asks of the store.
pub enum Op {
    List,
    /// Store the slot `staged` holds; `readable` limits the files it may copy in.
    Save {
        staged: PathBuf,
        readable: Option<Readable>,
    },
    Enable(String),
    Disable,
    Remove(String),
    Footswitch(String, Option<i64>),
}

/// The store as it then stands, and the id a save made.
pub struct Answer {
    pub saved: Option<String>,
    pub slots: Value,
}

type Reply = oneshot::Sender<Result<Answer, String>>;

/// A way into the part for the tunnel's jobs, which have a `Ctx` of another part.
#[derive(Clone)]
pub struct Handle(mpsc::UnboundedSender<(Op, Reply)>);

impl Handle {
    pub async fn ask(&self, op: Op) -> Result<Answer, String> {
        let (tx, rx) = oneshot::channel();
        self.0
            .send((op, tx))
            .map_err(|_| "this machine keeps no slots".to_string())?;
        rx.await
            .map_err(|_| "this machine keeps no slots".to_string())?
    }
}

static HANDLES: Mutex<BTreeMap<PathBuf, Handle>> = Mutex::new(BTreeMap::new());

/// The part of the core keeping the state folder `paths` names; None when no core runs it.
pub fn handle(paths: &Paths) -> Option<Handle> {
    HANDLES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(paths.studio_dir())
        .cloned()
}

static PACE: OnceLock<Duration> = OnceLock::new();

/// How often the sessions are asked while a slot is being followed; only a test has a reason to change it.
pub fn set_pace(p: Duration) {
    let _ = PACE.set(p);
}

pub fn spec() -> PartSpec {
    PartSpec {
        name: "slots",
        commands: &[
            "slot.save",
            "slot.enable",
            "slot.disable",
            "slot.remove",
            "slot.footswitch",
        ],
        start,
    }
}

/// Everything the task answers itself.
enum Inner {
    Sessions(u64, Option<Vec<Session>>),
    Saved(Result<String, String>, Reply),
}

pub(crate) struct State {
    cx: Ctx,
    client: EngineClient,
    inner: mpsc::UnboundedSender<Inner>,
    rows: Vec<SlotRow>,
    enabled: String,
    up: bool,
    port: u16,
    /// The enabled slot sent and not yet seen loaded, or seen loaded.
    pending: bool,
    live: bool,
    polling: bool,
}

fn start(cx: Ctx, rx: mpsc::UnboundedReceiver<Event>) {
    let (req_tx, req_rx) = mpsc::unbounded_channel();
    HANDLES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(cx.paths.studio_dir().to_path_buf(), Handle(req_tx));
    tokio::spawn(run(cx, rx, req_rx));
}

async fn run(
    cx: Ctx,
    mut rx: mpsc::UnboundedReceiver<Event>,
    mut requests: mpsc::UnboundedReceiver<(Op, Reply)>,
) {
    let (inner_tx, mut inner) = mpsc::unbounded_channel();
    let port = cx.engine.borrow().port;
    let mut st = State {
        client: EngineClient::new(port),
        cx,
        inner: inner_tx,
        rows: Vec::new(),
        enabled: String::new(),
        up: false,
        port: 0,
        pending: false,
        live: false,
        polling: false,
    };
    let mut tick = interval(PACE.get().copied().unwrap_or(Duration::from_secs(2)));
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                None => break,
                Some(Event::Boot(ack)) => {
                    st.refresh();
                    let _ = ack.send(());
                }
                Some(Event::EngineUp(up)) => st.engine_up(up.port),
                Some(Event::EngineDown { ack, .. }) => {
                    st.engine_down();
                    if let Some(a) = ack {
                        let _ = a.send(());
                    }
                }
                Some(Event::Command(c)) => st.command(c),
                Some(Event::Shutdown(ack)) => {
                    st.client.close();
                    let _ = ack.send(());
                }
            },
            Some((op, reply)) = requests.recv() => st.run_op(op, reply),
            Some(done) = inner.recv() => st.inner(done),
            _ = tick.tick() => st.poll_sessions(),
        }
    }
    HANDLES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(st.cx.paths.studio_dir());
}

impl State {
    /// `slot.save <staged file>`, `slot.enable <id>`, `slot.disable`, `slot.remove <id>`, `slot.footswitch <id>[\x1f<cc>]`.
    fn command(&mut self, c: Command) {
        let op = match c.cmd.as_str() {
            "slot.save" => Op::Save {
                staged: PathBuf::from(&c.arg),
                readable: None,
            },
            "slot.enable" => Op::Enable(c.arg.clone()),
            "slot.disable" => Op::Disable,
            "slot.remove" => Op::Remove(c.arg.clone()),
            "slot.footswitch" => {
                // `<id>\x1f<cc>` binds it, `<id>` alone lets it go (`hostProtocol.md`).
                let (id, cc) = match c.arg.split_once(UNIT_SEP) {
                    None => (c.arg.as_str(), None),
                    Some((id, n))
                        if (1..=3).contains(&n.len()) && n.bytes().all(|d| d.is_ascii_digit()) =>
                    {
                        (id, n.parse::<i64>().ok())
                    }
                    Some(_) => {
                        self.cx
                            .reply(c.id, false, "a footswitch is a CC number, 0 to 127");
                        return;
                    }
                };
                Op::Footswitch(id.to_string(), cc)
            }
            other => {
                self.cx
                    .reply(c.id, false, &format!("{other} is not a slot command"));
                return;
            }
        };
        let (tx, rx) = oneshot::channel();
        self.run_op(op, tx);
        let cx = self.cx.clone();
        tokio::spawn(async move {
            match rx.await {
                Ok(Ok(a)) => cx.reply(c.id, true, a.saved.as_deref().unwrap_or("")),
                Ok(Err(why)) => cx.reply(c.id, false, &why),
                Err(_) => {}
            }
        });
    }
}

/// What is installed here, read now, for a save that copies off it: the plugin scan the engine holds, the instruments on disk.
pub(crate) async fn installed_now(client: &EngineClient, cx: &Ctx) -> Installed {
    let mut plugins = std::collections::BTreeSet::new();
    for p in client
        .list_plugins()
        .await
        .map(|c| c.plugins)
        .unwrap_or_default()
    {
        plugins.insert(format!(
            "{}\u{1f}{}",
            p.format.to_lowercase(),
            p.name.to_lowercase()
        ));
    }
    let root = cx.paths.instruments_dir();
    Installed {
        plugin: Some(Arc::new(move |f, n| {
            plugins.contains(&format!("{}\u{1f}{}", f.to_lowercase(), n.to_lowercase()))
        })),
        instrument: Some(Arc::new(move |n| {
            crate::instruments::find(n).is_some_and(|i| crate::instruments::installed(&root, i))
        })),
        readable: None,
    }
}
