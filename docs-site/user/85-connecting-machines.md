# Reaching an engine

An engine is useful once your browser can reach it. This page explains how, on the same computer, on a phone and across a local network.

## The engine of the device

The tray and the phone app list their engine at the k7 they run, and nowhere else: the engine is the device's own, and no server holds a list of engines. The command-line engine still sends a heartbeat to the registries named by `registryUrls` in its config file ([The command-line engine](./81-command-line-engine.md)); it ships as it was, with that key kept for a later rework.

## How the browser reaches the engine

When k7 measures an engine, it tries every way to reach it **at the same time** and keeps the fastest that answers:

| Path | When it is offered | What it costs |
|---|---|---|
| **This machine** (`127.0.0.1`) | Always | Nothing: the fastest path, when the engine runs on the computer the browser runs on |
| **Your local network** (the engine's LAN address) | Only when k7 itself is opened over `http://`. A page opened over `https://` may not open an unencrypted connection to a LAN address, so browsers block it. | Pairing, the first time (see below) |

You do not choose a path; k7 shows you the result as one latency figure.

### Which web pages may connect directly

The engine only accepts a direct connection from a page served by a site it knows: a page on this same computer (`127.0.0.1` or `localhost`). Pages from any other website are refused at once, so a site you happen to visit cannot drive your engine. Programs that are not browsers, such as the supervisor itself, are not affected.

## Pairing a device on the local network

An engine trusts any program on its own machine. A device that connects **directly over the local network**, such as a tablet on the same Wi-Fi opening k7 over `http://`, must be approved once on the engine's machine.

On the device:

1. The engine picker shows *This device isn't paired with <machine>* with an explanation: over the local network the engine only answers devices its machine has approved.
2. Click **Pair this device**.
3. The device shows a six-digit code: *Approve on the machine running the engine — check it shows this code*.

On the engine's machine, a card appears: *<device> wants to pair. From <address>. Approve only if that device shows this code*, with **Approve** and **Deny**. The card appears in k7 when a page is open on that machine, in the Android app's tray, and in **Settings → Paired devices**.

Check that the codes match before approving: anyone on the network can ask, under any name. The device then shows the engine as usable and k7 continues. If you deny, the device says *The machine refused this device*. If nobody answers within two minutes, the request expires (*Nobody answered within two minutes: the request expired*). At most eight requests can wait at once.

The device remembers its approval in the browser. A private window forgets it when it closes.

### Managing paired devices

**Settings → Paired devices** lists every device allowed to drive this machine's engine, with when it was paired and last seen, and a **Revoke** button. Revoking disconnects the device at once; it must ask again to reconnect. The list can only be managed from the engine's own machine; elsewhere the section says *Manage paired devices from the machine running the engine.*

## A git host with a private certificate

If your git host's HTTPS certificate is signed by your own certificate authority, k7 must trust that authority, or a connect, a push and a pull fail.

- Command-line engine: point `extraCaCerts` at a PEM file of the authority ([config keys](./81-command-line-engine.md#every-key)).
- The tray: k7 trusts the operating system's certificate store, so install the authority there.
- Android app: **Settings → Private CA → Trust a certificate…** ([The Android app](./83-android-app.md#private-ca)).
