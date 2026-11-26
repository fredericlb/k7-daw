//! probe — `k7-supervisor --probe <address>`: asks a studio's registry whether it is one, prints the answer and
//! touches no state directory. Exit 0 when it is, 1 when it is not, 2 for an address that is no web address.

use crate::{registry, urls};

/// The run of `--probe`, given the arguments after the flag.
pub fn run(args: &[String]) -> i32 {
    let [address] = args else {
        eprint!("{}", crate::diagnostics::usage());
        return 2;
    };
    let Some(origin) = urls::normalise_user_origin(address) else {
        println!("{address} is not a web address");
        return 2;
    };
    let reg = urls::engine_registry_url(&origin);
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        println!("{reg}: no runtime to ask with");
        return 1;
    };
    let p = rt.block_on(registry::probe(&reg));
    println!("{reg}: {}", if p.ok { "an k7 registry" } else { &p.why });
    i32::from(!p.ok)
}
