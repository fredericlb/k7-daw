//! addresses.rs — what a heartbeat advertises: the LAN addresses a browser could dial (no VPN, container or
//! link-local ones), a fingerprint that depends on the host and the MACs and on nothing else, and the live
//! machine's own presence.

use k7_supervisor::addresses::{Iface, phantom_interface, presence, presence_of};

fn iface(name: &str, ip: &str, mac: &str, internal: bool) -> Iface {
    Iface {
        name: name.into(),
        ipv4: vec![ip.into()],
        mac: mac.into(),
        internal,
        addressed: true,
    }
}

#[test]
fn interfaces_that_reach_no_browser_are_phantoms() {
    for name in [
        "vEthernet (WSL)",
        "WSL",
        "docker0",
        "br-1a2b",
        "veth12",
        "utun3",
        "tun0",
        "tap0",
        "Tailscale",
        "ZeroTier One",
        "wg0",
        "VMware Network",
        "vmnet8",
    ] {
        assert!(phantom_interface(name), "{name}");
    }
    for name in [
        "en0", "eth0", "Ethernet", "Wi-Fi", "wlan0", "wget0", "bridge0",
    ] {
        assert!(!phantom_interface(name), "{name}");
    }
}

#[test]
fn the_lan_list_leaves_out_what_no_browser_reaches() {
    let list = [
        iface("lo0", "127.0.0.1", "", true),
        iface("en0", "192.168.1.20", "aa:bb:cc:00:00:01", false),
        iface("en1", "169.254.7.7", "aa:bb:cc:00:00:02", false),
        iface("utun3", "100.64.0.9", "", false),
        iface("docker0", "172.17.0.1", "aa:bb:cc:00:00:03", false),
    ];
    let p = presence_of("studio", &list, 8765);
    assert_eq!(
        p["addresses"],
        serde_json::json!({"loopback": "127.0.0.1", "lanIps": ["192.168.1.20"], "port": 8765})
    );
    assert_eq!(
        (p["hostname"].as_str(), p["port"].as_u64()),
        (Some("studio"), Some(8765))
    );
}

#[test]
fn the_fingerprint_is_the_host_and_the_sorted_macs() {
    let a = [
        iface("en0", "10.0.0.2", "aa:bb:cc:00:00:02", false),
        iface("en1", "10.0.0.3", "aa:bb:cc:00:00:01", false),
    ];
    let reordered = [
        a[1].clone(),
        a[0].clone(),
        iface("lo0", "127.0.0.1", "00:00:00:00:00:00", true),
    ];
    let fp = |h: &str, l: &[Iface]| {
        presence_of(h, l, 1)["machineFingerprint"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(fp("studio", &a), fp("studio", &reordered));
    assert_ne!(fp("studio", &a), fp("other", &a));
    assert_ne!(fp("studio", &a), fp("studio", &a[..1]));
    // sha256("studio|aa:bb:cc:00:00:01,aa:bb:cc:00:00:02")
    assert_eq!(fp("studio", &a).len(), 64);
}

#[test]
fn this_machine_has_a_presence() {
    let p = presence(8765);
    assert!(!p["hostname"].as_str().unwrap().is_empty());
    assert_eq!(p["addresses"]["port"], 8765);
}
