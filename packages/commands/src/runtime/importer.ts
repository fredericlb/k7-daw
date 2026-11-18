// importer.ts — a foreign project imported where the files are: translated in the Worker, samples uploaded, texts landed by `/changes`.
// What a reader gets wrong: the samples go FIRST, into the folder the set then creates; a set that does not land takes that folder away.

import { posix } from "node:path";
import { useVirtualDisk } from "@studio/song/injectedRoots";
import type { SampleRef } from "@studio/translator/ableton";
import { emitFolder } from "@studio/translator/emit";
import { withBinarySink } from "@studio/translator/emitNaming";
import {
  checkDroppable,
  checkSize,
  folderName,
  IMPORTABLE,
  importLog,
  MAX_IMPORT_BYTES,
  translate,
} from "@studio/translator/import";
import { LibraryCache, loadProject, relativeTo } from "./bundle.ts";
import { installGlobals } from "./globals.ts";
import type { ChangeOp, Remote } from "./remote.ts";
import { setVfs } from "./shims/current.ts";

/** What an import answers: the new project's folder and the lines the dialog shows. */
export interface ImportResult {
  project: string;
  log: string[];
}

export interface Importer {
  /** A file whose bytes the caller holds (a drop on the home page): the self-contained formats only. */
  importFile(
    data: Blob | Uint8Array | ArrayBuffer,
    name: string,
    as?: string,
  ): Promise<ImportResult>;
  /** A file already in the workspace, clicked in the tree: a `.als` reads the samples beside it. */
  importPath(rel: string, as?: string): Promise<ImportResult>;
}

export interface ImporterOptions {
  remote: Remote;
  libraries?: LibraryCache;
  /** The inflate ceiling of an imported archive; the translator's default when unset. */
  inflateLimit?: number;
}

/** Where the engine's plugin list is laid for the emitter, which reads a scan FILE. */
const SCAN = "/tmp/k7-import/exts.json";

const enc = new TextEncoder();

/** The engine's scan as the file the emitter reads; an engine-less server answers none, which reads as "unknown". */
async function scanText(remote: Remote): Promise<string> {
  const a = await remote.host("plugins", null).catch(() => undefined);
  const plugins = a?.status === 200 ? (a.body.plugins as unknown) : [];
  return JSON.stringify({
    entries: [{ plugins: Array.isArray(plugins) ? plugins : [] }],
  });
}

const sizeOf = (d: Blob | Uint8Array | ArrayBuffer): number =>
  d instanceof Blob ? d.size : d.byteLength;

async function bytesOf(
  d: Blob | Uint8Array | ArrayBuffer,
): Promise<Uint8Array> {
  if (d instanceof Blob) return new Uint8Array(await d.arrayBuffer());
  return d instanceof Uint8Array ? d : new Uint8Array(d);
}

/** One `.als` sample reference as a workspace path below the file's folder; null when it climbs out of the workspace. */
function sampleCandidate(alsRel: string, relative: string): string | null {
  const at = posix.join(
    posix.dirname(alsRel.replace(/\\/g, "/")),
    relative.replace(/\\/g, "/"),
  );
  return at === ".." || at.startsWith("../") || at.startsWith("/") ? null : at;
}

interface Source {
  source: string;
  file: string;
  bytes: Uint8Array;
  as?: string;
  /** The `.als`'s own path in the workspace, when it sits there: its samples are read beside it. */
  alsRel?: string;
}

