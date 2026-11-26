//! dxgi — the adapters DXGI lists, in its order, which is the index DirectML takes.

use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIFactory1,
};

use super::dml_choice::Adapter;

/// Every adapter, software ones included (they keep their index); none when DXGI cannot start.
pub fn adapters() -> Vec<Adapter> {
    // SAFETY: plain COM calls on a factory this function owns.
    unsafe {
        let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut index = 0u32;
        while let Ok(a) = factory.EnumAdapters1(index) {
            if let Ok(d) = a.GetDesc1() {
                let end = d
                    .Description
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(d.Description.len());
                out.push(Adapter {
                    index,
                    name: String::from_utf16_lossy(&d.Description[..end]),
                    software: d.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0,
                    dedicated_vram: d.DedicatedVideoMemory as u64,
                });
            }
            index += 1;
        }
        out
    }
}
