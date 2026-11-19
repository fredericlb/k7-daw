# The Android app

The k7 Android app turns a phone into a k7 machine. Like the [K7 DAW](./82-desktop-app.md) on a computer, it is standalone: it runs its own small k7 backend, the supervisor and the engine, so the phone opens its own k7 and needs no network.

For what the engine and supervisor are, see [Engines](./80-engines.md).

## Requirements

- Android 10 or newer.
- A 64-bit ARM phone (`arm64-v8a`). Other processors are not supported.
- Enough storage for the sampled instruments (see below).

## Setup

1. Install the APK.
2. Open **k7**. The first start unpacks the app's backend for a few seconds.
3. Android asks for **microphone** access. The engine opens the audio device with its inputs, so it can record and monitor. If you refuse, the engine still starts, with output only.
4. k7 opens on the phone's own k7, as its local user. There is no account to sign in to.

The phone downloads **every** sampled instrument in the background as soon as the supervisor starts, because there is no Downloads list to choose from on the phone. That is several gigabytes; use Wi-Fi the first time.

## What works on the phone

| Works | Does not |
|---|---|
| Playing and recording projects through the phone's audio device | Hosting VST3 or LADSPA plugins: there are none on Android |
| Built-in modules: the sampler (SFZ) instruments, Neural Amp Modeler and JavaScript DSP modules, the eurorack modules, Strudel pattern lanes, the mixer, recorder and turntable nodes | The Surge XT synthesizer, which is not built for Android |
| k7, full screen | Opening a plugin's own window |
| Playing the phone's own projects | Running in the background with no page playing |

The engine asks the phone for a 256-frame buffer, or two of the device's bursts if that is larger. If the audio underruns, the engine grows the buffer one step at a time, up to 40 ms, rather than keep crackling.

## Foreground only

The engine lives exactly as long as the app is open:

- The app keeps the screen on while it is in front.
- While a page is **playing**, the app shows a media notification and stays alive with the screen off or with another app in front. Paused, the notification stays, but Android may freeze the app.
- **Leaving the app** (backing out of it, or swiping it away) stops the engine and the phone's backend.
- Rotating the phone does not restart anything.

A stopped engine cannot be restarted inside the same app process. To restart the engine, quit the app and open it again.

## The phone's engine settings

Open k7's **Settings** page. On the phone it starts with the phone's engine:

### This phone's engine

The status line reads, for example, *Engine running on ws://192.168.1.23:8765*. The address is the phone's Wi-Fi address, which is what a browser on the same network would dial. Under it: the engine's state. **Quit** stops the engine and the app.

### Private CA

Only for a git host whose HTTPS certificate is signed by a certificate authority of your own. Android keeps the CAs you installed in its Settings out of an app's reach, so you pick the certificate file (`.pem`, `.crt` or `.der`) here with **Trust a certificate…**. The engine and k7 both trust it. **Forget** returns to Android's CAs only. 

### Paired devices

The phones, tablets and computers allowed to drive this phone's engine over the local network, and the requests waiting for your answer. See [Reaching an engine](./85-connecting-machines.md#pairing-a-device-on-the-local-network).

## The desktop app

When k7 does not load, the app shows its **tray** instead of k7: the same engine status, the pairing requests with **Approve** and **Deny**, the Private CA section and a **Quit** button. If k7 failed to load, the desktop app says why and offers **Retry**.

## The name other people see

Engine pickers show the phone under the device name you set in Android's settings (*About phone*), or its model name if you set none.
