// workspaceFiles.ts — the file calls the home page makes on one workspace: a file's content for the tree, and the one
// write there is, a library pin (LibraryPins.tsx). Same transport as api.ts's `accountFiles`.

import { readJson as json } from "@studio/http-client/call";
import { type FileContent, WORKSPACE_HEADER } from "@studio/web-protocol";
import { type AccountFiles, filesHeaders } from "./api";

/** What the read-only tree and its viewer read through: one workspace's files. */
export interface TreeSource {
  /** Changes when the source does, so what was read of the last one is dropped. */
  key: string;
  read(path: string): Promise<FileContent>;
}

/** A workspace of the account, as the home page reads it. */
export const workspaceSource = (
  m: AccountFiles,
  workspace: string,
): TreeSource => ({
  key: `workspace:${workspace}`,
  read: (path) => workspaceFiles.read(m, workspace, path),
});

export const workspaceFiles = {
  read: async (
    m: AccountFiles,
    workspace: string,
    path: string,
  ): Promise<FileContent> =>
    json<FileContent>(
      await fetch(`${m.fileBase}/file?path=${encodeURIComponent(path)}`, {
        headers: { ...filesHeaders(m), [WORKSPACE_HEADER]: workspace },
      }),
      "file",
    ),

  /** Replace one file's text: the Libraries page moves a workspace's pin this way, the only write made from here. */
  write: async (
    m: AccountFiles,
    workspace: string,
    path: string,
    content: string,
  ): Promise<void> => {
    await json<{ ok: true }>(
      await fetch(`${m.fileBase}/file`, {
        method: "PUT",
        headers: {
          ...filesHeaders(m),
          [WORKSPACE_HEADER]: workspace,
          "content-type": "application/json",
        },
        body: JSON.stringify({ path, content }),
      }),
      "write",
    );
  },
};
