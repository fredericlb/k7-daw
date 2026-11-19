//! 1l's names (2444, 2451, 2460) and the route's clamps (2427) replayed from the TypeScript's own answers.

use k7_file_service::media::names::{
    Take, models_root, prefetch_signature,
    take_name,
};
use k7_file_service::media::clamp_buckets;


#[test]
fn a_model_root_is_the_environment_or_the_homes_models_folder() {
    assert_eq!(models_root(Some("/srv/m/"), "/home/u/.k7"), "/srv/m");
    assert_eq!(models_root(Some(""), "/home/u/.k7"), "/home/u/.k7/models");
    assert_eq!(models_root(None, "C:/Users/u/.k7"), "C:/Users/u/.k7/models");
}

#[test]
fn a_mirrors_take_keeps_only_its_name() {
    assert_eq!(
        take_name("/rec", "C:\\Users\\x\\mirror\\take.wav"),
        Take::Land("/rec/take.wav".into())
    );
    assert_eq!(
        take_name("/rec", "/rec/sub/../take.wav"),
        Take::Here("/rec/sub/../take.wav".into())
    );
    assert_eq!(take_name("/rec", "/m/.."), Take::Refused);
    assert_eq!(take_name("", "/m/take.wav"), Take::Refused);
}

#[test]
fn the_prefetch_signature_is_the_address_and_the_sorted_files() {
    let files = vec!["/m/a.wav".to_string(), "/m/b.wav".to_string()];
    assert_eq!(
        prefetch_signature("http://j", &files),
        "http://j\n/m/a.wav\n/m/b.wav"
    );
    assert_eq!(prefetch_signature("http://j", &[]), "http://j\n");
}



#[test]
fn a_fractional_bucket_count_is_cut_after_the_clamp() {
    assert_eq!(clamp_buckets(Some("16.5")), 16);
    assert_eq!(clamp_buckets(Some("999.9")), 999);
    assert_eq!(clamp_buckets(Some("3999.99")), 3999);
    assert_eq!(clamp_buckets(Some("4000.5")), 4000);
    assert_eq!(clamp_buckets(Some("0.5")), 16);
    assert_eq!(clamp_buckets(Some("0")), 1000);
    assert_eq!(clamp_buckets(Some("1.5")), 16);
}
