// VideoExportRow.tsx — the share panel's video export: MP4 first when this browser can encode it,
// since that is what a phone and every social site play, WebM otherwise; 720p or 1080p. It only
// offers the `project export video …` line the panel runs once a folder is chosen. See ../../docs/shell.md.

import { Chip } from "@studio/ui/chip";
import { Download } from "@studio/ui/icons";
import { RowButton } from "@studio/ui/row-group";
import { probeExportSupport } from "@studio/ui-eyes/exportSupport";
import { useEffect, useState } from "react";
import { m } from "../paraglide/messages.js";

type Format = "mp4" | "webm";
type Size = "720p" | "1080p";

/** The frame each size is probed at: the encoder is asked about what it will actually be given. */
const PROBE: Record<Size, [number, number]> = {
  "720p": [1280, 720],
  "1080p": [1920, 1080],
};

export function VideoExportRow({
  busy,
  chosen,
  onChoose,
}: {
  busy: boolean;
  /** The line the panel has chosen, to mark this row when it is its own. */
  chosen: string;
  onChoose: (command: string) => void;
}) {
  const [size, setSize] = useState<Size>("1080p");
  // Null while asked; MP4 is offered only once the encoder says yes.
  const [mp4, setMp4] = useState<boolean | null>(null);
  const [format, setFormat] = useState<Format | null>(null);

  useEffect(() => {
    let live = true;
    const [w, h] = PROBE[size];
    void probeExportSupport("mp4", w, h)
      .then((s) => live && setMp4(s.ok))
      .catch(() => live && setMp4(false));
    return () => {
      live = false;
    };
  }, [size]);

  // MP4 by default where it can be made; a choice made by hand is kept.
  const fmt: Format = format ?? (mp4 ? "mp4" : "webm");
  const lineOf = (f: Format, z: Size) => `project export video ${f} size=${z}`;
  const line = lineOf(fmt, size);
  // A pill moved on a chosen row moves the chosen line with it.
  const retarget = (f: Format, z: Size) => {
    if (chosen.startsWith("project export video")) onChoose(lineOf(f, z));
  };
  return (
    <div>
      <RowButton
        tall
        icon={<Download />}
        detail={m["share.detailVideo"]()}
        selected={chosen === line}
        disabled={busy || mp4 === null}
        onClick={() => onChoose(line)}
      >
        {m["share.exportVideo"]({ format: fmt.toUpperCase(), size })}
      </RowButton>
      <div className="flex flex-wrap items-center gap-2 px-3.5 pb-3">
        {(mp4 ? (["mp4", "webm"] as const) : (["webm"] as const)).map((f) => (
          <Chip
            key={f}
            variant="filter"
            selected={fmt === f}
            onClick={() => {
              setFormat(f);
              retarget(f, size);
            }}
          >
            {f.toUpperCase()}
          </Chip>
        ))}
        {(["720p", "1080p"] as const).map((z) => (
          <Chip
            key={z}
            variant="filter"
            selected={size === z}
            onClick={() => {
              setSize(z);
              retarget(fmt, z);
            }}
          >
            {z}
          </Chip>
        ))}
      </div>
      {mp4 === false && (
        <p className="type-status px-3.5 pb-3 text-fg-3">
          {m["share.noMp4"]()}
        </p>
      )}
    </div>
  );
}
