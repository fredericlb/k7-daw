//! ops.rs — what the part does to the store for each verb: save (off the task, since an SFZ's folder can be
//! large), enable, disable, remove, bind a footswitch; each answers with the store as it then stands.

use serde_json::{Value, json};

use super::{Answer, Inner, Op, Reply, State, installed_now};
use crate::slots;

impl State {
    /// The slots as a server's `slots` reply lists them.
    pub(super) fn slots_json(&self) -> Value {
        let rows: Vec<Value> = self
            .rows
            .iter()
            .map(|s| json!({"id": s.id, "name": s.name, "chain": s.chain, "savedAt": s.saved_at, "footswitch": s.footswitch}))
            .collect();
        let enabled = (!self.enabled.is_empty()).then_some(&self.enabled);
        json!({"type": "slots", "slots": rows, "enabledSlot": enabled})
    }

    fn answer(&self, saved: Option<String>) -> Result<Answer, String> {
        Ok(Answer {
            saved,
            slots: self.slots_json(),
        })
    }

    pub(super) fn run_op(&mut self, op: Op, reply: Reply) {
        let dir = self.cx.paths.slots_dir();
        let result = match op {
            Op::List => self.answer(None),
            Op::Save { staged, readable } => {
                let (client, cx, inner) =
                    (self.client.clone(), self.cx.clone(), self.inner.clone());
                tokio::spawn(async move {
                    let mut installed = installed_now(&client, &cx).await;
                    installed.readable = readable;
                    let saved =
                        tokio::task::spawn_blocking(move || slots::save(&dir, &staged, &installed))
                            .await
                            .unwrap_or_else(|_| Err("the save stopped unexpectedly".into()));
                    let _ = inner.send(Inner::Saved(saved, reply));
                });
                return;
            }
            Op::Enable(id) => {
                if slots::document(&dir, &id).is_none() {
                    Err(format!("no slot '{id}'"))
                } else {
                    slots::set_enabled(&dir, Some(&id));
                    self.refresh();
                    self.apply();
                    self.answer(None)
                }
            }
            Op::Footswitch(id, cc) => {
                if slots::set_footswitch(&dir, &id, cc) {
                    self.refresh();
                    if id == self.enabled {
                        self.apply();
                    }
                    self.answer(None)
                } else {
                    let cc = cc.map(|c| c.to_string()).unwrap_or_default();
                    Err(format!("no slot '{id}', or no CC {cc}"))
                }
            }
            Op::Disable => return self.drop_slot(None, reply),
            Op::Remove(id) => return self.drop_slot(Some(id), reply),
        };
        let _ = reply.send(result);
    }

    /// Disable the enabled slot, or remove `id` (disabling it first); answers once the engine let it go.
    fn drop_slot(&mut self, remove: Option<String>, reply: Reply) {
        let dir = self.cx.paths.slots_dir();
        let was_enabled = remove.as_ref().is_none_or(|r| *r == self.enabled);
        match &remove {
            Some(id) if !slots::remove(&dir, id) => {
                let _ = reply.send(Err(format!("no slot '{id}'")));
                return;
            }
            None => slots::set_enabled(&dir, None),
            _ => {}
        }
        self.refresh();
        if !was_enabled {
            let _ = reply.send(self.answer(None));
            return;
        }
        self.pending = false;
        self.live = false;
        let (client, answer) = (self.client.clone(), self.answer(None));
        tokio::spawn(async move {
            client.drop_session(slots::SESSION).await;
            let _ = reply.send(answer);
        });
    }

    /// A save finished off the task: listed, and played at once when it is the enabled one.
    pub(super) fn saved(&mut self, saved: Result<String, String>, reply: Reply) {
        match saved {
            Ok(id) => {
                self.refresh();
                if id == self.enabled {
                    self.apply();
                }
                let _ = reply.send(self.answer(Some(id)));
            }
            Err(why) => {
                let _ = reply.send(Err(why));
            }
        }
    }
}
