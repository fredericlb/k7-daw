// raw.d.ts — the bundler's `?raw` import: a file's text, as the Worker takes commands.yaml.

declare module "*?raw" {
  const text: string;
  export default text;
}
