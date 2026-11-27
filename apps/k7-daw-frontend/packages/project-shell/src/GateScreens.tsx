// GateScreens.tsx — the project gate's screens other than the app itself: the waits, the
// refusals and the errors. Each draws one phase and decides nothing. See docs/launch.md.

import { Button } from "@studio/ui/button";
import {
  FolderGit2,
  FolderSearch,
  LibraryBig,
  ServerCrash,
  UserRoundX,
} from "@studio/ui/icons";
import { Spinner } from "@studio/ui/spinner";
import type { ReactNode } from "react";
import type { ProjectGateStrings } from "./gateTypes.ts";

/** What a workspace with no project says when the app draws no wizard: nothing is wrong, it is empty. */
const EMPTY_WORKSPACE =
  "This workspace holds no project yet — add one from the projects page.";

/** The one layout every gate screen shares: an icon over a centred column, `wide` for a bar to span. */
function GateCard({
  icon,
  wide,
  children,
}: {
  icon: ReactNode;
  wide?: boolean;
  children: ReactNode;
}) {
  return (
    <div className="flex h-full items-center justify-center bg-ground p-8 text-fg-1">
      <div
        className={`flex ${wide ? "w-full " : ""}max-w-md flex-col items-center gap-4 text-center`}
      >
        {icon}
        {children}
      </div>
    </div>
  );
}

/** The raw message behind a screen, folded away: the sentence above it is what to do, this is what to report. */
function Detail({ label, text }: { label: string; text: string }) {
  return (
    <details className="w-full max-w-full text-left text-xs text-fg-2">
      <summary className="cursor-pointer select-none">{label}</summary>
      <code className="mt-1 block max-w-full overflow-x-auto whitespace-pre-wrap rounded bg-(--k7-input-fill) px-2 py-1">
        {text}
      </code>
    </details>
  );
}

function Back({ label, onClick }: { label: string; onClick: () => void }) {
  return <Button onClick={onClick}>{label}</Button>;
}

/** "Somebody else opened this project" — one screen, and no Retry. See docs/launch.md. */
export function ProjectTakenOver({
  heldBy,
  title,
  body,
  back,
  onBack,
}: {
  /** Who has it now — empty when the registry could not say, in which case body carries it. */
  heldBy: string;
  title: string;
  body: (heldBy: string) => string;
  back: string;
  onBack: () => void;
}) {
  return (
    <GateCard icon={<UserRoundX className="h-10 w-10 text-warn" />}>
      <h1 className="text-lg font-semibold">{title}</h1>
      <p className="text-sm text-fg-2">{body(heldBy)}</p>
      <Back label={back} onClick={onBack} />
    </GateCard>
  );
}

/** A wait with no measure — the bench's catalogue being fetched. */
export function CloningScreen({
  title,
  detail,
}: {
  title: string;
  detail: string;
}) {
  return (
    <GateCard wide icon={<FolderGit2 className="h-10 w-10 text-fg-2" />}>
      <h1 className="text-lg font-semibold">{title}</h1>
      <p className="text-sm text-fg-2">{detail}</p>
      {/* No measure to fill, so the bar only says that something is moving. */}
      <div className="h-1.5 w-full overflow-hidden rounded bg-(--k7-progress-track)">
        <div className="h-full w-1/3 animate-pulse bg-(--k7-progress-fill)" />
      </div>
    </GateCard>
  );
}

/** Repository fine, machine fine, project not in it: picking again is the only move, so no Retry. */
export function MissingScreen({
  strings,
  hint,
  detail,
  onBack,
}: {
  strings: ProjectGateStrings;
  /** What to do instead, when the gate knows better than the generic body. */
  hint?: string;
  /** The service's own sentence, folded away under `strings.details`. */
  detail?: string;
  onBack: () => void;
}) {
  return (
    <GateCard icon={<FolderSearch className="h-10 w-10 text-warn" />}>
      <h1 className="text-lg font-semibold">{strings.missingTitle}</h1>
      <p className="text-sm text-fg-2">{hint ?? strings.missingBody}</p>
      {detail && <Detail label={strings.details ?? "Details"} text={detail} />}
      <Back label={strings.back} onClick={onBack} />
    </GateCard>
  );
}

/** No library repository: not an error, and re-asking gives the same answer until it is set. */
export function NoLibrariesScreen({
  strings,
  addOne,
  onBack,
}: {
  strings: ProjectGateStrings;
  /** The way to ADD one, right here: a refusal that sends you elsewhere to fix it is a dead end. */
  addOne: ReactNode;
  onBack: () => void;
}) {
  return (
    <GateCard icon={<LibraryBig className="h-10 w-10 text-warn" />}>
      <h1 className="text-lg font-semibold">{strings.librariesTitle}</h1>
      <p className="text-sm text-fg-2">{strings.librariesBody}</p>
      {addOne && <div className="w-full text-left">{addOne}</div>}
      <Back label={strings.back} onClick={onBack} />
    </GateCard>
  );
}

/** Nothing in the workspace, and no wizard from the app: a sentence and the way back. */
export function EmptyWorkspaceScreen({
  strings,
  onBack,
}: {
  strings: ProjectGateStrings;
  onBack: () => void;
}) {
  return (
    <GateCard icon={<FolderSearch className="h-10 w-10 text-warn" />}>
      <p className="text-fg-2 text-sm">
        {strings.emptyWorkspace ?? EMPTY_WORKSPACE}
      </p>
      <Back label={strings.back} onClick={onBack} />
    </GateCard>
  );
}

/** The file service did not answer: one screen, the error as its detail. */
export function ErrorScreen({
  strings,
  message,
  lines,
  onRetry,
  onBack,
}: {
  strings: ProjectGateStrings;
  message: string;
  lines?: string[];
  onRetry: () => void;
  onBack: () => void;
}) {
  return (
    <GateCard icon={<ServerCrash className="h-10 w-10 text-warn" />}>
      <h1 className="text-lg font-semibold">{strings.unreachableTitle}</h1>
      <p className="text-sm text-fg-2">{strings.bothBody}</p>
      <Detail
        label={strings.details ?? "Details"}
        text={lines?.length ? lines.join("\n") : message}
      />
      <div className="flex items-center gap-2">
        <Button variant="glass" onClick={onRetry}>
          {strings.retry}
        </Button>
        <Button variant="ghost" onClick={onBack}>
          {strings.back}
        </Button>
      </div>
    </GateCard>
  );
}

/** Resolving: a spinner that says so to a screen reader. */
export function GateSpinner({ label }: { label: string }) {
  return (
    <div
      role="status"
      aria-label={label}
      className="flex h-full items-center justify-center bg-ground text-fg-2"
    >
      <Spinner inherit />
    </div>
  );
}
