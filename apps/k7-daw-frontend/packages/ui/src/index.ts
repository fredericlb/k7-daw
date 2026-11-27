// @studio/ui — shadcn/ui primitives shared by the app and its packages.
//
// The app itself imports these via per-component subpaths (e.g.
// "@studio/ui/button"), matching shadcn's usual per-file convention. This barrel
// exists only as a single entry point for tooling that wants the whole
// package's export surface at once (type extraction, bundling for external
// consumption) — nothing in the frontend imports from it directly.

export * from "./backdrop";
export * from "./badge";
export * from "./button";
export * from "./capsule";
export * from "./card";
export * from "./checkbox";
export * from "./chip";
export * from "./choice";
export * from "./dialog";
export * from "./dropdown-menu";
export * from "./empty-state";
export * from "./error-card";
export * from "./file-tree";
export * from "./input";
export * from "./knob";
export * from "./locale-picker";
export * from "./picker";
export * from "./popover";
export * from "./progress";
export * from "./row-group";
export * from "./section-heading";
export * from "./select";
export * from "./side-panel";
export * from "./sign-in-screen";
export * from "./spinner";
export * from "./starfield";
export * from "./switch";
export * from "./tab-bar";
export * from "./tabs";
export * from "./track-toggle";
