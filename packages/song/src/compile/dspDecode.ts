// compile/dspDecode.ts — the host's decoder of a `dsp.k7x`, which only the song crate holds: node has none.
// What a reader gets wrong: a host without it reads the `jsdsp.json` beside the file, which a test keeps equal to the decode.

/** Whether this host decodes a `dsp.k7x` itself. */
export const dspCanDecode = (): boolean => false;

/** The graph JSON text a `dsp.k7x` decodes to; throws the reason, `line N: …`, when it does not. */
export function decodeDsp(_text: string, _name: string): string {
  throw new Error("this machine cannot decode dsp.k7x");
}
