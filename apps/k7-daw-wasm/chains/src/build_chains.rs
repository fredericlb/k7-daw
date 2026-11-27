// build_chains.rs — the steps of the build that turn the document into cards and cables: each chain's modules with the curves bound to them.
// A reader would get wrong that a step only writes through `Build`: an id or a dedup rule is spelled once, in build.rs.

use crate::build::{Build, chain_id, is_sum, module_id, sink_id, track_id};
use crate::geom::{self, MAIN_OUT, MIX_OUT, RENDER_OUT, VIDEO_OUT, clips_of, node_size, note_size};
use crate::graph::{Chain, Curve, Step};
use crate::node::{CurveOf, Depth, EdgeAutomation, Kind, Node, Port, Pt, Signal};
use crate::node_ui::{step_ui, track_ui, video_track_ui};
use crate::panel::Panel;
use serde_json::Value;

/// One step's contribution to its node: its carried fields and the flags, definite rather than absent.
fn carry_step(s: &Step, n: &mut Node) {
    let get = |k: &str| s.rest.get(k).cloned();
    n.carried.category = get("category");
    n.carried.assets = get("assets");
    n.carried.asset_choices = get("assetChoices");
    n.carried.cc = get("cc");
    n.carried.defaults = get("defaults");
    n.carried.plugin_id = get("pluginId");
    n.carried.channel = get("channel");
    n.carried.tap = get("tap");
    n.carried.alternatives = get("alternatives");
    n.carried.alternative = get("alternative");
    n.carried.engines = get("engines");
    n.carried.engine = get("engine");
    n.carried.rack = get("rack");
    n.carried.rack_kit = get("rackKit");
    n.carried.base = get("base");
    n.carried.shader = get("shader");
    n.carried.uses = get("uses");
    n.carried.place = get("place");
    n.carried.stage = get("stage");
    n.carried.model_download = get("modelDownload");
    n.carried.amp_model = get("ampModel");
    n.inputs = s.inputs.clone();
    n.outputs = s.outputs.clone();
    n.events = s.events.clone();
    n.monitor = s.monitor.clone();
    n.camera = s.camera.clone();
    n.device = s.device.clone();
    n.strip = s.strip;
    n.params = s.params.clone();
    n.missing = s.missing == Some(true);
    n.stub = s.stub == Some(true);
    n.not_supported = s.not_supported == Some(true);
    n.unknown_plugin = s.unknown_plugin == Some(true);
    n.has_native_plugin_ui = s.has_native_plugin_ui == Some(true);
}

/// The node-local centre of the widget bound to `param`: where an automation arrow lands.
fn widget_center(ui: Option<&Panel>, param: &str) -> Option<Pt> {
    let ui = ui?;
    let (_, w) = ui.all_widgets().find(|(_, w)| w.bind.as_deref() == Some(param))?;
    let x = geom::column_w(Some(ui)) + geom::UI_PAD + w.col * (geom::CELL + geom::COL_GAP);
    let y = geom::menu_h(Some(ui)) + geom::UI_CONTENT_TOP + w.row * (geom::CELL + geom::ROW_GAP);
    Some(Pt {
        x: x + geom::cols_width(w.cols()) / 2.0,
        y: y + geom::rows_height(w.rows()) / 2.0,
    })
}

/// Whether a node's events fire this curve: an envelope always, any other when one of its conditions is an event.
fn fired_by_event(curve: &Curve) -> bool {
    if curve.kind == "adsr" {
        return true;
    }
    let conditions = curve
        .spec
        .as_ref()
        .and_then(|s| s.get("conditions"))
        .and_then(Value::as_array);
    conditions.is_some_and(|cs| {
        cs.iter()
            .any(|c| c.get("type").and_then(Value::as_str) == Some("event"))
    })
}

/// A curve card's second line: the kind, plus the oscillator's shape.
fn curve_sub_label(curve: &Curve) -> String {
    match curve.kind.as_str() {
        "lfo" => format!("lfo {}", curve.shape.as_deref().unwrap_or("sine")),
        "adsr" => "adsr".to_owned(),
        _ => "custom".to_owned(),
    }
}

/// How far a curve moves the control it drives; none for an on/off curve.
pub(crate) fn depth_of(curve: &Curve) -> Option<Depth> {
    if curve.kind == "onoff" {
        return None;
    }
    let spec = curve.spec.as_ref();
    let cut = spec
        .and_then(|s| s.get("cut"))
        .and_then(Value::as_str)
        .filter(|c| *c != "none");
    Some(Depth {
        amplitude: spec
            .and_then(|s| s.get("amplitude"))
            .and_then(Value::as_f64)
            .unwrap_or(1.0),
        bipolar: spec.and_then(|s| s.get("polarity")).and_then(Value::as_str) == Some("bipolar"),
        cut: cut.map(str::to_owned),
    })
}

