// tree.ts — the order a person reads names in: blind to case and accents, "2" before "10".
// The services answer in code-point order (song-schema/docs/textOrder.md), so every view that
// draws a listing for people sorts it again with these; one collator, so two views never
// disagree. Here rather than in an app because pl8 draws its families with it too.

// Intl rather than `<`: "Ébène" belongs beside "Ebene", and a take numbered 10 after one numbered 2.
const collator = new Intl.Collator("fr", {
  numeric: true,
  sensitivity: "base",
});

/** Two names as a person orders them; names the collator calls equal fall back to code units, so the order is total. */
export function byPeople(a: string, b: string): number {
  return collator.compare(a, b) || (a < b ? -1 : a > b ? 1 : 0);
}

/** Two slash paths, one segment at a time: a folder's contents stay together, under it. */
export function byPeoplePath(a: string, b: string): number {
  const p = a.split("/");
  const q = b.split("/");
  for (let i = 0; i < Math.min(p.length, q.length); i++) {
    const c = byPeople(p[i], q[i]);
    if (c) return c;
  }
  return p.length - q.length;
}

/** A NEW array of `items`, sorted for people by the text `of` each. */
export function sortForPeople<T>(
  items: readonly T[],
  of: (item: T) => string,
): T[] {
  return [...items].sort((a, b) => byPeople(of(a), of(b)));
}

/** Sort a level and everything under it, into NEW arrays: what comes in is React state. */
export function sortNodes<
  N extends { name: string; dir?: boolean; children?: N[] },
>(
  nodes: N[],
  // Folders above files, where a view keeps the file service's grouping; mixed otherwise.
  opts: { foldersFirst?: boolean } = {},
): N[] {
  const rank = (n: N) => (opts.foldersFirst && (n.dir ?? !!n.children) ? 0 : 1);
  return nodes
    .map((n) =>
      n.children ? { ...n, children: sortNodes(n.children, opts) } : n,
    )
    .sort((a, b) => rank(a) - rank(b) || byPeople(a.name, b.name));
}
