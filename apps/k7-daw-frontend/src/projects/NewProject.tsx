// NewProject.tsx — "New project" on a workspace's card: the archetype, the name, and the derived folder shown BEFORE
// creating — as AddSong shows the tree before uploading, because here too the name decides
// everything. The card already knows its workspace. See ../../docs/projects.md.

import type { ProjectShape } from "@studio/song-schema/yaml/projectSchema";
import { Button } from "@studio/ui/button";
import { Plus } from "@studio/ui/icons";
import { useState } from "react";
import { m } from "../paraglide/messages.js";
import { NameFormDialog, useNameSubmit } from "./NameForm";
import { projectFolderName } from "./projectName";
import { ShapePicker } from "./ShapePicker";

export function NewProject({
  /** Why it is impossible, or "" — a file service not answering is one. */
  blocked,
  workspace,
  onCreate,
}: {
  blocked: string;
  /** Where it lands. */
  workspace: string;
  /** Returns when done; `onStep` narrates the wait, cloning taking tens of seconds and a silent screen looking frozen. */
  onCreate: (
    workspace: string,
    folder: string,
    onStep: (s: string) => void,
    shape: ProjectShape,
  ) => Promise<void>;
}) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [shape, setShape] = useState<ProjectShape>("arrangement");
  const { busy, step, error, run } = useNameSubmit();

  // One suffix is left, and this is it: every shape lives in a `.k7` folder.
  const { folder, error: badName } = projectFolderName(name, "k7");

  const submit = () => {
    if (!folder) return;
    run(
      (onStep) => onCreate(workspace, folder, onStep, shape),
      () => {
        setOpen(false);
        setName("");
      },
    );
  };

  return (
    <>
      <Button
        size="sm"
        variant="glass"
        disabled={Boolean(blocked)}
        title={blocked || m["project.newHint"]()}
        aria-label={m["project.newIn"]({ workspace })}
        onClick={() => setOpen(true)}
      >
        <Plus className="size-5" />
        {/* On a phone the plus is enough: the name needs the width more. */}
        <span className="max-sm:hidden">{m["project.new"]()}</span>
      </Button>

      <NameFormDialog
        open={open}
        onClose={() => setOpen(false)}
        title={m["project.newTitle"]()}
        wide
        description={m["project.newBody"]()}
        before={
          // The archetype decides the TEMPLATE written in the folder; the suffix is not a choice, there being only one.
          <ShapePicker shape={shape} onShape={setShape} busy={busy} />
        }
        placeholder={m["project.namePlaceholder"]()}
        name={name}
        onName={setName}
        onSubmit={submit}
        busy={busy}
        step={step}
        error={error}
        badName={badName}
        preview={folder ? `${workspace}/${folder}` : folder}
        submit={m["common.create"]()}
        canSubmit={Boolean(folder)}
      />
    </>
  );
}
