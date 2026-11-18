// clips/warp.rs — a clip file's stretch map as the compiled document carries it; after warpPoints.ts' readClipWarp.
// A reader would get wrong that the map and the file's refusal of it are answered apart: a placement may decline it too.

use super::doc::WarpDoc;
use crate::files::Warp;

/// The map a clip file declares (two markers at least) and whether the file declines it.
pub fn read_clip_warp(raw: Option<&Warp>) -> (Option<WarpDoc>, bool) {
    let Some(w) = raw else { return (None, false) };
    let map = (w.markers.len() >= 2).then(|| WarpDoc {
        algorithm: w.algorithm.0.clone(),
        markers: w.markers.clone(),
    });
    (map, w.disable == Some(true))
}
