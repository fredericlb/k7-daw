// spectrum.rs — what colour a column of the waveform is, from the three band peaks IN it, as an RGBA strip per tile.
// A reader would get wrong that the loudest band normalises the mix, not the sum: pure bass stays pure red.

/// The three band colours, ADDITIVE, each kept low in the two channels it does not own.
pub const BAND_COLOURS: [[f64; 3]; 3] = [
    [255.0, 45.0, 10.0],
    [90.0, 235.0, 80.0],
    [110.0, 80.0, 255.0],
];

/// One column's three band peaks, 0..1: low, mid, high.
pub type Bands = [f64; 3];

/// The colour of a column, normalised by the loudest band; `fallback` when no band has anything in it.
#[must_use]
pub fn column_rgb(b: Bands, fallback: [u8; 3]) -> [u8; 3] {
    let top = b[0].max(b[1]).max(b[2]);
    if top.is_nan() || top <= 0.0 {
        return fallback;
    }
    let (l, m, h) = (b[0] / top, b[1] / top, b[2] / top);
    let mix = |i: usize| {
        (BAND_COLOURS[0][i] * l + BAND_COLOURS[1][i] * m + BAND_COLOURS[2][i] * h)
            .round()
            .min(255.0) as u8
    };
    [mix(0), mix(1), mix(2)]
}

/// An RGBA strip, four bytes a column, opaque; a column with no analysis (`None`) takes `plain`.
#[must_use]
pub fn strip(columns: &[Option<Bands>], plain: [u8; 3]) -> Vec<u8> {
    let mut out = Vec::with_capacity(columns.len() * 4);
    for c in columns {
        let [r, g, b] = c.map_or(plain, |b| column_rgb(b, plain));
        out.extend_from_slice(&[r, g, b, 255]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: [u8; 3] = [148, 163, 184];
    fn rgb(l: f64, m: f64, h: f64) -> [u8; 3] {
        column_rgb([l, m, h], PLAIN)
    }

    #[test]
    fn red_where_the_bass_is() {
        let [r, g, b] = rgb(0.8, 0.02, 0.01);
        assert!(r > g && r > b && r > 200);
    }

    #[test]
    fn green_where_the_voice_is() {
        let [r, g, b] = rgb(0.02, 0.8, 0.02);
        assert!(g > r && g > b);
    }

    #[test]
    fn blue_where_the_cymbals_are() {
        let [r, g, b] = rgb(0.01, 0.05, 0.8);
        assert!(b > r && b > g);
    }

    #[test]
    fn blends_additively() {
        let [r, g, b] = rgb(0.8, 0.8, 0.02);
        assert!(r > 150 && g > 150 && b < 120);
        let [r, g, b] = rgb(0.8, 0.02, 0.8);
        assert!(r > 150 && b > 150 && g < 150);
        let [r, g, b] = rgb(0.8, 0.8, 0.8);
        assert!(r.min(g).min(b) > 200);
    }

    #[test]
    fn says_the_same_quietly_as_loudly() {
        assert_eq!(rgb(0.9, 0.3, 0.1), rgb(0.09, 0.03, 0.01));
    }

    #[test]
    fn keeps_a_pure_band_pure() {
        assert_eq!(rgb(0.5, 0.0, 0.0), [255, 45, 10]);
    }

    #[test]
    fn falls_back_rather_than_black() {
        assert_eq!(rgb(0.0, 0.0, 0.0), PLAIN);
        assert_eq!(column_rgb([0.0; 3], [1, 2, 3]), [1, 2, 3]);
    }

    #[test]
    fn a_strip_is_four_bytes_a_column_and_plain_where_unanalysed() {
        let s = strip(&[Some([0.5, 0.0, 0.0]), None], PLAIN);
        assert_eq!(s, vec![255, 45, 10, 255, 148, 163, 184, 255]);
    }
}
