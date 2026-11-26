//! commands.rs — what the registry part does for the app's commands: add a studio (after its hello proves it a
//! registry), pair with one (the same, then its device token is kept), remove one, and leave every one.
//! What a reader gets wrong: the hello comes BEFORE anything is written, since a heartbeat carries the token.

use tokio::sync::mpsc;

use super::state::{State, instance_key};
use super::{Done, Reply};
use crate::protocol::{Command, InstanceCheck, UNIT_SEP};
use crate::registry::{self, Probe};
use crate::urls;

/// A hello answered, with what asked for it.
pub struct Probed {
    cmd: Command,
    registry: String,
    origin: String,
    /// The device token a pairing brings; none for a plain add.
    token: Option<String>,
    probe: Probe,
}

fn unknown() -> Probe {
    Probe {
        ok: false,
        why: String::new(),
    }
}

impl State {
    pub async fn command(&mut self, cmd: Command, dtx: &mpsc::UnboundedSender<Done>) {
        match cmd.cmd.as_str() {
            "weburi.add" => self.web_add(cmd, dtx),
            "weburi.pair" => self.web_pair(cmd, dtx),
            "weburi.remove" => {
                self.remove_instance(&cmd.arg).await;
                self.cx.reply(cmd.id, true, "");
            }
            "registry.deregister" => {
                self.light = false;
                self.publish();
                let list = self.urls.clone();
                self.leave(list).await;
                self.cx.reply(cmd.id, true, "");
            }
            other => self
                .cx
                .reply(cmd.id, false, &format!("{other} is not a registry command")),
        }
    }

    fn spawn_probe(&self, p: Probed, dtx: &mpsc::UnboundedSender<Done>) {
        let dtx = dtx.clone();
        tokio::spawn(async move {
            let probe = registry::probe(&p.registry).await;
            let _ = dtx.send(Done::Probed(Probed { probe, ..p }));
        });
    }

    fn web_add(&mut self, cmd: Command, dtx: &mpsc::UnboundedSender<Done>) {
        let Some(origin) = urls::normalise_user_origin(&cmd.arg) else {
            let message = format!("{} is not a web address", cmd.arg);
            self.check = Some(InstanceCheck {
                url: cmd.arg.clone(),
                state: "refused".into(),
                message,
            });
            self.publish();
            self.cx.reply(cmd.id, false, "not a valid web UI address");
            return;
        };
        // ASKED before anything is written: a heartbeat carries this engine's token.
        let reg = urls::engine_registry_url(&origin);
        self.check = Some(InstanceCheck {
            url: reg.clone(),
            state: "checking".into(),
            message: String::new(),
        });
        self.publish();
        self.spawn_probe(
            Probed {
                cmd,
                registry: reg,
                origin,
                token: None,
                probe: unknown(),
            },
            dtx,
        );
    }

    /// `<origin>U+001F<token>`: the studio an account of this machine paired with, listed with the token that names its owner there.
    fn web_pair(&mut self, cmd: Command, dtx: &mpsc::UnboundedSender<Done>) {
        let (named, token) = cmd.arg.split_once(UNIT_SEP).unwrap_or((&cmd.arg, ""));
        let (Some(origin), false) = (urls::normalise_user_origin(named), token.is_empty()) else {
            self.cx.reply(
                cmd.id,
                false,
                "weburi.pair wants <origin>\u{1f}<device token>",
            );
            return;
        };
        let (reg, token) = (urls::engine_registry_url(&origin), token.to_string());
        // Probed like an add: the token is only ever sent to a registry that answers as one.
        self.spawn_probe(
            Probed {
                cmd,
                registry: reg,
                origin,
                token: Some(token),
                probe: unknown(),
            },
            dtx,
        );
    }

    /// A hello came back: refused, or the studio is added (and paired when a token came with it).
    pub fn probed(&mut self, p: Probed, dtx: &mpsc::UnboundedSender<Done>) {
        let Probed {
            cmd,
            registry: reg,
            origin,
            token,
            probe,
        } = p;
        if !probe.ok {
            let what = if token.is_some() {
                "pairing with"
            } else {
                "adding"
            };
            crate::log::line("registry", &format!("not {what} {reg}: {}", probe.why));
            if token.is_none() {
                let message = format!("{} — nothing was added", probe.why);
                self.check = Some(InstanceCheck {
                    url: reg,
                    state: "refused".into(),
                    message,
                });
                self.publish();
            }
            self.cx.reply(cmd.id, false, &probe.why);
            return;
        }
        self.check = None;
        if let Some(token) = token
            && self.tokens.get(&reg) != Some(&token)
        {
            // Keyed as `headers_for` looks it up, and saved before the list, so the first heartbeat carries it.
            self.tokens.insert(reg.clone(), token.clone());
            let key = reg.clone();
            self.cx.update_settings(move |s| {
                s.device_tokens.insert(key, token);
            });
            crate::log::line(
                "registry",
                &format!("paired with {reg}: its heartbeats name the account's owner there"),
            );
        }
        self.add_registry(
            &origin,
            Reply {
                id: cmd.id,
                ok: true,
                message: origin.clone(),
            },
            dtx,
        );
    }

    /// ADD a registry, always its family's root: persist, adopt, announce; an add and never a replace.
    fn add_registry(&mut self, named: &str, reply: Reply, dtx: &mpsc::UnboundedSender<Done>) {
        let u = urls::engine_registry_url(named);
        if u.is_empty() {
            self.cx.reply(reply.id, reply.ok, &reply.message);
            return;
        }
        if !self.urls.contains(&u) {
            crate::log::line("registry", &format!("added {u}"));
            self.urls.push(u);
            // Persisted as the explicit list, so the next boot takes it rather than the ambient env var.
            self.pinned.clone_from(&self.urls);
            let list = self.urls.clone();
            self.cx.update_settings(move |s| s.registry_urls = list);
            self.cx
                .set_origins(crate::engine::allowed_origins(&self.urls));
        }
        self.publish();
        self.beat(dtx, Some(reply));
    }

    /// Disconnect one instance: its registry leaves the pinned list and is told at once.
    async fn remove_instance(&mut self, named: &str) {
        let key = instance_key(named);
        let (dropped, kept): (Vec<String>, Vec<String>) = self
            .pinned
            .iter()
            .cloned()
            .partition(|p| instance_key(p) == key);
        // Its device token goes with it: a studio removed and later added by hand must not prove an account behind the user's back.
        let had_tokens = self.tokens.keys().any(|t| instance_key(t) == key);
        self.tokens.retain(|t, _| instance_key(t) != key);
        let tokens = self.tokens.clone();
        if dropped.is_empty() {
            if had_tokens {
                self.cx.update_settings(move |s| s.device_tokens = tokens);
            }
            self.publish();
            return;
        }
        self.pinned.clone_from(&kept);
        self.cx.update_settings(move |s| {
            s.device_tokens = tokens;
            s.registry_urls = kept;
        });
        for d in &dropped {
            crate::log::line("registry", &format!("removed {d}"));
        }
        // Emptied, the list falls back to `$STUDIO_REGISTRY_URL`: what a fresh boot would resolve.
        self.urls = registry::resolve(&self.pinned);
        self.cx
            .set_origins(crate::engine::allowed_origins(&self.urls));
        self.publish();
        if self.up() {
            self.leave(registry::resolve(&dropped)).await;
        }
    }
}
