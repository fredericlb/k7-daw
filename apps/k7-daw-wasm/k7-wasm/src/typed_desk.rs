// typed_desk.rs — the desk's hot call: numbers in, a number out, no JSON, once per pointer move of a scratch.
// A reader would get wrong how "no rate" crosses: NaN, since a release hands the record back and never waits.

use crate::typed::opt;
use turntables::deck::scratch_delay;

/// How long to hold this scratch, in milliseconds: 0 goes now. `rate` NaN is a release, and `every` NaN is the default pace.
#[unsafe(no_mangle)]
pub extern "C" fn k7_g_scratch_delay(since_last: f64, rate: f64, every: f64) -> f64 {
    scratch_delay(since_last, opt(rate), opt(every))
}