fn curve_card(id: &str, label: &str, curve: &Curve, of: CurveOf) -> Node {
    let mut n = Node::plain(id, label, Kind::Curve);
    n.sub = Some(curve_sub_label(curve));
    n.event_in = Some(fired_by_event(curve));
    n.curve_of = Some(of);
    n.curve_spec = curve.spec.clone().filter(|s| !s.is_null());
    n
}

impl Build<'_> {
    /// A chain's modules, in series, each with its bound curves.
    pub fn add_chain_modules(&mut self, name: &str, c: &Chain) {
        for (i, s) in c.steps.iter().enumerate() {
            let mid = module_id(&s.module);
            // When the same module appears in multiple chains, merge into one node.
            let mut chains: Vec<String> = self.nodes.get(&mid).and_then(|n| n.chains.clone()).unwrap_or_default();
            if !chains.iter().any(|x| x == name) {
                chains.push(name.to_owned());
            }
            // Read once: the node's panel and the node's size are the same answer.
            let ui = step_ui(s, self.opts.machine());
            let mut n = Node::plain(&mid, &s.module, Kind::Module);
            // Only the shared case earns a sub line: editing this module reaches more than one chain.
            n.sub = (chains.len() > 1).then(|| chains.join(", "));
            n.chain = Some(chains[0].clone());
            n.chains = Some(chains);
            n.dim = s.enabled == Some(false);
            n.bypassed = Some(s.enabled == Some(false));
            n.module = Some(s.module.clone());
            n.track_face = self.mixer_owner.get(&s.module).cloned();
            n.frozen = self.graph.frozen.as_ref().and_then(|f| f.get(&s.module)).cloned();
            n.auto_frozen = self.graph.auto_frozen.as_ref().and_then(|f| f.get(&s.module)).cloned();
            carry_step(s, &mut n);
            (n.w, n.h) = node_size(ui.as_deref(), s.strip != Some(true));
            n.ui = ui.map(std::borrow::Cow::into_owned);
            // …and a note is as big as its text, which overrides that: a note has no widget grid.
            if let Some(text) = self
                .graph
                .modules
                .as_ref()
                .and_then(|m| m.get(&s.module))
                .and_then(|d| d.content.as_ref())
            {
                (n.w, n.h) = note_size(text);
                n.note = Some(text.clone());
            }
            self.add(n);
            // Track the maximum position this module occupies across all chains.
            let rank = self.module_rank.get(&mid).copied().unwrap_or(0.0).max(i as f64);
            self.module_rank.insert(mid.clone(), rank);
            if i > 0 {
                let prev = &c.steps[i - 1];
                let prev_id = module_id(&prev.module);
                // The cable between two modules carries what the upstream one emits.
                self.link(&prev_id, &mid, None, prev.emits, None, None);
                // Within-chain series edges get a higher weight so the layout keeps them tight.
                if let Some(e) = self.edges.get_mut(&format!("{prev_id}->{mid}")) {
                    e.weight = Some(e.weight.unwrap_or(1.0) + 4.0);
                }
            }
            self.add_bound_curves(s);
        }
    }

    /// One curve node per binding, the id carrying it.
    fn add_bound_curves(&mut self, s: &Step) {
        let graph = self.graph;
        for a in s.automations.iter().flatten() {
            let Some(curve) = graph.curves.as_ref().and_then(|c| c.get(&a.curve)) else {
                continue;
            };
            self.bound_curves.insert(a.curve.clone());
            let cid = format!("crv:{}@{}:{}", a.curve, s.module, a.param);
            let of = CurveOf {
                curve: a.curve.clone(),
                module: Some(s.module.clone()),
                param: Some(a.param.clone()),
            };
            self.add(curve_card(&cid, &a.curve, curve, of));
            // The node id already names the binding, so the edge key is just its two ends.
            let target = module_id(&s.module);
            let key = format!("{cid}->{target}");
            let mut e = crate::node::Edge::new(&key, &cid, &target);
            e.automation = Some(EdgeAutomation {
                param: a.param.clone(),
                widget: widget_center(s.ui.as_ref(), &a.param),
                depth: depth_of(curve),
            });
            self.edges.insert(key, e);
            // …and the cable that fires it, for an envelope or an event condition.
            if fired_by_event(curve) {
                let from = module_id(a.trigger.as_deref().unwrap_or(&s.module));
                let ek = format!("ev:{from}->{cid}");
                let mut e = crate::node::Edge::new(&ek, &from, &cid);
                e.signal = Some(Signal::Event);
                e.source_port = Some("note-on".to_owned());
                self.edges.insert(ek, e);
            }
        }
    }

    /// A lane's card, the strip of its kind; a routed lane also sizes to the clips it holds.
    fn lane_card(&self, lane: &str, with_clips: bool) -> Node {
        let g = self.graph;
        let video = self.is_video_track(lane);
        let ui = if video { video_track_ui() } else { track_ui() };
        let mut n = Node::plain(&track_id(lane), lane, Kind::Track);
        n.video = Some(video);
        // The two outputs a lane offers, audio first: a picture lane's are its picture and its shots' sound.
        let port = |name: &str, kind| Port {
            name: name.to_owned(),
            kind,
            mandatory: None,
        };
        n.outputs = Some(if video {
            vec![port("video", Signal::Video), port("audio", Signal::Audio)]
        } else {
            vec![port("audio", Signal::Audio), port("midi", Signal::Midi)]
        });
        n.color = g.track_colors.as_ref().and_then(|c| c.get(lane)).cloned();
        // A lane is a module node, so it is addressable like one.
        n.module = Some(lane.to_owned());
        if let Some(alt) = g.track_alternatives.as_ref().and_then(|a| a.get(lane)) {
            n.carried.alternatives = alt.get("alternatives").cloned();
            n.carried.alternative = alt.get("alternative").cloned();
        }
        n.frozen = g.frozen.as_ref().and_then(|f| f.get(lane)).cloned();
        n.auto_frozen = g.auto_frozen.as_ref().and_then(|f| f.get(lane)).cloned();
        let size = node_size(Some(&ui), false);
        (n.w, n.h) = size;
        n.ui = Some(ui);
        // …and again to fit what the lane holds.
        if with_clips
            && let Some(list) = clips_of(g.track_clips.as_ref().and_then(|c| c.get(lane)), size, self.clip_span)
        {
            n.clips = Some(list.clips);
            n.clips_more = list.more;
            (n.w, n.h) = (list.w, list.h);
        }
        n
    }

    /// Where a chain starts: a pass-through card when it holds nothing, then the cable from its `from`.
    pub fn add_chain_source(&mut self, name: &str, c: &Chain) {
        // A pass-through card for a chain holding no module, except either master run, whose ends are cards already.
        if c.steps.is_empty() && !is_sum(name) {
            let mut n = Node::plain(&chain_id(name), name, Kind::Chain);
            n.chain = Some(name.to_owned());
            self.add(n);
        }
        let Some(from) = c.from.as_deref().filter(|f| !f.is_empty()) else {
            return;
        };
        let entry = self.entry_id(name);
        // The master run starts at a sum, which is a built-in output and not a lane.
        if is_sum(from) {
            let sink = self.add_sink(from, false);
            let signal = if from == RENDER_OUT {
                Signal::Video
            } else {
                Signal::Audio
            };
            self.link(&sink, &entry, None, Some(signal), None, None);
        } else if from != name && self.is_chain(from) && !self.is_lane(from) {
            let signal = self.exit_signal(from);
            let exit = self.exit_id(from);
            self.link(&exit, &entry, None, signal, None, None);
        } else {
            let video = self.is_video_track(from);
            let card = self.lane_card(from, true);
            self.add(card);
            // What the cable carries and which jack it leaves by: `fromPort` is both.
            let port = c.from_port.as_deref();
            let signal = if video {
                Signal::Video
            } else {
                match port {
                    Some("midi") => Signal::Midi,
                    Some("audio") => Signal::Audio,
                    _ => self
                        .graph
                        .track_signals
                        .as_ref()
                        .and_then(|t| t.get(from))
                        .copied()
                        .unwrap_or(Signal::Audio),
                }
            };
            self.link(&track_id(from), &entry, None, Some(signal), None, port);
        }
    }

    /// Where a chain goes: its exit cabled to each target, another chain's entry, a module or a sink.
    pub fn add_chain_outputs(&mut self, name: &str, c: &Chain) {
        for o in &c.out {
            // A destination may name a port (`crossfade#b`), split off before any test below or it invents a sink.
            let (to_node, to_port) = split_port(&o.to);
            // A cable into the mix is never drawn, the render's are.
            if is_sum(to_node) {
                self.feeds_sum
                    .insert(if to_node == MIX_OUT { MIX_OUT } else { RENDER_OUT }, true);
                if to_node != RENDER_OUT {
                    continue;
                }
            }
            // A host may ask for the devices to be left out entirely.
            if crate::build::is_device(to_node) && self.opts.draw_main_out == Some(false) {
                continue;
            }
            let to = if self.is_lane(to_node) {
                track_id(to_node)
            } else if self.is_chain(to_node) {
                self.entry_id(to_node)
            } else if self.used_modules.contains(to_node) {
                module_id(to_node)
            } else {
                self.add_sink(to_node, to_node == MAIN_OUT)
            };
            let label = (o.gain != 1.0).then(|| format!("{}%", crate::widget_value::js_round(o.gain * 100.0)));
            let (exit, signal) = (self.exit_id(name), self.exit_signal(name));
            self.link(&exit, &to, label, signal, to_port, o.from_port.as_deref());
        }
    }

    /// A rack's taps routed to subchains: each a cable from an output of its own, named after the tap.
    pub fn add_tap_feeds(&mut self) {
        let graph = self.graph;
        for f in graph.tap_feeds.iter().flatten() {
            let src_id = module_id(&f.node);
            let dst = if self.is_chain(&f.to) {
                self.entry_id(&f.to)
            } else {
                module_id(&f.to)
            };
            if !self.nodes.contains_key(&src_id) || !self.nodes.contains_key(&dst) {
                continue;
            }
            if let Some(src) = self.nodes.get_mut(&src_id) {
                let mut outs = src.outputs.clone().filter(|o| !o.is_empty()).unwrap_or_else(|| {
                    vec![Port {
                        name: "out".to_owned(),
                        kind: Signal::Audio,
                        mandatory: None,
                    }]
                });
                if !outs.iter().any(|o| o.name == f.tap) {
                    outs.push(Port {
                        name: f.tap.clone(),
                        kind: Signal::Audio,
                        mandatory: None,
                    });
                    src.outputs = Some(outs);
                }
            }
            self.link(
                &src_id,
                &dst,
                Some(f.tap.clone()),
                Some(Signal::Audio),
                None,
                Some(&f.tap),
            );
        }
    }

    /// Each sum and the wire out of it, when the file declares no master run.
    pub fn add_sums(&mut self) {
        for sum in [MIX_OUT, RENDER_OUT] {
            if !self.feeds_sum.get(sum).copied().unwrap_or(false) || self.is_chain(sum) {
                continue;
            }
            let card = self.add_sink(sum, false);
            if self.opts.draw_main_out == Some(false) {
                continue;
            }
            let video = sum == RENDER_OUT;
            let device = if video {
                self.add_sink(VIDEO_OUT, false)
            } else {
                self.add_sink(MAIN_OUT, true)
            };
            let signal = if video { Signal::Video } else { Signal::Audio };
            self.link(&card, &device, None, Some(signal), None, None);
        }
        // The render sink wears the frame's shape wherever it came from, and its two sockets whatever reached it so far.
        let render_id = sink_id(RENDER_OUT);
        let opts = self.opts;
        if let Some(sink) = self.nodes.get_mut(&render_id) {
            sink.inputs = Some(vec![
                Port {
                    name: "in".to_owned(),
                    kind: Signal::Video,
                    mandatory: None,
                },
                Port {
                    name: "audio".to_owned(),
                    kind: Signal::Audio,
                    mandatory: None,
                },
            ]);
            Build::render_card(opts, sink);
        }
    }

    /// Lanes no chain mentions, drawn as orphans: the missing cable is the mark.
    pub fn add_orphan_lanes(&mut self) {
        let graph = self.graph;
        for id in graph.tracks.iter().chain(graph.video_tracks.iter().flatten()) {
            if self.nodes.contains_key(&track_id(id)) {
                continue;
            }
            let card = self.lane_card(id, false);
            self.add(card);
        }
    }

    /// The curves nothing binds: one free card each, their ids carrying no binding.
    pub fn add_free_curves(&mut self) {
        let graph = self.graph;
        for (id, curve) in graph.curves.iter().flatten() {
            if self.bound_curves.contains(id) {
                continue;
            }
            let of = CurveOf {
                curve: id.clone(),
                module: None,
                param: None,
            };
            self.add(curve_card(&format!("crv:{id}"), id, curve, of));
        }
    }
}

/// A ref split into node and port; only the last `#` is the port, so the others are left alone.
pub fn split_port(r: &str) -> (&str, Option<&str>) {
    match r.rfind('#') {
        None | Some(0) => (r, None),
        Some(cut) if cut + 1 == r.len() => (r, None),
        Some(cut) => (&r[..cut], Some(&r[cut + 1..])),
    }
}
