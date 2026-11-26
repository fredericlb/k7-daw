//! tcp — the pid listening on a port, read from the TCP table Windows keeps (`GetExtendedTcpTable`), where
//! the other systems ask `lsof`. What a reader gets wrong: a row's port is a `u32` whose LOW 16 bits are in
//! network order, so it is read as a big-endian `u16`, never as the whole word.

/// `MIB_TCPTABLE_OWNER_PID`: a count, then rows of six words (state, local addr, local port, remote addr, remote port, pid).
const ROW_BYTES: usize = 24;

/// The owning pid of the first listener on `port` in a table's bytes; None when none, or when the bytes are cut short.
pub fn listener_in(table: &[u8], port: u16) -> Option<u32> {
    let word = |bytes: &[u8], at: usize| {
        let w = bytes.get(at * 4..at * 4 + 4)?;
        Some(u32::from_ne_bytes([w[0], w[1], w[2], w[3]]))
    };
    let count = word(table, 0)? as usize;
    for i in 0..count {
        let row = table.get(4 + i * ROW_BYTES..4 + (i + 1) * ROW_BYTES)?;
        if u16::from_be(word(row, 2)? as u16) == port {
            return word(row, 5);
        }
    }
    None
}

/// The pid listening on `port` over IPv4, asked of the TCP table; None when none.
#[cfg(windows)]
pub fn listener_pid(port: u16) -> Option<u32> {
    use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
    use windows::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, TCP_TABLE_OWNER_PID_LISTENER,
    };
    use windows::Win32::Networking::WinSock::AF_INET;

    // Words, not bytes: the table is read in place as 32-bit fields, so its buffer is 4-aligned.
    let mut buf: Vec<u32> = Vec::new();
    let mut size = 0u32;
    let mut rc = ERROR_INSUFFICIENT_BUFFER.0;
    // The table may grow between the sizing call and the read, so the read is retried with the new size.
    for _ in 0..3 {
        if rc != ERROR_INSUFFICIENT_BUFFER.0 {
            break;
        }
        buf.resize((size as usize).div_ceil(4), 0);
        let at = (!buf.is_empty()).then(|| buf.as_mut_ptr().cast());
        // SAFETY: `at` is null or `size` bytes of writable memory, which the call fills.
        rc = unsafe {
            GetExtendedTcpTable(
                at,
                &mut size,
                false,
                u32::from(AF_INET.0),
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
    }
    if rc != NO_ERROR.0 || buf.is_empty() {
        return None;
    }
    // SAFETY: `buf` holds `size` initialised bytes, as the call wrote them.
    let bytes = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<u8>(), buf.len() * 4) };
    listener_in(bytes, port)
}
