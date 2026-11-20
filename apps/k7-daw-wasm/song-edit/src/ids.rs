// ids.rs — where a fresh node id comes from: the host hands one in, since the crate reads neither a clock nor a random source.
// A reader would get wrong the order: a function takes its ids in the order the TypeScript made them, which is what a replay pins.

/// A source of v4 UUIDs, one per call.
pub type NewId<'a> = &'a mut dyn FnMut() -> String;
