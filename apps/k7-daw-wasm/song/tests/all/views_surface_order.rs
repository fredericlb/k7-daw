// views_surface_order.rs — the surfaces come in the order `projectSurfaces` sorts its `[base, name]` pairs: as strings.
// A reader would get wrong that this is the order `read_family` lists: there `9` comes before `10`, and here it does not.

use crate::views_world;

use song::Cache;
use song::views::surfaces::project_surfaces;
use views_world::world;

const SURFACE: &str = "type(\"timeline\")\nlayout(\"split\")\n";

// 603: integer-like names sort as text (`10` before `9`), and `-` is below `.` in the pair's comma form.
#[test]
fn surfaces_sort_as_pairs_of_text_and_not_as_object_keys() {
    let files: Vec<(String, &str)> = [
        "surfaces/9.k7x",
        "surfaces/10.k7x",
        "surfaces/a.k7x",
        "surfaces/a-b.k7x",
        "surfaces/a.b.k7x",
    ]
    .iter()
    .map(|f| ((*f).to_owned(), SURFACE))
    .collect();
    let refs: Vec<(&str, &str)> = files.iter().map(|(f, t)| (f.as_str(), *t)).collect();
    let got = project_surfaces(&world(&refs), &mut Cache::new(), "/p/ws/proj").expect("surfaces");
    let ids: Vec<&str> = got.surfaces.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["10", "9", "a", "a-b", "a.b"]);
}
