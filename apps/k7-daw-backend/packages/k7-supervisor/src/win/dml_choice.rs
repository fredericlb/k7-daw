//! dml_choice — which DXGI adapter DirectML runs on: the first real GPU with video memory of its own, in
//! DXGI's order (which is ONNX Runtime's index). An integrated GPU shares the CPU's memory and only slows the
//! machine, so a machine with nothing else is refused outright; below 1 GiB dedicated an adapter is integrated
//! whatever it calls itself.

/// What DXGI says of one adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct Adapter {
    /// Its place in DXGI's list, software adapters counted.
    pub index: u32,
    pub name: String,
    pub software: bool,
    pub dedicated_vram: u64,
}

/// Below this much dedicated video memory an adapter is integrated.
pub const DISCRETE_VRAM: u64 = 1 << 30;

#[derive(Debug, Clone, PartialEq)]
pub enum Choice {
    /// A discrete GPU, by its DXGI index.
    Discrete { device_id: u32, name: String },
    /// Only integrated adapters exist, the first named.
    IntegratedOnly { name: String },
    /// DXGI listed nothing: ONNX Runtime picks, on index 0.
    Unknown,
}

pub fn choose(list: &[Adapter]) -> Choice {
    let mut real = list.iter().filter(|a| !a.software);
    let first = real.clone().next();
    if let Some(a) = real.find(|a| a.dedicated_vram >= DISCRETE_VRAM) {
        return Choice::Discrete {
            device_id: a.index,
            name: a.name.clone(),
        };
    }
    match first {
        Some(a) => Choice::IntegratedOnly {
            name: a.name.clone(),
        },
        None => Choice::Unknown,
    }
}
