// cube.rs — a `.cube` 3D LUT read into what a 3D texture takes: its edge size and an RGBA float per entry, red varying fastest.
// A reader would get wrong that a malformed file is refused by name here, not by a black frame: a LUT read wrongly grades wrongly.

/// A 3D LUT ready to upload: `size`³ entries, four floats each, already mapped onto 0..1.
#[derive(Clone, Debug, PartialEq)]
pub struct Cube {
    pub size: usize,
    pub data: Vec<f32>,
}

/// JavaScript's `Number(text)` for the forms a `.cube` holds: a decimal, or NaN.
fn number(s: &str) -> f64 {
    match s.parse::<f64>() {
        Ok(v) if v.is_finite() => v,
        // `inf` and `nan` are Rust's spellings, which JavaScript does not read.
        _ => f64::NAN,
    }
}

fn triple(rest: &[&str]) -> Vec<f64> {
    rest.iter().map(|s| number(s)).collect()
}

/// Parse a `.cube` file; the error names what is wrong.
pub fn parse_cube(text: &str) -> Result<Cube, String> {
    let mut size = 0.0_f64;
    let mut min = vec![0.0, 0.0, 0.0];
    let mut max = vec![1.0, 1.0, 1.0];
    let mut rows: Vec<[f64; 3]> = Vec::new();
    for raw in text.split('\n') {
        let line = raw.trim_end_matches('\r').trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let (key, rest) = (words[0], &words[1..]);
        match key {
            "TITLE" => continue,
            "LUT_1D_SIZE" => return Err("a 1D LUT — only a 3D one (LUT_3D_SIZE) grades a picture here".into()),
            "LUT_3D_SIZE" => {
                size = rest.first().map_or(f64::NAN, |s| number(s));
                continue;
            }
            "DOMAIN_MIN" => {
                min = triple(rest);
                continue;
            }
            "DOMAIN_MAX" => {
                max = triple(rest);
                continue;
            }
            _ => {}
        }
        let v: Vec<f64> = words.iter().map(|s| number(s)).collect();
        if v.len() == 3 && v.iter().all(|x| x.is_finite()) {
            rows.push([v[0], v[1], v[2]]);
        }
    }
    if !(2.0..=256.0).contains(&size) {
        let shown = if size.is_nan() || size == 0.0 {
            "missing".to_owned()
        } else {
            size.to_string()
        };
        return Err(format!("LUT_3D_SIZE {shown} — expected 2 to 256"));
    }
    let need = size.powi(3);
    if rows.len() as f64 != need {
        return Err(format!("{} entries for a {size}³ LUT, which needs {need}", rows.len()));
    }
    let at = |v: &[f64], c: usize| v.get(c).copied().unwrap_or(f64::NAN);
    let mut data = Vec::with_capacity(rows.len() * 4);
    for r in &rows {
        // A domain other than 0..1 is folded back onto it, which is the range a texture holds.
        for (c, x) in r.iter().enumerate() {
            let span = at(&max, c) - at(&min, c);
            let span = if span == 0.0 || span.is_nan() { 1.0 } else { span };
            data.push(((x - at(&min, c)) / span) as f32);
        }
        data.push(1.0);
    }
    Ok(Cube {
        size: size as usize,
        data,
    })
}