export function createImporter(o: ImporterOptions): Importer {
  installGlobals();
  useVirtualDisk();
  const libraries = o.libraries ?? new LibraryCache();
  const { remote } = o;

  /** The samples a `.als` names relative to itself, fetched from the workspace: two passes, since a parse reads them synchronously. */
  async function samplesOf(s: Source): Promise<{
    read: (ref: SampleRef) => Uint8Array | null;
    outside: Set<string>;
  }> {
    const outside = new Set<string>();
    const wanted = new Map<string, string>();
    const alsRel = s.alsRel as string;
    const ask = (ref: SampleRef): string | null => {
      if (!ref.relative) {
        // Counted by FILE, not by reference: nine clips off five files is five things to go and collect.
        outside.add(ref.absolute ?? ref.name);
        return null;
      }
      const at = sampleCandidate(alsRel, ref.relative);
      if (at === null) outside.add(ref.relative);
      return at;
    };
    translate({
      source: s.source,
      file: s.file,
      bytes: s.bytes,
      readSample: (ref) => {
        const at = ask(ref);
        if (at !== null) wanted.set(at, ref.name);
        return at === null ? null : new Uint8Array(1);
      },
    });
    const held = new Map<string, Uint8Array>();
    let total = 0;
    for (const at of wanted.keys()) {
      const bytes = await remote.bytes?.(at);
      if (!bytes) continue;
      total += bytes.byteLength;
      if (total > MAX_IMPORT_BYTES)
        throw new Error(
          "the samples this set names weigh more than an import takes",
        );
      held.set(at, bytes);
    }
    outside.clear();
    return {
      read: (ref) => {
        const at = ask(ref);
        return at === null ? null : (held.get(at) ?? null);
      },
      outside,
    };
  }

  async function run(s: Source): Promise<ImportResult> {
    if (!remote.upload)
      throw new Error(
        "this server takes no uploads, so it cannot take an import",
      );
    const sample =
      s.alsRel !== undefined && s.file.toLowerCase().endsWith(".als")
        ? await samplesOf(s)
        : undefined;
    const translation = translate({
      source: s.source,
      file: s.file,
      bytes: s.bytes,
      readSample: sample?.read,
      inflateLimit: o.inflateLimit,
    });
    const l = await loadProject(remote, "", libraries, false);
    const { vfs, workspaceRoot: root } = l;
    setVfs(vfs);
    const names = new Set(vfs.readdir(root).map(([n]) => n));
    const project = folderName(s.file, s.as, (n) => names.has(n));
    const scan = await scanText(remote);
    vfs.load(SCAN, { text: scan, len: enc.encode(scan).length });
    const binaries = new Map<string, Uint8Array>();
    const emitted = withBinarySink(
      (path, bytes) => {
        binaries.set(path, bytes);
        vfs.load(path, { len: bytes.byteLength, binary: true });
      },
      () =>
        emitFolder(
          `${root}/${project}`,
          translation.song,
          translation.files,
          SCAN,
        ),
    );
    const ops: ChangeOp[] = [];
    for (const op of vfs.staged()) {
      const file = relativeTo(root, op.file);
      if (op.kind !== "text" || file === null)
        throw new Error(
          `the importer staged '${op.kind}' of a path it cannot land`,
        );
      ops.push({ kind: "text", file, text: op.text, was: op.was });
    }
    const written = new Set(ops.map((x) => x.file));
    for (const [path, was] of vfs.reads) {
      const file = relativeTo(root, path);
      if (file !== null && !written.has(file))
        ops.push({ kind: "read", file, was });
    }
    // The shared folders' uploads outlive a failed set (only the new project folder is taken away by it).
    const shared: string[] = [];
    try {
      for (const [path, bytes] of [...binaries].sort(([a], [b]) =>
        a < b ? -1 : 1,
      )) {
        const rel = relativeTo(root, path) as string;
        if (!rel.startsWith(`${project}/`)) shared.push(rel);
        const up = await remote.upload(
          posix.dirname(rel),
          posix.basename(rel),
          bytes,
        );
        if (up.status !== 200)
          throw new Error(
            String(up.body.error ?? `/upload answered ${up.status}`),
          );
      }
      const answer = await remote.changes({
        at: "",
        base: l.rev,
        label: `import ${s.source}`,
        create: project,
        ops,
      });
      if (answer.status !== 200)
        throw new Error(
          answer.body.error ?? `/changes answered ${answer.status}`,
        );
    } catch (e) {
      // A set that never landed has already taken its folder; one that never got that far is told to.
      await remote
        .changes({
          at: "",
          base: l.rev,
          label: "import",
          create: project,
          discard: true,
          ops: [],
        })
        .catch(() => undefined);
      for (const rel of shared)
        await remote.remove?.(rel).catch(() => undefined);
      throw e;
    }
    return {
      project,
      log: importLog(
        s.source,
        project,
        translation,
        emitted,
        sample?.outside ?? new Set(),
      ),
    };
  }

  return {
    async importFile(data, name, as) {
      checkDroppable(name);
      checkSize(name, sizeOf(data));
      if (sizeOf(data) === 0)
        throw new Error(`cannot import "${name}": the file is empty`);
      return run({ source: name, file: name, bytes: await bytesOf(data), as });
    },
    async importPath(rel, as) {
      const file = posix.basename(rel.replace(/\\/g, "/"));
      const ext = file.slice(file.lastIndexOf(".")).toLowerCase();
      if (!(IMPORTABLE as readonly string[]).includes(ext))
        throw new Error(
          `cannot import "${rel}": expected one of ${IMPORTABLE.join(", ")}`,
        );
      const bytes = await remote.bytes?.(rel);
      if (!bytes) throw new Error(`no such file: ${rel}`);
      return run({ source: rel, file, bytes, as, alsRel: rel });
    },
  };
}
