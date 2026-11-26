// text_hash.rs — the fingerprint a write names as the base its buffer was edited from.
// A reader would get wrong that cyrb53 runs over UTF-16 code units, not bytes or chars.

/// cyrb53 over the UTF-16 code units of `text`, as 14 hex digits.
pub fn text_hash(text: &str) -> String {
    let (mut h1, mut h2) = (0xdead_beef_u32, 0x41c6_ce57_u32);
    for c in text.encode_utf16() {
        h1 = (h1 ^ u32::from(c)).wrapping_mul(2_654_435_761);
        h2 = (h2 ^ u32::from(c)).wrapping_mul(1_597_334_677);
    }
    h1 = (h1 ^ (h1 >> 16)).wrapping_mul(2_246_822_507);
    h1 ^= (h2 ^ (h2 >> 13)).wrapping_mul(3_266_489_909);
    h2 = (h2 ^ (h2 >> 16)).wrapping_mul(2_246_822_507);
    h2 ^= (h1 ^ (h1 >> 13)).wrapping_mul(3_266_489_909);
    let n = 4_294_967_296_u64 * u64::from(2_097_151 & h2) + u64::from(h1);
    format!("{n:014x}")
}
