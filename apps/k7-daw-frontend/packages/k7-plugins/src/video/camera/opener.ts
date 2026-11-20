// opener.ts — which lens a camera label opens, the one choice the viewport and the recorder share.
// What reads wrong: a `remote:` label is the account's camera on another device, opened over a peer
// connection whichever browser draws it; any other label is this browser's own (packages/song/docs/camera.md).

import { type LensOpener, localLens } from "@studio/ui-eyes/camera";
import { isRemoteLabel } from "@studio/web-protocol/cameras";
import { remoteCameraLens } from "./remoteCamera";

/** The account's camera for a `remote:` label, this browser's for any other; the local opener says why it cannot. */
export const lensOpener = (): LensOpener => (label) =>
  isRemoteLabel(label) ? remoteCameraLens(label) : localLens(label);
