// midiAccess.ts — the page's one MIDIAccess, asked for once and kept for the page's life; every MIDI user shares it.
// What a reader gets wrong: releasing it is not free — Android's WebView leaks a listener at each MIDI start (docs/midi.md).

type MidiNavigator = Navigator & {
  requestMIDIAccess?: (opts?: { sysex?: boolean }) => Promise<MIDIAccess>;
};

let pending: Promise<MIDIAccess | null> | undefined;

/** Whether this browser offers Web MIDI at all, without asking for it. */
export function hasWebMidi(): boolean {
  return (
    typeof navigator !== "undefined" &&
    !!(navigator as MidiNavigator).requestMIDIAccess
  );
}

/** The shared access, null when there is no Web MIDI or it was refused; a refusal is asked again next time. */
export function midiAccess(): Promise<MIDIAccess | null> {
  if (!hasWebMidi()) return Promise.resolve(null);
  pending ??= (navigator as Required<MidiNavigator>)
    .requestMIDIAccess()
    .catch(() => {
      pending = undefined;
      return null;
    });
  return pending;
}

/** The listeners one MIDI user put on the shared ports, so it takes its own off and nobody else's. */
export class MidiListeners {
  private inputs = new Map<MIDIInput, (e: Event) => void>();
  private state?: { access: MIDIAccess; h: () => void };

  /** Listen on every input of `access`, replacing what this owner listened on before. */
  listen(
    access: MIDIAccess,
    handler: (input: MIDIInput, e: MIDIMessageEvent) => void,
  ): void {
    this.unlistenInputs();
    for (const input of access.inputs.values()) {
      const h = (e: Event) => handler(input, e as MIDIMessageEvent);
      input.addEventListener("midimessage", h);
      // A listener added this way opens no port by itself, where `onmidimessage` would.
      void input.open?.();
      this.inputs.set(input, h);
    }
  }

  /** Hear the port list change. */
  onState(access: MIDIAccess, h: () => void): void {
    this.state?.access.removeEventListener("statechange", this.state.h);
    access.addEventListener("statechange", h);
    this.state = { access, h };
  }

  private unlistenInputs(): void {
    for (const [input, h] of this.inputs)
      input.removeEventListener("midimessage", h);
    this.inputs.clear();
  }

  /** Every listener of this owner off; the access itself stays open for the page. */
  clear(): void {
    this.unlistenInputs();
    this.state?.access.removeEventListener("statechange", this.state.h);
    this.state = undefined;
  }
}
