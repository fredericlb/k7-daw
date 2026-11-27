// projectSchema.ts — the project root's constants the page reads: the master fader's range and the project shapes.
// A reader would get wrong that nothing here validates: the file's shape is the Rust reader's, asked through `file.validate`.

/** The master fader's range in dB, so the page's fader and `project master` cannot disagree (docs/projectFile.md). */
export const MASTER_MIN_DB = -100;
export const MASTER_MAX_DB = 12;

export { PROJECT_SHAPES, type ProjectShape } from "../projectShapes";
