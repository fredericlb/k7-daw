//! bounds.rs — the numbers a document off the wire is held to, in one table (`docs/project.md` says why each).
//! What a reader gets wrong: a value past a bound is clamped or replaced by its default, never refused, so a
//! project with one wild number still plays; only a value of the wrong TYPE is refused.

/// A pad's `repeat` and `skip`: loop counts the builder writes notes for.
pub const MAX_PAD_REPEAT: i32 = 64;
/// The output pairs a sampler or a routed tap can address: 0..=31.
pub const MAX_PAIR: i32 = 31;
/// A MIDI note or key number.
pub const MAX_KEY: i32 = 127;
/// The rates an interface opens; any other is read as `DEFAULT_SAMPLE_RATE`.
pub const SAMPLE_RATES: std::ops::RangeInclusive<i32> = 8000..=768_000;
pub const DEFAULT_SAMPLE_RATE: i32 = 48_000;
pub const DEFAULT_TEMPO: f64 = 120.0;
/// The numerators and denominators Tracktion can divide by; any other signature is read as 4/4.
pub const TIME_SIG_NUMERATORS: std::ops::RangeInclusive<i32> = 1..=64;
pub const TIME_SIG_DENOMINATORS: std::ops::RangeInclusive<i32> = 1..=32;

/// `v` held to `lo..=hi`.
pub fn clamp(v: i32, lo: i32, hi: i32) -> i32 {
    v.max(lo).min(hi)
}

/// A denominator Tracktion can divide by: a power of two up to 32.
pub fn denominator_ok(d: i32) -> bool {
    TIME_SIG_DENOMINATORS.contains(&d) && d & (d - 1) == 0
}
