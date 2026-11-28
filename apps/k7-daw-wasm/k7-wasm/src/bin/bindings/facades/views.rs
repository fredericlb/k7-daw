// facades/views.rs — the wrappers of the desk's and the module builder's panel families.
// A reader would get wrong that an op missing here has no caller: one whose answer the page types by hand is wrapped by hand.

use crate::facade::{Facade, bare, facade, op};
use k7_wasm::ops::desk as d;
use k7_wasm::ops::panel as p;
use ts_rs::TS;

/// The panel as the page's graph model types it.
const UI: &str = "import(\"@studio/graph-model\").WebModuleUI";

#[rustfmt::skip]
pub fn all() -> Vec<Facade> {
    vec![
        facade("desk", vec![d::DeskRequest::name()], vec![
            op::<d::PhaseArgs>("beatPhase", "desk.beat_phase", "BeatGrid",
                "Where the beat grid starts: a circular mean of every onset, not the first transient (docs/turntables/bpm.md)."),
            op::<d::AlignArgs>("alignSeek", "desk.align_seek", "number | null",
                "Where to seek the slave so its beats land on the master's, or null when a grid is unknown."),
            op::<d::CorrectedArgs>("corrected", "desk.corrected", "DeckTempo", "The analysis with a corrected tempo laid over it: the correction belongs to the record."),
            op::<d::TempoArgs>("beatSeconds", "desk.beat_seconds", "number", "One beat in source seconds, at the record's own tempo."),
            op::<d::TempoArgs>("canLoop", "desk.can_loop", "boolean", "Can this deck loop at all? It takes a measured tempo and a length."),
            op::<d::SnapArgs>("snapToBeat", "desk.snap_to_beat", "number", "The nearest beat to `at`; `at` itself when the deck has no grid."),
            op::<d::ResizedArgs>("resized", "desk.resized", "number", "A length halved or doubled, held between the two ends."),
            op::<d::BetweenArgs>("beatsBetween", "desk.beats_between", "number", "How many beats long a manual IN to OUT is."),
            op::<d::EdgesArgs>("loopEdges", "desk.loop_edges", "LoopEdges | null", "The edges to send, or null when they would not be a loop."),
            op::<d::JumpArgs>("beatJump", "desk.beat_jump", "number", "Where a jump of `beats` lands, held inside the record."),
            op::<d::SpanArgs>("loopSpan", "desk.loop_span", "LoopSpan | null", "The loop in source seconds, for drawing it."),
            op::<d::PlattersArgs>("plattersOf", "desk.platters_of", "string[]", "Every platter a deck drives: four when the record is separated, else its own."),
            op::<d::RateArgs>("platterRate", "desk.platter_rate", "number", "How fast a platter runs, as a multiple of the record's own clock; 0 when stopped."),
            op::<d::KeyArgs>("platterKey", "desk.platter_key", "number", "What a platter's `Key` should be: 1 holds the pitch; a hand on the record wins."),
            op::<d::MixArgs>("crossfadeLevels", "desk.crossfade_levels", "LevelDb[]",
                "Every deck's level at crossfade `mix`, as the engine's `set-plugin-param` takes it."),
            op::<d::RebuildArgs>("worthARebuild", "desk.worth_a_rebuild", "boolean",
                "Has any deck's tempo moved far enough since the last send to be worth another document?"),
            op::<d::MappingArgs>("unmappedControls", "desk.unmapped_controls", "MixControl[]",
                "The required controls a `mapping` does not name; the optional ones are left alone."),
            op::<d::PublishArgs>("offlinePublish", "desk.offline_publish", "Published[]",
                "What every tempo module with no plugin should read, for a host to lend to the graph."),
            op::<d::ValueForArgs>("valueFor", "desk.value_for", "number",
                "What to send for a control at `value`: normalised by the control's range, expanded by the widget's."),
        ]),
        facade("panel", vec![p::PanelRequest::name()], vec![
            op::<p::TwoWidgets>("clash", "panel.clash", "boolean",
                "Whether two widgets COLLIDE: they overlap, unless one is a group wholly framing the other (never two groups)."),
            op::<p::UiArgs>("twinIssues", "panel.twin_issues", "string[]",
                "Every way a panel's two shapes break the rules: sizes, the twin's own geometry, and controls one shape lacks."),
            op::<p::UiArgs>("reflowNarrow", "panel.reflow_narrow", "NonNullable<import(\"@studio/graph-model\").WebModuleUI[\"vertical\"]>",
                "A first VERTICAL layout from the panel: every control at its size, packed into 4 columns in reading order."),
            op::<p::UiArgs>("alignTabs", "panel.align_tabs", UI, "The vertical layout's tabs brought in line with the panel's."),
            op::<p::CornerArgs>("cornerAnchor", "panel.corner_anchor", "Cell", "The cell a corner's resize keeps still: the widget's OPPOSITE corner cell."),
            op::<p::WriteAtArgs>("writeWidgetsAt", "panel.write_widgets_at", UI, "`ui` with the map at `at` replaced; the other tabs are untouched."),
            op::<p::UiArgs>("addTab", "panel.add_tab", "Tabbed", "A tab added at the end; the FIRST one added makes two."),
            op::<p::IdArgs>("clipIds", "panel.clip_ids", "string[]", "The ids a cut or a copy of `id` takes: a group brings what it frames, first in the list."),
            op::<p::IdArgs>("copyWidgets", "panel.copy_widgets", "Clip | null",
                "A copy of `id` (and, for a group, its members), moved so the block starts at cell 0,0."),
            op::<p::NameArgs>("familyOf", "panel.family_of", "string", "Which family a parameter's NAME suggests."),
            op::<p::NameArgs>("isPlumbing", "panel.is_plumbing", "boolean", "Whether a parameter is plumbing the proposal skips."),
            op::<p::NameArgs>("isBlend", "panel.is_blend", "boolean", "Whether a parameter is the module's OWN blend: the test is the FAMILY, not the word."),
            op::<p::ParamsArgs>("groupParams", "panel.group_params", "Page[]", "The pages that got something, never a `drop` one, in the table's own order."),
            op::<p::ParamsArgs>("autoOrganize", "panel.auto_organize", UI, "A whole panel proposed from a parameter list, in both its shapes."),
            op::<p::ContentArgs>("readParams", "panel.read_params", "Record<string, string>",
                "The `params(…)` block as name to value as TEXT, never numbers: `1.0` stays `1.0`."),
            op::<p::ContentArgs>("paramValues", "panel.param_values", "Record<string, number>", "The same values as numbers; anything that is not one is left out."),
            op::<p::AiPromptArgs>("aiUserPrompt", "panel.ai_user_prompt", "string", "The per-module half of the prompt: everything that changes between two requests."),
            bare("aiDesignBrief", "panel.ai_brief", "string",
                "The builder, the widgets, the screen contract and the design brief: what any answer, API or files, is designed by."),
            op::<p::UiTextArgs>("hasLayout", "panel.has_layout", "boolean",
                "Whether a panel file lays anything out: the question that decides whether there is a layout to update."),
        ]),
    ]
}
