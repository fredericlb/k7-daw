// quality.rs — how good a link to an engine is, as one number and a band: milliseconds from a gesture to a sound.
// A reader would get wrong that an audio latency of 0 means "not measured", never "fast".

use serde::{Deserialize, Serialize};

/// The bands: Q1 is playable, Q2 workable, Q3 for listening rather than performing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Quality {
    Q1,
    Q2,
    Q3,
}

/// The band a latency falls in; Q1 < 20 <= Q2 < 50 <= Q3, a boundary belonging to the slower band.
pub fn quality_of(latency_ms: f64) -> Quality {
    if latency_ms < 20.0 {
        Quality::Q1
    } else if latency_ms < 50.0 {
        Quality::Q2
    } else {
        Quality::Q3
    }
}

/// Twice the buffer period, a floor; 0 when either figure is missing.
pub fn audio_latency_ms(buffer_frames: f64, sample_rate: f64) -> f64 {
    if buffer_frames > 0.0 && sample_rate > 0.0 {
        2.0 * buffer_frames * 1000.0 / sample_rate
    } else {
        0.0
    }
}

/// One engine's link, as measured.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct EngineLatency {
    /// The whoami round trip, ms.
    pub network_ms: f64,
    /// What the interface adds, ms; 0 when the engine reported no open device.
    pub audio_ms: f64,
    /// Their sum, which the band is read off.
    pub total_ms: f64,
    pub quality: Quality,
}

pub fn engine_latency(network_ms: f64, buffer_frames: f64, sample_rate: f64) -> EngineLatency {
    let audio_ms = audio_latency_ms(buffer_frames, sample_rate);
    let total_ms = network_ms + audio_ms;
    EngineLatency {
        network_ms,
        audio_ms,
        total_ms,
        quality: quality_of(total_ms),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_at_20_and_50_ms() {
        assert_eq!(quality_of(0.0), Quality::Q1);
        assert_eq!(quality_of(19.9), Quality::Q1);
        assert_eq!(quality_of(20.0), Quality::Q2);
        assert_eq!(quality_of(49.9), Quality::Q2);
        assert_eq!(quality_of(50.0), Quality::Q3);
        assert_eq!(quality_of(1000.0), Quality::Q3);
    }

    #[test]
    fn counts_a_buffer_in_and_a_buffer_out() {
        assert!((audio_latency_ms(256.0, 48000.0) - 10.667).abs() < 0.01);
        assert!((audio_latency_ms(128.0, 48000.0) - 5.333).abs() < 0.01);
        assert!((audio_latency_ms(2048.0, 44100.0) - 92.88).abs() < 0.05);
    }

    #[test]
    fn reads_a_device_that_has_not_opened_as_unmeasured() {
        assert_eq!(audio_latency_ms(0.0, 48000.0), 0.0);
        assert_eq!(audio_latency_ms(256.0, 0.0), 0.0);
        assert_eq!(audio_latency_ms(f64::NAN, 48000.0), 0.0);
    }

    #[test]
    fn adds_the_round_trip_to_the_interface_and_bands_the_sum() {
        let near = engine_latency(2.0, 256.0, 48000.0);
        assert!((near.total_ms - 12.67).abs() < 0.05);
        assert_eq!(near.quality, Quality::Q1);
        assert_eq!(engine_latency(2.0, 2048.0, 44100.0).quality, Quality::Q3);
        assert_eq!(engine_latency(40.0, 128.0, 48000.0).quality, Quality::Q2);
    }
}
