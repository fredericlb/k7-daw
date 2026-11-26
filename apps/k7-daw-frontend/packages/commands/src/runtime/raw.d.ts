// raw.d.ts — the bundler's `?raw` import: a file's text, as the bundles take it.

declare module "*?raw" {
  const text: string;
  export default text;
}
