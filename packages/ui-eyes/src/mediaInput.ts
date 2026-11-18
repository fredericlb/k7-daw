// mediaInput.ts — a mediabunny Input on a URL or a Blob, every container mediabunny reads allowed.

import { ALL_FORMATS, BlobSource, Input, UrlSource } from "mediabunny";

/** A string is fetched with ranges; a Blob is read in place. */
export function openInput(src: string | Blob): Input {
  return new Input({
    source: typeof src === "string" ? new UrlSource(src) : new BlobSource(src),
    formats: ALL_FORMATS,
  });
}
