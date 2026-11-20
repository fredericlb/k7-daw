// offer.ts — the host's half of a lens handshake, for a device in camera mode (./cameraLink.ts): one
// peer per viewer, keyed by the id the cameras socket gives.
// What reads wrong: the offer frame itself is the caller's to send; only the ICE frame is written here.

/** The frames that reach a viewer's peer once its offer is out. */
export type PeerFrame =
  | { type: "answer"; from?: string; sdp: { type: string; sdp?: string } }
  | { type: "ice"; from?: string; candidate: RTCIceCandidateInit }
  | { type: "gone"; from: string };

/** Open a peer for viewer `id` carrying `tracks`, and return the offer the caller sends it. */
export async function offerStream(o: {
  peers: Map<string, RTCPeerConnection>;
  id: string;
  config: RTCConfiguration;
  stream: MediaStream;
  tracks: MediaStreamTrack[];
  sendIce: (f: {
    type: "ice";
    to: string;
    candidate: RTCIceCandidateInit;
  }) => void;
  drop: (id: string) => void;
}): Promise<RTCSessionDescriptionInit> {
  const { id, drop } = o;
  const pc = new RTCPeerConnection(o.config);
  o.peers.set(id, pc);
  for (const t of o.tracks) pc.addTrack(t, o.stream);
  pc.onicecandidate = (e) => {
    if (e.candidate)
      o.sendIce({ type: "ice", to: id, candidate: e.candidate.toJSON() });
  };
  // A viewer that leaves without closing its socket is only noticed here.
  pc.onconnectionstatechange = () => {
    if (pc.connectionState === "failed" || pc.connectionState === "closed")
      drop(id);
  };
  const sdp = await pc.createOffer();
  await pc.setLocalDescription(sdp);
  return pc.localDescription ?? sdp;
}

/** Hand an answer or a candidate to its viewer's peer, or drop a viewer that left. */
export function toPeer(
  peers: Map<string, RTCPeerConnection>,
  f: PeerFrame,
  drop: (id: string) => void,
): void {
  if (f.type === "answer" && f.from)
    void peers
      .get(f.from)
      ?.setRemoteDescription(f.sdp as RTCSessionDescriptionInit)
      .catch(() => undefined);
  if (f.type === "ice" && f.from)
    void peers
      .get(f.from)
      ?.addIceCandidate(f.candidate)
      .catch(() => undefined);
  if (f.type === "gone") drop(f.from);
}
