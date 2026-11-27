// cases.rs — the entries of each context: derived from the catalogue where a type answers, written out where a gesture set is not a family.
// A reader would get wrong that a hand-written entry is an exception: it is named for the gesture and carries its own message, not the catalogue's sentence.

use super::{Entry, Item, MenuCtx, Text, build, quote_arg};
use crate::palette::CommandInfo;
use crate::ui::prefs::Prefs;
use song_edit::chain_item_edit::timeline_lane_of;

/// One argument the clicked thing can answer, in the order the command asks: the precise provider, or the scalar's class.
struct Slot<'a> {
    provider: Option<&'a str>,
    kind: Option<&'a str>,
    /// The argument's own name, when even the provider cannot tell two commands apart.
    name: Option<&'a str>,
    value: String,
}

fn provider<'a>(p: &'a str, value: &str) -> Slot<'a> {
    Slot {
        provider: Some(p),
        kind: None,
        name: None,
        value: value.to_owned(),
    }
}

fn kind<'a>(k: &'a str, value: &str) -> Slot<'a> {
    Slot {
        provider: None,
        kind: Some(k),
        name: None,
        value: value.to_owned(),
    }
}

/// Every catalogue command whose leading arguments are exactly what this thing can answer: the whole mechanism.
fn from_catalogue(commands: &[CommandInfo], slots: &[Slot]) -> Vec<Entry> {
    commands
        .iter()
        .filter(|c| {
            c.args.len() >= slots.len()
                && slots.iter().enumerate().all(|(i, s)| {
                    let a = &c.args[i];
                    let shape = match s.provider {
                        Some(p) => a.provider.as_deref() == Some(p),
                        None => Some(a.kind.as_str()) == s.kind,
                    };
                    shape && s.name.is_none_or(|n| a.name == n)
                })
        })
        .map(|c| Entry {
            verb: c.command.clone(),
            arg: Some(slots.iter().map(|s| quote_arg(&s.value)).collect::<Vec<_>>().join(" ")),
            // Anything left is still to type: the trailing space is where the caret lands.
            needs_more: c.args.len() > slots.len(),
            help: None,
            name: None,
        })
        .collect()
}

/// The verbs that still mean something on a node nothing answers to: each acts on graph.k7x alone.
const SURVIVES_MISSING: [&str; 2] = ["node bypass", "graph unlink"];

fn msg(key: &str) -> Text {
    Text::msg(key)
}

/// A number as the page spells it in a line.
fn num(n: f64) -> String {
    if n == 0.0 { "0".to_owned() } else { format!("{n}") }
}

