// webview_linux.rs — what WebKitGTK needs before k7 works in it: media capture, WebRTC and media source are off
// unless the host turns them on (wry sets only WebGL, Web Audio and the clipboard), and the DMA-BUF renderer paints
// a blank window under NVIDIA's proprietary driver. See docs/webviews.md.

use tauri::WebviewWindow;
use webkit2gtk::{SettingsExt, WebViewExt};

/// The variable that turns the DMA-BUF renderer off: WebKitGTK 2.42 and later draw nothing with the NVIDIA driver.
const DMABUF: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";

/// Whether the renderer must be turned off: NVIDIA's driver is loaded and the person has not chosen either way.
pub fn dmabuf_must_go(already_set: bool, nvidia_loaded: bool) -> bool {
    !already_set && nvidia_loaded
}

/// Before GTK starts: the variable is read when the first webview is made.
pub fn before_gtk() {
    let nvidia = std::path::Path::new("/proc/driver/nvidia").exists();
    if dmabuf_must_go(std::env::var_os(DMABUF).is_some(), nvidia) {
        std::env::set_var(DMABUF, "1");
    }
}

/// The settings k7's page uses: getUserMedia and MediaRecorder for the camera, WebRTC, MSE for playback.
pub fn configure(window: &WebviewWindow) {
    let _ = window.with_webview(|platform| {
        if let Some(settings) = platform.inner().settings() {
            settings.set_enable_media_stream(true);
            settings.set_enable_webrtc(true);
            settings.set_enable_mediasource(true);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_renderer_goes_only_under_nvidia_and_only_unasked() {
        assert!(dmabuf_must_go(false, true));
        assert!(!dmabuf_must_go(true, true));
        assert!(!dmabuf_must_go(false, false));
    }
}
