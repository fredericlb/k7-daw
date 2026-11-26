//! net — the adapters that are up, with their IPv4 addresses and hardware address, from `GetAdaptersAddresses`.

use std::net::Ipv4Addr;

use windows::Win32::Foundation::ERROR_BUFFER_OVERFLOW;
use windows::Win32::NetworkManagement::IpHelper::{
    GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses,
    IF_TYPE_SOFTWARE_LOOPBACK, IP_ADAPTER_ADDRESSES_LH,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Networking::WinSock::{AF_INET, AF_UNSPEC, SOCKADDR_IN};

use super::Adapter;

/// The adapters, or none when Windows would not list them.
pub fn adapters() -> Vec<Adapter> {
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    let mut size = 16 * 1024u32;
    // Words of 8 bytes, since the list is read in place as structs.
    let mut buf = vec![0u64; (size as usize).div_ceil(8)];
    for _ in 0..2 {
        let list = buf.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        // SAFETY: `buf` is `size` bytes, as the call is told.
        let rc = unsafe {
            GetAdaptersAddresses(u32::from(AF_UNSPEC.0), flags, None, Some(list), &mut size)
        };
        if rc == ERROR_BUFFER_OVERFLOW.0 {
            buf = vec![0u64; (size as usize).div_ceil(8)];
            continue;
        }
        // SAFETY: on success `buf` holds the list, linked inside itself.
        return if rc == 0 {
            unsafe { walk(list) }
        } else {
            Vec::new()
        };
    }
    Vec::new()
}

/// # Safety
/// `list` heads a list `GetAdaptersAddresses` filled, which stays alive for the call.
unsafe fn walk(list: *const IP_ADAPTER_ADDRESSES_LH) -> Vec<Adapter> {
    let mut out = Vec::new();
    // SAFETY: every node and address below is inside the filled list.
    unsafe {
        let mut at = list;
        while let Some(a) = at.as_ref() {
            at = a.Next;
            if a.OperStatus != IfOperStatusUp {
                continue;
            }
            let mut f = Adapter {
                name: a.FriendlyName.to_string().unwrap_or_default(),
                loopback: a.IfType == IF_TYPE_SOFTWARE_LOOPBACK,
                addressed: !a.FirstUnicastAddress.is_null(),
                ..Adapter::default()
            };
            let n = (a.PhysicalAddressLength as usize).min(a.PhysicalAddress.len());
            f.mac = a.PhysicalAddress[..n].to_vec();
            let mut u = a.FirstUnicastAddress;
            while let Some(unicast) = u.as_ref() {
                u = unicast.Next;
                let sa = unicast.Address.lpSockaddr;
                if !sa.is_null() && (*sa).sa_family == AF_INET {
                    let sin = &*sa.cast::<SOCKADDR_IN>();
                    f.ipv4
                        .push(Ipv4Addr::from(u32::from_be(sin.sin_addr.S_un.S_addr)));
                }
            }
            out.push(f);
        }
    }
    out
}
