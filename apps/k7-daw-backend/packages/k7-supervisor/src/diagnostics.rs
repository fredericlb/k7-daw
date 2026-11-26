//! diagnostics.rs — the one-shot commands that touch no state directory and print their answer:
//! `--probe <address>` (a registry's hello) and `--separate <mix> <model> <outDir> [providers]`. A milestone
//! adds its diagnostic as one line in `ALL`; `main` runs the first whose flag leads the arguments.

/// A flag, its usage line, and the run that takes the arguments after the flag and gives the exit code.
pub struct Diagnostic {
    pub flag: &'static str,
    pub usage: &'static str,
    pub run: fn(&[String]) -> i32,
}

/// Every diagnostic, by flag.
pub const ALL: &[Diagnostic] = &[Diagnostic {
    flag: "--separate",
    usage: "--separate <mix> <model> <outDir> [providers]",
    run: crate::separation::cli::main,
}];

/// The usage text, the app's mode and `--config` first.
pub fn usage() -> String {
    let mut u = String::from("usage: k7-supervisor --config <file> [--port <n>]\n");
    for d in ALL {
        u += &format!("       k7-supervisor {}\n", d.usage);
    }
    u + "       k7-supervisor            (driven by an app over stdio)\n"
}
