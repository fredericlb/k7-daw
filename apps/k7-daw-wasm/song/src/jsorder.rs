// jsorder.rs — the two orders JavaScript gives a listing, written once: `[base, name]` pairs sorted bare, and an object's own keys.
// A reader would get wrong that bytewise is neither: `-` is below `.`, UTF-16 differs from UTF-8 above U+FFFF, and `"10"` follows `"9"`.

use indexmap::IndexMap;
use std::cmp::Ordering;

/// How `[...pairs].sort()` orders `[base, name]`: by `base + "," + name` in UTF-16 units (decision 381).
pub fn pair_cmp(a: (&str, &str), b: (&str, &str)) -> Ordering {
    fn units<'a>(p: (&'a str, &'a str)) -> impl Iterator<Item = u16> + 'a {
        p.0.encode_utf16()
            .chain(std::iter::once(0x2c))
            .chain(p.1.encode_utf16())
    }
    units(a).cmp(units(b))
}

/// The number an array-index key spells (`"0"`, `"7"`, never `"07"` or `"4294967295"`), which JavaScript lists first.
fn index_of(key: &str) -> Option<u32> {
    let canonical = key == "0" || (!key.starts_with('0') && !key.is_empty());
    if !canonical || key.len() > 10 || !key.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    key.parse::<u32>().ok().filter(|n| *n != u32::MAX)
}

/// Puts a map in the order `Object.entries` lists its own keys: integer-like keys ascending, then the rest as inserted.
pub fn own_key_order<T>(map: &mut IndexMap<String, T>) {
    if !map.keys().any(|k| index_of(k).is_some()) {
        return;
    }
    let all = std::mem::take(map);
    let (mut numbers, rest): (Vec<_>, Vec<_>) = all.into_iter().partition(|(k, _)| index_of(k).is_some());
    numbers.sort_by_key(|(k, _)| index_of(k));
    map.extend(numbers);
    map.extend(rest);
}

/// A map's entries in that order, borrowed.
pub fn own_key_entries<T>(map: &IndexMap<String, T>) -> Vec<(&String, &T)> {
    let mut out: Vec<(&String, &T)> = map.iter().collect();
    out.sort_by_key(|(k, _)| index_of(k).map_or((1, 0), |n| (0, n)));
    out
}

/// The same order over a plain list of keys.
pub fn own_key_sort(keys: &mut [String]) {
    keys.sort_by_key(|k| index_of(k).map_or((1, 0), |n| (0, n)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dash_sorts_below_the_extension_and_the_comma_decides_the_pair() {
        assert_eq!(pair_cmp(("a", "a.k7x"), ("a-b", "a-b.k7x")), Ordering::Less);
        assert_eq!(pair_cmp(("a-b", "a-b.k7x"), ("a", "a.k7x")), Ordering::Greater);
    }

    #[test]
    fn integer_like_keys_come_first_and_ascending() {
        let mut m: IndexMap<String, u8> = ["b", "10", "a", "9", "07", "4294967295"]
            .iter()
            .enumerate()
            .map(|(i, k)| ((*k).to_owned(), i as u8))
            .collect();
        own_key_order(&mut m);
        let keys: Vec<&str> = m.keys().map(String::as_str).collect();
        assert_eq!(keys, ["9", "10", "b", "a", "07", "4294967295"]);
    }
}
