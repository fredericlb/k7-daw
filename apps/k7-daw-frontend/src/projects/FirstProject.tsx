// FirstProject.tsx — the wizard a workspace with nothing in it opens on. A workspace is created
// empty on purpose (NewWorkspace.tsx): the archetype decides what the screen will LOOK like, so
// it is asked here, where the answer is about to be looked at, rather than in a dialog on a list.
// Not a dialog either — there is nothing behind it to dismiss it onto.
// See ../../docs/projects.md and apps/k7-daw-frontend/packages/project-shell/docs/launch.md.

import { projectApi } from "@studio/project-shell/project";
import type { ProjectShape } from "@studio/song-schema/yaml/projectSchema";
import { Button } from "@studio/ui/button";
import { FolderPlus, Loader2 } from "@studio/ui/icons";
import { useState } from "react";
import { m } from "../paraglide/messages.js";
import { NameField } from "./NameForm";
import { projectFolderName } from "./projectName";
import { ShapePicker } from "./ShapePicker";

export function FirstProject({
  workspace,
  /** Re-runs the gate's whole resolve, so what opens is what the file service now holds. */
  onCreated,
}: {
  workspace: string;
  onCreated: () => void;
}) {
  const [name, setName] = useState("");
  const [shape, setShape] = useState<ProjectShape>("arrangement");
  const [busy, setBusy] = useState(false);
  const [step, setStep] = useState("");
  const [error, setError] = useState("");
  const [owed, setOwed] = useState("");

  // One suffix is left, and this is it: every shape lives in a `.k7` folder.
  const { folder, error: badName } = projectFolderName(name, "k7");

  const submit = () => {
    if (!folder || busy) return;
    setBusy(true);
    setError("");
    void (async () => {
      setStep(m["project.creating"]({ folder }));
      const { project, missing = [] } = await projectApi.createProject(
        shape,
        folder,
      );
      // What the project still owes is said here, before it opens, rather than heard as silence once it has.
      if (!missing.length) return onCreated();
      setStep("");
      setOwed(
        m["project.missingModules"]({ project, modules: missing.join(", ") }),
      );
    })().catch((e: Error) => {
      setError(e.message);
      setBusy(false);
      setStep("");
    });
  };

  return (
    <div className="flex h-full items-center justify-center bg-background p-8 text-foreground">
      <div className="flex w-full max-w-2xl flex-col gap-4">
        <div className="flex flex-col gap-1">
          <h1 className="flex items-center gap-2 font-semibold text-lg">
            <FolderPlus className="h-5 w-5" />{" "}
            {m["project.firstTitle"]({ workspace })}
          </h1>
          <p className="type-secondary text-fg-2">{m["project.firstBody"]()}</p>
        </div>

        <ShapePicker shape={shape} onShape={setShape} busy={busy} />

        {/* What will actually be created, before confirming — as on the projects page. */}
        <NameField
          placeholder={m["project.namePlaceholder"]()}
          name={name}
          onName={setName}
          onSubmit={submit}
          busy={busy}
          step={step}
          error={error}
          badName={badName}
          preview={folder}
        />

        {owed && (
          <p className="rounded-xl border border-(--k7-warn-line) bg-(--k7-warn-fill) px-3 py-2 text-warn type-secondary">
            {owed}
          </p>
        )}

        <div className="flex justify-end">
          {owed ? (
            <Button onClick={onCreated}>{m["common.continue"]()}</Button>
          ) : (
            <Button disabled={busy || !folder} onClick={submit}>
              {busy && <Loader2 className="mr-1 size-5 animate-spin" />}
              {m["common.create"]()}
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
