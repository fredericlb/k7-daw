//! urls.rs — the cases of `apps/k7-daw-frontend/packages/web-protocol/services.test.ts`, replayed: the supervisor
//! and the web apps must derive the same registry from the same text. (`$STUDIO_PORTS` is the registry part's
//! own test, which owns its process.)

use k7_supervisor::engine::allowed_origins;
use k7_supervisor::urls::*;

#[test]
fn the_registry_is_the_family_root_whichever_service_was_named() {
    assert_eq!(
        engine_registry_url("https://app.k7.rocks"),
        "https://app.k7.rocks"
    );
    assert_eq!(
        engine_registry_url("https://app.k7.rocks/band/winter.k7"),
        "https://app.k7.rocks"
    );
}

#[test]
fn on_loopback_the_registry_is_k7s_port() {
    assert_eq!(
        engine_registry_url("http://localhost:8080"),
        "http://localhost:8789"
    );
    assert_eq!(
        engine_registry_url("http://127.0.0.1:5223"),
        "http://127.0.0.1:8789"
    );
}

#[test]
fn k7s_port_follows_a_moved_ports_spec() {
    assert_eq!(k7_port_in("k7=18789,pl8=18795"), "18789");
    assert_eq!(k7_port_in("pl8=1,k7=2"), "2");
    for bad in [
        "k7=nope",
        "k7=0",
        "k7=65536",
        "k7=123456",
        "k7=",
        "pl8=9",
        "",
    ] {
        assert_eq!(k7_port_in(bad), "8789", "{bad}");
    }
}

#[test]
fn every_service_resolves_to_the_origin_it_is_given() {
    assert_eq!(service_url("https://app.k7.rocks"), "https://app.k7.rocks");
    assert_eq!(
        service_url("https://studio.example:8443/songs/a"),
        "https://studio.example:8443"
    );
    assert_eq!(
        service_url("http://localhost:5223"),
        "http://localhost:5223"
    );
    assert_eq!(service_url("not a url"), "");
}

#[test]
fn a_non_url_or_a_non_web_scheme_has_no_registry() {
    assert_eq!(engine_registry_url("not a url"), "");
    assert_eq!(engine_registry_url("ftp://app.k7.rocks"), "");
}

#[test]
fn the_family_is_the_origin_or_bare_loopback() {
    assert_eq!(
        studio_family("https://studio.example:8443/library"),
        "https://studio.example:8443"
    );
    assert_eq!(
        studio_family("https://app.k7.rocks"),
        "https://app.k7.rocks"
    );
    assert_eq!(studio_family("http://localhost:8080"), "http://localhost");
    assert_eq!(studio_family("http://localhost:8789"), "http://localhost");
    assert_eq!(studio_family("not a url"), "");
}

#[test]
fn a_users_origin_is_strict() {
    assert_eq!(
        normalise_user_origin("app.k7.rocks").as_deref(),
        Some("https://app.k7.rocks")
    );
    assert_eq!(
        normalise_user_origin("  https://Daw.Test:443/path?q=1 ").as_deref(),
        Some("https://daw.test")
    );
    assert_eq!(
        normalise_user_origin("http://localhost:8789/x").as_deref(),
        Some("http://localhost:8789")
    );
    for bad in [
        "ftp://daw.test",
        "https://user:pw@daw.test",
        "https://*.daw.test",
        "   ",
        "https://bad host",
    ] {
        assert_eq!(normalise_user_origin(bad), None, "{bad}");
    }
}

// A heartbeat carries the token and the tunnel secret, so plain http goes only where nobody can listen.
#[test]
fn plain_http_goes_only_to_loopback_or_the_allowlist() {
    for ok in [
        "https://app.k7.rocks",
        "wss://app.k7.rocks/api/tunnel",
        "http://localhost:8789",
        "ws://127.0.0.1:8789/x",
        "http://[::1]:8789",
    ] {
        assert_eq!(plaintext_refusal_with(ok, ""), "", "{ok}");
    }
    for refused in [
        "http://app.k7.rocks",
        "ws://studio.lan:8080",
        "http://192.168.1.2:8789",
        "not a url",
    ] {
        assert!(!plaintext_refusal_with(refused, "").is_empty(), "{refused}");
    }
    assert!(
        plaintext_refusal_with("ftp://app.k7.rocks", "k7").contains("is not an http(s) address")
    );
    assert!(plaintext_refusal_with("http://app.k7.rocks", "").contains("would cross in clear"));
    assert_eq!(plaintext_refusal_with("http://k7:8789", "k7"), "");
    assert_eq!(
        plaintext_refusal_with("ws://K7:8789/api", " other , K7 "),
        ""
    );
    assert_eq!(
        plaintext_refusal_with("http://studio.lan:8080", "studio.lan:8080"),
        ""
    );
    assert!(!plaintext_refusal_with("http://studio.lan:9090", "studio.lan:8080").is_empty());
    assert!(!plaintext_refusal_with("http://k7.evil:8789", "k7").is_empty());
}

#[test]
fn a_cache_folder_is_named_by_the_host() {
    assert_eq!(
        cache_key("https://pl8.priv.frediver.se:8443/api"),
        "pl8.priv.frediver.se_8443"
    );
    assert_eq!(cache_key("https://app.k7.rocks"), "app.k7.rocks");
    assert_eq!(cache_key("http://[::1]:8795"), "___1__8795");
}

#[test]
fn loopback_hosts() {
    for yes in ["localhost", "127.0.0.1", "127.10.2.3", "[::1]"] {
        assert!(is_loopback_host(yes), "{yes}");
    }
    for no in [
        "127.0.0",
        "128.0.0.1",
        "127.0.0.1.2",
        "127.0.0.x",
        "example.com",
    ] {
        assert!(!is_loopback_host(no), "{no}");
    }
}

#[test]
fn joining_drops_one_trailing_slash() {
    assert_eq!(
        join("https://a.example/", "/api/x"),
        "https://a.example/api/x"
    );
    assert_eq!(
        join("https://a.example", "/api/x"),
        "https://a.example/api/x"
    );
}

// The engine lets in the pages of the studios it is listed at: their origins, once each, nothing else.
#[test]
fn the_engine_lets_in_the_studios_origins() {
    let list: Vec<String> = [
        "https://studio.example.org/",
        "http://studio.lan:8080",
        "https://studio.example.org",
        "not a url",
        "ftp://x.example",
    ]
    .map(String::from)
    .into();
    assert_eq!(
        allowed_origins(&list),
        "https://studio.example.org,http://studio.lan:8080"
    );
    assert_eq!(allowed_origins(&[]), "");
}
