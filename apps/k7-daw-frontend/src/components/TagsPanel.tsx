// TagsPanel.tsx — the open project's TAGS, the third tab of Tasks & history: a chip per tag with its ✕, and a field
// to add one. It writes nothing itself — each change is `project tag` / `project untag`, then a re-read of
// project.k7x, since the FILE is the truth (../docs/tasks.md).

import { projectTagsOf } from "@studio/k7-wasm/rules";
import { X } from "@studio/ui/icons";
import { Input } from "@studio/ui/input";
import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import { useRevision } from "../hooks/useRevision";
import { m } from "../paraglide/messages.js";
import { projectDir } from "../projectPaths";

export function TagsPanel({
  project,
  onRun,
}: {
  /** The open project as the workspace names it; none open, nothing to tag. */
  project: string | null;
  onRun: (line: string) => Promise<void>;
}) {
  const [tags, setTags] = useState<string[]>([]);
  const [tag, setTag] = useState("");
  const revision = useRevision();

  const refreshTags = useCallback(async () => {
    if (!project) return setTags([]);
    try {
      const dir = projectDir(project);
      const file = dir ? `${dir}/project.k7x` : "project.k7x";
      setTags(projectTagsOf((await api.read(file)).content));
    } catch {
      setTags([]);
    }
  }, [project]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: `revision` is the trigger, not an input.
  useEffect(() => {
    void refreshTags();
  }, [refreshTags, revision]);
  const runTag = async (line: string) => {
    await onRun(line);
    await refreshTags();
  };

  if (!project)
    return (
      <p className="px-2 py-4 text-center text-xs text-fg-2">
        {m["tags.noProject"]()}
      </p>
    );
  return (
    <form
      className="flex flex-wrap items-center gap-1 px-1 py-2"
      onSubmit={(e) => {
        e.preventDefault();
        const t = tag.trim();
        if (!t) return;
        setTag("");
        void runTag(`project tag ${t}`);
      }}
    >
      {tags.map((t) => (
        <span
          key={t}
          className="flex items-center gap-0.5 rounded bg-row py-0.5 pr-0.5 pl-1.5 text-fg-2 text-xs"
        >
          {t}
          <button
            type="button"
            className="rounded p-0.5 hover:bg-row hover:text-fg-1"
            onClick={() => void runTag(`project untag ${t}`)}
            title={m["tasks.untag"]({ tag: t })}
            aria-label={m["tasks.untag"]({ tag: t })}
          >
            <X className="h-3 w-3" />
          </button>
        </span>
      ))}
      <Input
        value={tag}
        onChange={(e) => setTag(e.target.value)}
        placeholder={m["tasks.tagPlaceholder"]()}
        title={m["tasks.tagHint"]()}
        aria-label={m["tasks.tagHint"]()}
        className="min-w-[6rem]"
      />
    </form>
  );
}
