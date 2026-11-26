//! windows.rs — the decisions Windows' calls feed, run on every platform: the TCP table's rows, the trust
//! store's selection and DirectML's adapter choice. The calls themselves run on a Windows machine only.

use std::collections::BTreeSet;

use k7_supervisor::win::certs::{Candidate, SERVER_AUTH, Usage, select, serves_tls};
use k7_supervisor::win::dml_choice::{Adapter, Choice, DISCRETE_VRAM, choose};
use k7_supervisor::win::tcp::listener_in;

fn table(rows: &[(u16, u32)]) -> Vec<u8> {
    let mut b = (rows.len() as u32).to_ne_bytes().to_vec();
    for (port, pid) in rows {
        // state, local addr, local port (network order in the low 16 bits), remote addr, remote port, pid.
        for w in [2u32, 0, u32::from(port.to_be()), 0, 0, *pid] {
            b.extend(w.to_ne_bytes());
        }
    }
    b
}

#[test]
fn the_tables_listener_is_found_by_its_port_in_network_order() {
    let t = table(&[(80, 10), (18789, 4242), (9000, 7)]);
    assert_eq!(listener_in(&t, 18789), Some(4242));
    assert_eq!(listener_in(&t, 9000), Some(7));
    assert_eq!(listener_in(&t, 81), None);
}

#[test]
fn a_cut_or_empty_table_names_nobody() {
    assert_eq!(listener_in(&[], 80), None);
    assert_eq!(listener_in(&table(&[]), 80), None);
    let t = table(&[(80, 10), (81, 11)]);
    assert_eq!(listener_in(&t[..t.len() - 4], 81), None);
    // A count larger than the bytes never reads past them.
    let mut lie = table(&[(80, 10)]);
    lie[..4].copy_from_slice(&u32::MAX.to_ne_bytes());
    assert_eq!(listener_in(&lie, 99), None);
}

#[test]
fn a_root_is_good_for_tls_unless_its_usages_exclude_server_auth() {
    assert!(serves_tls(&Usage::Unrestricted));
    assert!(serves_tls(&Usage::Only(vec![
        "1.3.6.1.5.5.7.3.4".into(),
        SERVER_AUTH.into()
    ])));
    assert!(!serves_tls(&Usage::Only(vec!["1.3.6.1.5.5.7.3.2".into()])));
    assert!(!serves_tls(&Usage::Only(vec![])));
}

fn root(thumb: u8, der: u8, usage: Usage) -> Candidate {
    Candidate {
        thumbprint: vec![thumb],
        der: vec![der],
        usage,
    }
}

#[test]
fn the_selection_drops_the_disallowed_the_unfit_and_the_repeated() {
    let denied = BTreeSet::from([vec![2u8]]);
    let picked = select(
        vec![
            root(1, 10, Usage::Unrestricted),
            root(2, 20, Usage::Unrestricted),
            root(3, 30, Usage::Only(vec!["1.3.6.1.5.5.7.3.3".into()])),
            root(1, 11, Usage::Unrestricted),
            Candidate {
                thumbprint: vec![],
                der: vec![40],
                usage: Usage::Unrestricted,
            },
            Candidate {
                thumbprint: vec![],
                der: vec![41],
                usage: Usage::Unrestricted,
            },
        ],
        &denied,
    );
    assert_eq!(picked, vec![vec![10], vec![40], vec![41]]);
}

fn gpu(index: u32, name: &str, software: bool, vram: u64) -> Adapter {
    Adapter {
        index,
        name: name.into(),
        software,
        dedicated_vram: vram,
    }
}

#[test]
fn directml_takes_the_first_discrete_gpu_by_its_dxgi_index() {
    let list = [
        gpu(0, "Intel UHD", false, 128 << 20),
        gpu(1, "Basic Render", true, 0),
        gpu(2, "RTX 4070", false, 12 << 30),
        gpu(3, "RX 7600", false, 8 << 30),
    ];
    assert_eq!(
        choose(&list),
        Choice::Discrete {
            device_id: 2,
            name: "RTX 4070".into()
        }
    );
}

#[test]
fn directml_refuses_a_machine_with_only_integrated_gpus() {
    let list = [
        gpu(0, "Basic Render", true, 0),
        gpu(1, "AMD Radeon Graphics", false, DISCRETE_VRAM - 1),
    ];
    assert_eq!(
        choose(&list),
        Choice::IntegratedOnly {
            name: "AMD Radeon Graphics".into()
        }
    );
    assert_eq!(
        choose(&[gpu(0, "X", false, DISCRETE_VRAM)]),
        Choice::Discrete {
            device_id: 0,
            name: "X".into()
        }
    );
    assert_eq!(choose(&[]), Choice::Unknown);
    assert_eq!(choose(&[gpu(0, "Basic", true, 0)]), Choice::Unknown);
}
