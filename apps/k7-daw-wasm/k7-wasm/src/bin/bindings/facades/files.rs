// facades/files.rs — the wrappers of the families over names, paths and repositories: address, intake, kit, paths, git,
// and the editors' clipboard and curve draft. A reader would get wrong that an op missing here has no caller: it is wrapped by hand.

use crate::facade::{Facade, facade, op, whole};
use k7_wasm::ops::address::*;
use k7_wasm::ops::clipboard::*;
use k7_wasm::ops::draft::*;
use k7_wasm::ops::git::*;
use k7_wasm::ops::intake::*;
use k7_wasm::ops::paths::*;
use ts_rs::TS;

#[rustfmt::skip]
pub fn all() -> Vec<Facade> {
    vec![
        facade("address", vec![AddressRequest::name()], vec![
            op::<AddressAt>("parseLaunch", "address.parse_launch", "Launch | null", "Read `/<repo>/<project…>[/<view…>]` under `base`, or null — docs/addresses.md."),
            op::<AddressAt>("projectBasename", "address.basename", "string",
                "The router basename: a slice of `pathname` up to the project's end, `base` when it names none."),
            op::<AddressAt>("parseExternal", "address.parse_external", "string | null", "The surface an address opens on its own, or null."),
            op::<AddressAt>("parseLanSurface", "address.parse_lan_surface", "LanSurface | null", "A `@lan` surface address read back, or null for anything else."),
            op::<AddressAt>("isHomePage", "address.is_home_page", "boolean", "Whether an address is a page that hangs off the home, or below one."),
            op::<AddressProject>("launchPath", "address.launch_path", "string", "`/<repo>/<project>`, each segment encoded; the project `.` adds none."),
            op::<AddressExternal>("externalOf", "address.external_of", "string", "A surface hung off a project URL somebody else built."),
            op::<AddressLan>("lanSurfacePath", "address.lan_surface_path", "string", "The path of a surface of another device."),
            op::<AddressLocal>("localProjectPath", "address.local_project_path", "string", "The path of a project in a workspace with no repository."),
            op::<AddressName>("localTarget", "address.local_target", "LocalTarget | null", "Split the project half of a `@local` address, checked not trusted."),
            op::<AddressChain>("benchProject", "address.bench_project", "string", "The project half of a bench address, seeded with `chain` when one is given."),
            whole::<AddressProject>("projectClaimKey", "address.claim_key", "string", "How a project is named in an engine claim."),
            op::<AddressName>("claimIsBench", "address.claim_is_bench", "boolean", "Whether a claimed address plays in the bench's engine session."),
            op::<AddressName>("isProjectName", "address.is_project", "boolean", "Whether a folder NAME is a project's."),
            op::<AddressName>("projectLabel", "address.project_label", "string", "The folder name with its suffix taken off."),
            op::<AddressName>("isDroppableProject", "address.is_droppable", "boolean", "Whether a file NAME is a project a drop may carry."),
        ]),
        facade("intake", vec![IntakeRequest::name()], vec![
            op::<IntakeNameArgs>("droppedKind", "intake.kind", "DropKind | null",
                "Which kind of material a dropped file is, or null: the EXTENSION decides, as it does for the compiler."),
            op::<IntakeUploadArgs>("clipDecl", "intake.decl", "string", "The declaration a dropped file gets: `source` and nothing else, the window being the whole file."),
            op::<IntakeTextArgs>("declaredSource", "intake.declared_source", "string | null", "The `source` a declaration names, or null when it has none or does not read."),
            op::<IntakeMimeArgs>("extForMime", "intake.ext_for_mime", "string", "The extension a container lands under: the compiler reads a clip's KIND off it."),
            op::<IntakeTakeIdArgs>("takeId", "intake.take_id", "string",
                "A take's clip id: the lane it was recorded on and when, reduced to what a path and a k7x key hold."),
        ]),
        facade("kit", vec![IntakeRequest::name()], vec![
            op::<KitFileNameArgs>("defaultSymbol", "kit.default_symbol", "string",
                "A pad's default symbol: the file's stem, upper-cased and squeezed to the characters a symbol is addressed by."),
            op::<KitBaseModulesArgs>("rackModuleRef", "kit.module_ref", "string | null",
                "Which library's sampler a kit extends: `@k7-core`'s first, else any library's; null when none holds one."),
            op::<KitUploadDirArgs>("kitUploadDir", "kit.upload_dir", "string", "The folder the samples are uploaded into, workspace-relative."),
            op::<KitSampleRefArgs>("kitSampleRef", "kit.sample_ref", "string",
                "The name a tap writes for one of them: `@project/<kit>/<file>` or `@<library>/<kit>/<file>`."),
            op::<KitNodeArgs>("buildKitNode", "kit.node", "string", "The KIT node the builder writes, as k7x: a node of the sampler, one tap per pad, one per line."),
        ]),
        facade("paths", vec![PathsRequest::name()], vec![
            op::<PathsPathArgs>("rackKeyOf", "paths.rack_key", "string | null",
                "The chain-`use` key a file answers to in `hints.racks`: a module's folder name or a NODE's stem."),
            op::<PathsEntityArgs>("entityFile", "paths.entity_file", "string | null", "The file in `files` that IS `<folder>/<id>`, or null: a lookup and not a search."),
            op::<PathsPathArgs>("fileStem", "paths.stem", "string", "A file's id: its name without the folder or the extension."),
            op::<PathsDirArgs>("projectDir", "paths.project_dir", "string",
                "A project folder, slashed and without a trailing `/`; \"\" when there is none or it is the workspace itself."),
            op::<PathsProjectArgs>("toProjectRelative", "paths.relative", "string",
                "A workspace-relative path made relative to the project folder, or merely slashed when it lies outside it."),
            op::<PathsProjectArgs>("inActiveProject", "paths.in_project", "boolean", "Is this path inside the project the ENGINE holds?"),
            op::<PathsPathArgs>("moduleFileOf", "paths.module_file_of", "string", "The generic module file beside `path`: a node's sibling."),
            op::<PathsPathArgs>("baseName", "paths.base", "string", "The last segment of a path: the name a rename starts from."),
            op::<PathsHeadArgs>("headTokens", "paths.head_tokens", "string[]", "A line split into its leading `n` tokens and the rest, so a name with spaces survives."),
            op::<PathsGutterArgs>("resolveGutterSource", "paths.gutter", "string",
                "A gutter source where the COMPILER puts it: a rack's sits beside its file, a clip's at the project root."),
            op::<PathsTextArgs>("isAudioClipFile", "paths.audio_clip", "boolean",
                "A clip file's kind: audio unless its source ends in a picture extension; a half-typed file answers."),
            op::<PathsTextArgs>("findUseRack", "paths.use_rack", "string | null",
                "The rack module a sequence names via `use_rack`, so a slice rack can dock its source waveform."),
            op::<PathsTreeArgs>("textFiles", "paths.text_files", "string[]", "The tree flattened to openable text paths: binaries can't be shown."),
            op::<PathsTreeArgs>("allEntries", "paths.entries", "TreeEntry[]",
                "EVERY path the tree holds, folders included and binaries kept: what a `file …` command may name."),
        ]),
        facade("git", vec![GitRequest::name()], vec![
            op::<StateArgs>("barView", "git.bar", "BarView", "What the workspace bar offers on a state; a missing state, or a workspace with no repository, has no bar."),
            op::<StateArgs>("menuView", "git.menu", "MenuView", "What the workspace menu offers: connect with no remote, else the remote's name."),
            op::<UrlArgs>("remoteName", "git.remoteName", "string", "A remote as a person reads it: the host and the repository, never the user or the scheme."),
            op::<EntriesArgs>("commitView", "git.commitView", "CommitView", "What a Save takes of a status: the files but the ones too large for a host."),
            op::<UrlArgs>("sshHost", "git.sshHost", "SshHost | null", "The host and port an ssh address names, or null."),
            op::<UrlArgs>("isWebAddress", "git.isWebAddress", "boolean", "Whether the address is a web one (`https`, `http`, `git`), which k7 never takes."),
            op::<UrlArgs>("nameFromUrl", "git.nameFromUrl", "string", "The repository's own name in an address, or \"\"."),
            op::<CloneNameArgs>("cloneName", "git.cloneName", "string", "The name a clone starts with: its own, or the next free `name 2`."),
            op::<VersionArgs>("shortVersion", "git.shortVersion", "string", "A server's version without its build metadata."),
            op::<OfferedArgs>("offered", "git.offered", "MergeChoice[]", "The choices a conflict offers."),
            op::<ProgressArgs>("remaining", "git.remaining", "number", "Conflicts still without a choice, given the ids that have one."),
            op::<ProgressArgs>("canFinish", "git.canFinish", "boolean", "Whether \"Finish merge\" is allowed: every conflict has a choice."),
            op::<ArrangeArgs>("arrange", "git.arrange", "Arranged", "The picker's two lists: k7 workspaces first, each most recently pushed first."),
            op::<AgoArgs>("pushedAgo", "git.pushedAgo", "Ago", "How long ago a push was, in the largest whole unit."),
            op::<RoamArgs>("roamOffered", "git.roamOffered", "RoamEntry[]", "The roam entries to offer, in the list's order: the caller sorts them by name."),
        ]),
        facade("clipboard", vec![ClipboardRequest::name()], vec![
            op::<ClipPasteNotesArgs>("pasteNotes", "clipboard.paste_notes", "PastedNote[]", "…and back out at a caret: anything pushed off the keyboard is DROPPED, not clamped."),
            op::<ClipPasteEventsArgs>("pasteEvents", "clipboard.paste_events", "PastedEvent[]", "…and back out onto real lanes: as many as the paste can reach, the rest dropped."),
            op::<ClipDroppedArgs>("droppedCount", "clipboard.dropped", "number",
                "How many of a paste's items would be dropped for not fitting: what the status line reports."),
        ]),
        facade("draft", vec![DraftRequest::name()], vec![
            op::<DraftKindArgs>("shapesFor", "draft.shapes_for", "DraftShape[]",
                "The shapes a param of this kind can follow: a number every curve but on/off, a boolean on/off alone."),
            op::<DraftOfArgs>("draftOf", "draft.of", "CurveDraft", "The draft a curve opens on: its own fields over the defaults, or the defaults alone for a new one."),
            op::<DraftArgs>("fieldsOf", "draft.fields", "Record<string, unknown>",
                "The fields `curve set` writes: the spec, the polarity only when not the default, the conditions when there are any."),
            op::<DraftArgs>("conditionsProblem", "draft.conditions_problem", "string | null",
                "Why the draft cannot be saved yet, or null: the first of its conditions that cannot be written."),
            op::<DraftConditionArgs>("conditionProblem", "draft.condition_problem", "string | null",
                "Why one condition cannot be written, or null: a moment that is not a timecode, an event with no name."),
            op::<DraftArgs>("durationProblem", "draft.duration_problem", "string | null", "Why the duration cannot be saved, or null."),
            op::<DraftWithPointArgs>("withPoint", "draft.with_point", "DraftPoint[]",
                "The points in time order, one per moment: the later write wins, as `curve point add` sets rather than duplicates."),
        ]),
    ]
}