pub(super) fn menu(ctx: &MenuCtx, commands: &[CommandInfo], prefs: &Prefs, locale: &str) -> Vec<Item> {
    let from = |slots: &[Slot]| from_catalogue(commands, slots);
    let also: &[&str] = match ctx {
        MenuCtx::Module { rack: Some(true), .. } => &["rack"],
        _ => &[],
    };
    // Bound once, so no case can forget the prefs or the head, which would ignore what someone turned off.
    let offer = |entries: Vec<Entry>| build(commands, entries, prefs, ctx.clicked_head(), locale, ctx.entity(), also);
    let ui_file = |arg: &str, help: &str| Entry::new("ui file").arg(arg).help(msg(help));
    match ctx {
        MenuCtx::Tap { id, symbol } => {
            // Not derived: the rows are named for the pad, in the order a kit is edited.
            let at = format!("{} {}", quote_arg(id), quote_arg(symbol));
            offer(vec![
                Entry::new("node tap note").arg(&at).more().named(msg("ctx.tapNote")),
                Entry::new("node tap sample")
                    .arg(&at)
                    .more()
                    .named(msg("ctx.tapSample")),
                Entry::new("node tap rename")
                    .arg(&at)
                    .more()
                    .named(msg("ctx.tapRename")),
                Entry::new("node tap remove").arg(&at).named(msg("ctx.tapRemove")),
                Entry::new("node tap insert")
                    .arg(format!("{} before {}", quote_arg(id), quote_arg(symbol)))
                    .named(msg("ctx.tapBefore")),
                Entry::new("node tap insert")
                    .arg(format!("{} after {}", quote_arg(id), quote_arg(symbol)))
                    .named(msg("ctx.tapAfter")),
                // Its own chain of effects, in a popin: a rack's tap or a kit's pad alike.
                Entry::new("ui tap chain")
                    .arg(&at)
                    .named(msg("ctx.tapChain"))
                    .help(msg("ctx.tapChainHelp")),
            ])
        }
        MenuCtx::Track { id, picked } => {
            let several = picked.as_ref().filter(|p| p.len() > 1);
            let mut entries = Vec::new();
            // Several lanes picked: grouping them comes first, and the clicked lane's own one-track group is not offered beside it.
            if let Some(p) = several {
                entries.push(
                    Entry::new("track group")
                        .arg(quote_arg(&p.join(",")))
                        .named(Text::msg_with("ctx.groupTracks", "n", p.len())),
                );
            }
            entries.extend(
                from(&[provider("tracks", id)])
                    .into_iter()
                    .filter(|e| several.is_none() || e.verb != "track group"),
            );
            // The shell's own, and last: everything above acts on the track, this one opens the file that spells it.
            entries.push(ui_file(id, "ctx.trackFileHelp"));
            offer(entries)
        }
        // Not derived, deliberately: a group answers `track mute` and `track solo` but is not a value the `tracks` provider lists.
        MenuCtx::Group { id } => offer(vec![
            Entry::new("track mute").arg(id.as_str()),
            Entry::new("track solo").arg(id.as_str()),
        ]),
        MenuCtx::Module {
            id,
            missing,
            ambient,
            model,
            native,
            ..
        } => {
            // A block in the graph is both a node and the module behind it, so the menu offers both families, the use first.
            let own: Vec<Entry> = from(&[provider("nodes", id)])
                .into_iter()
                .chain(from(&[provider("modules", id)]))
                .filter(|e| *native == Some(true) || e.verb != "node external-ui")
                .collect();
            if *missing == Some(true) {
                return offer(
                    own.into_iter()
                        .filter(|e| SURVIVES_MISSING.contains(&e.verb.as_str()))
                        .collect(),
                );
            }
            let mut entries = Vec::new();
            // First when it applies: nothing else on this node can work until the model is here.
            if *model == Some(true) {
                entries.push(
                    Entry::new("ui model")
                        .arg(id.as_str())
                        .named(msg("ctx.downloadModel"))
                        .help(msg("ctx.downloadModelHelp")),
                );
            }
            entries.extend(own);
            // A module the engine merely reports is an engine ref, which `module own` takes; with no file anywhere, it replaces `ui file`.
            if *ambient == Some(true) {
                entries.extend(from(&[provider("engineModules", id)]));
            } else {
                entries.push(ui_file(id, "ctx.moduleFileHelp"));
            }
            offer(entries)
        }
        MenuCtx::Clip { id, track, tc, sel } => {
            // Every verb the rectangle can answer whole, so the derivation below does not offer the same verb one argument short.
            let range = sel.as_ref().map_or_else(Vec::new, |s| {
                from(&[
                    provider("tracks", &s.tracks),
                    kind("timecode", &s.from),
                    kind("timecode", &s.to),
                ])
            });
            let ranged: Vec<String> = range.iter().map(|e| e.verb.clone()).collect();
            // Deliberately not the `clip ...` family: what was right-clicked is an event, and the material may sit on other lanes too.
            let mut entries = vec![ui_file(id, "ctx.clipFileHelp")];
            entries.extend(range);
            if let (Some(t), Some(c)) = (
                track.as_deref().filter(|t| !t.is_empty()),
                tc.as_deref().filter(|c| !c.is_empty()),
            ) {
                entries.extend(
                    from(&[provider("tracks", t), kind("timecode", c)])
                        .into_iter()
                        .filter(|e| !ranged.contains(&e.verb)),
                );
            }
            offer(entries)
        }
        MenuCtx::Param {
            id,
            param,
            label: _,
            cc,
            automated,
        } => {
            // Two shapes, one control: `node param` names a use and `module cc` the module the map follows.
            let named = |p: &'static str| Slot {
                provider: None,
                kind: Some("text"),
                name: Some(p),
                value: param.clone(),
            };
            let target = [provider("nodes", id), named("param")];
            let on_module = [provider("modules", id), named("param")];
            let quoted = format!("{id} {}", quote_arg(param));
            let mut entries = vec![
                // Learn arms the browser's Web MIDI, and still goes through the bar like everything else.
                Entry::new("ui midi learn").arg(&quoted).help(msg("ctx.midiLearnHelp")),
                // The curve editor on it: the curve this param is bound to, or a new one bound on Save.
                Entry::new("ui automate").arg(&quoted).help(msg("ctx.automateHelp")),
            ];
            // The unbind, only when a curve drives it: `none` is a literal the entry writes, so it is named.
            if *automated == Some(true) {
                entries.push(
                    Entry::new("node automate")
                        .arg(format!("{id} {param} none"))
                        .named(msg("ctx.removeAutomation"))
                        .help(msg("ctx.removeAutomationHelp")),
                );
            }
            entries.extend(from(&target));
            entries.extend(from(&on_module));
            if cc.is_some() {
                entries.push(
                    Entry::new("module cc")
                        .arg(format!("{quoted} none"))
                        .named(msg("ctx.removeMidi"))
                        .help(msg("ctx.removeMidiHelp")),
                );
            }
            offer(entries)
        }
        MenuCtx::CurvePoint { curve, tc, .. } => {
            let at = format!("{curve} {tc}");
            offer(vec![
                Entry::new("curve point add").arg(&at).more(),
                Entry::new("curve point move").arg(&at).more(),
                Entry::new("curve point remove").arg(&at),
            ])
        }
        MenuCtx::CurveCard { curve, module, param } => {
            // A free card has no param to automate: its one row opens the curve itself.
            let (Some(module), Some(param)) = (
                module.as_deref().filter(|m| !m.is_empty()),
                param.as_deref().filter(|p| !p.is_empty()),
            ) else {
                return offer(vec![
                    Entry::new("ui curve")
                        .arg(curve.as_str())
                        .help(msg("ctx.editCurveHelp")),
                ]);
            };
            offer(vec![
                // The same editor the param's own menu opens, on the curve this card is.
                Entry::new("ui automate")
                    .arg(format!("{module} {}", quote_arg(param)))
                    .help(msg("ctx.editCurveParamHelp")),
                Entry::new("node automate")
                    .arg(format!("{module} {param} none"))
                    .named(msg("ctx.removeAutomation"))
                    .help(msg("ctx.removeAutomationCurveHelp")),
            ])
        }
        // A chain answers no command's first argument, and its one reader verb is gone; a sink has nothing per entity.
        MenuCtx::Chain { .. } | MenuCtx::Sink { .. } => vec![],
        MenuCtx::Assembly { id, uses } => {
            // Derived like every other family, plus the one door out of the box: its template.
            let mut entries = from(&[provider("assemblies", id)]);
            entries.push(
                Entry::new("ui assembly")
                    .arg(uses.as_str())
                    .help(msg("ctx.assemblyHelp")),
            );
            offer(entries)
        }
        MenuCtx::MarkerAdd { tc, loop_mode } => {
            // A bare timecode: `project marker new <tc> `, leaving the text cursor where the name is typed.
            let mut entries = from(&[kind("timecode", tc)]);
            // The shell's own: it acts on the selection the ruler swept, which no argument of the line carries.
            if let Some(mode) = loop_mode {
                let (name, help) = match mode {
                    super::LoopMode::Set => ("ctx.loopSet", "ctx.loopSetHelp"),
                    super::LoopMode::Clear => ("ctx.loopClear", "ctx.loopClearHelp"),
                };
                entries.push(Entry::new("transport loop").named(msg(name)).help(msg(help)));
            }
            offer(entries)
        }
        MenuCtx::Marker { name } => offer(from(&[provider("markerNames", name)])),
        MenuCtx::Timeline { id } => offer(from(&[provider("timelines", id)])),
        MenuCtx::WarpPoint { clip, src } => offer(vec![Entry::new("clip warp remove").arg(format!("{clip} {src}"))]),
        MenuCtx::PictureShot { lane, at, .. } => {
            // The lane and the start address the shot; the three that need a destination go to the bar with it typed.
            let where_ = format!("{lane} {at}");
            let open = format!("{where_} ");
            offer(vec![
                Entry::new("picture move").arg(&open).more(),
                Entry::new("picture head").arg(&open).more(),
                Entry::new("picture length").arg(&open).more(),
                Entry::new("picture silence").arg(&where_),
                Entry::new("picture delete").arg(&where_),
            ])
        }
        MenuCtx::PictureLane { lane } => {
            // An audio lane's rows, in its order, where a picture lane has the same act.
            let q = quote_arg(lane);
            let open = format!("{q} ");
            let shared = [
                Entry::new("picture lane rename").arg(&open).more(),
                Entry::new("timeline lane copy").arg(&open).more(),
            ];
            let shot = Entry::new("picture add").arg(&open).more();
            if let Some((timeline, _)) = timeline_lane_of(lane) {
                // A timeline's lane: its frame, visibility, order and camera are the song lane's it draws on.
                let mut entries = shared.to_vec();
                entries.push(Entry::new("picture lane delete").arg(&q));
                entries.push(shot);
                entries.push(ui_file(&format!("timelines/{timeline}.k7x"), "ctx.timelineFileHelp"));
                return offer(entries);
            }
            let mut entries = vec![
                Entry::new("picture lane order").arg(&open).more(),
                Entry::new("timeline lane copy")
                    .arg(format!("{q} main"))
                    .named(msg("ctx.duplicate"))
                    .help(msg("ctx.duplicateHelp")),
            ];
            entries.extend(shared);
            entries.extend([
                Entry::new("picture mute").arg(&q),
                Entry::new("picture solo").arg(&q),
                Entry::new("picture lane delete").arg(&q),
                shot,
                Entry::new("ui picture arm").arg(&q),
                ui_file(lane, "ctx.laneFileHelp"),
            ]);
            offer(entries)
        }
        MenuCtx::Note {
            sequence,
            note,
            start,
            end,
            at,
            region,
        } => {
            // The roll's own two, both already addressed by the click: nothing here owes an argument to the bar.
            let where_ = format!("{sequence} {} {}", num(*note), num(*start));
            let mut rows = vec![Entry::new("sequence note remove").arg(&where_)];
            // Split is offered only where there is something to split: a cut on either edge would leave a note of length zero.
            if let Some(a) = at
                && *a > start + 1e-6
                && *a < end - 1e-6
            {
                rows.push(Entry::new("sequence note split").arg(format!("{where_} {}", num(*a))));
            }
            // Then the rectangle's four, when the click landed in one. The three that owe a value go to the bar with the box spelt.
            if let Some(r) = region {
                let b = format!("{sequence} {} {} {} {}", num(r.lo), num(r.hi), num(r.from), num(r.to));
                rows.push(Entry::new("sequence notes remove").arg(&b));
                for verb in [
                    "sequence notes transpose",
                    "sequence notes quantize",
                    "sequence notes velocity",
                ] {
                    rows.push(Entry::new(verb).arg(&b).more());
                }
            }
            offer(rows)
        }
    }
}
