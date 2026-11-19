// @studio/ui — shadcn/ui primitives shared by the app and its packages.
//
// The app itself imports these via per-component subpaths (e.g.
// "@studio/ui/button"), matching shadcn's usual per-file convention. This barrel
// exists only as a single entry point for tooling that wants the whole
// package's export surface at once (type extraction, bundling for external
// consumption) — nothing in the frontend imports from it directly.

export * from "./app-header";
export * from "./ask";
export * from "./badge";
export * from "./button";
export * from "./card";
export * from "./checkbox";
export * from "./dialog";
export * from "./dropdown-menu";
export * from "./field";
export * from "./file-tree";
export * from "./frediverse-mark";
export * from "./hamburger-menu";
export * from "./input";
export * from "./label";
export * from "./locale-picker";
export * from "./popover";
export * from "./select";
export * from "./sign-in-screen";
export * from "./starfield";
export * from "./switch";
export * from "./table";
export * from "./tabs";
export * from "./textarea";
export * from "./timeline";
export * from "./user-menu";
