// aspect.rs — the shapes a project's FRAME can be: the short list a person picks from, and which one a viewport is.
// A reader would get wrong that a viewport is read by RATIO: `[1280, 720]` is 16:9 as surely as `[1920, 1080]`.

/// One frame shape: what it is called, and the pixels it means.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aspect {
    pub name: &'static str,
    pub w: f64,
    pub h: f64,
}

/// The shapes on offer: 1080-tall landscape and 1080-wide portrait.
pub const ASPECTS: [Aspect; 2] = [
    Aspect {
        name: "16:9",
        w: 1920.0,
        h: 1080.0,
    },
    Aspect {
        name: "9:16",
        w: 1080.0,
        h: 1920.0,
    },
];

/// Which shape a viewport IS, within a tenth of a percent of its ratio; None when it matches none.
#[must_use]
pub fn aspect_of(w: f64, h: f64) -> Option<Aspect> {
    if w.is_nan() || h.is_nan() || w <= 0.0 || h <= 0.0 {
        return None;
    }
    let ratio = w / h;
    ASPECTS
        .iter()
        .find(|a| (ratio - a.w / a.h).abs() < a.w / a.h / 1000.0)
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(w: f64, h: f64) -> Option<&'static str> {
        aspect_of(w, h).map(|a| a.name)
    }

    #[test]
    fn a_viewport_is_read_by_its_ratio_not_its_size() {
        assert_eq!(name(1920.0, 1080.0), Some("16:9"));
        assert_eq!(name(1280.0, 720.0), Some("16:9"));
        assert_eq!(name(1366.0, 768.0), Some("16:9"));
        assert_eq!(name(1080.0, 1920.0), Some("9:16"));
        assert_eq!(name(720.0, 1280.0), Some("9:16"));
    }

    #[test]
    fn a_shape_on_neither_list_says_nothing() {
        assert_eq!(name(640.0, 480.0), None);
        assert_eq!(name(1000.0, 1000.0), None);
    }

    #[test]
    fn a_viewport_that_is_not_one_answers_nothing() {
        assert_eq!(name(0.0, 1080.0), None);
        assert_eq!(name(1920.0, 0.0), None);
        assert_eq!(name(-1.0, -1.0), None);
        assert_eq!(name(f64::NAN, 1080.0), None);
    }

    #[test]
    fn every_name_on_the_list_round_trips() {
        for a in ASPECTS {
            assert_eq!(name(a.w, a.h), Some(a.name));
        }
    }
}
