# Engines: what plays your music

k7 runs in a browser, but the browser does not make the sound. The sound comes from an **engine**: a program that runs on a machine with an audio interface. That machine might be the laptop you are using, a desktop in the studio, a phone, or the k7 server itself. This page explains what an engine is, how k7 picks one for a project, and what the engine badges in the header mean.

## The two programs on an engine machine

Two programs run on every machine that plays k7 projects:

| Program | What it does for you |
|---|---|
| **The engine** (`k7-runtime`) | Plays the project. It opens your audio interface, hosts the plugins and instruments, plays the clips and patterns, records takes, and sends the playhead and meters back to every browser that is watching. It never reads your song files. k7 compiles the project and sends the result to the engine. |
| **The supervisor** (`k7-supervisor`) | Looks after the engine. It starts the engine, restarts it if it crashes, picks the audio interface you prefer, downloads the sampled instruments you ask for, and lists the engine at the studios you connect it to. It also does the jobs a studio asks of the machine, such as separating a song into stems. |

You never start the engine by hand. You start the supervisor, and it starts the engine. You can get them in four ways:

| Where | How you get it | Page |
|---|---|---|
| A desktop or laptop | **K7 DAW** runs k7, the supervisor and the engine on the computer, and shows k7 in its window | [K7 DAW](./82-desktop-app.md) |
| A desktop, laptop or small server, with no window | The **command-line engine**: `k7-supervisor --config engine.json` | [The command-line engine](./81-command-line-engine.md) |
| An Android phone | The **k7 Android app** | [The Android app](./83-android-app.md) |

One machine runs one supervisor. If you start a second one, it stops with `an engine supervisor is already running (pid …) — quit it first`. The desktop app and the command-line engine run the same supervisor in the same `~/.k7` folder, so a computer runs one or the other.

## The device's engine

The engine of a device is the one k7 on that device runs, and k7 lists it. With the desktop app this happens automatically for the k7 on your computer, and on the phone for the phone's own k7. The command-line engine can still be listed at registries named by the `registryUrls` key. [Reaching an engine](./85-connecting-machines.md) explains how the browser reaches the engine.

## How k7 finds the engine for a project

k7 always plays on the engine of this device. When you open a project, it opens a connection to that engine, asks it who it is, and times the round trip. The engine also reports its audio buffer size, which adds latency. The sum of the two is the engine's latency for you.

The project opens beside this search. If no engine answers, the project stays open and silent, and the bar's engine button says so; open it and choose **Engine settings** to start the engine. The button looks again for the engine each time it opens.

### Latency bands

Every latency is shown as a band and a number, for example `Q1 · 12 ms`:

| Band | Latency | What it is good for |
|---|---|---|
| **Q1** (green) | under 20 ms | Playing live: a keyboard, a pad, a turntable |
| **Q2** (plain) | 20 to 50 ms | Working: editing, mixing, arranging |
| **Q3** (amber) | 50 ms and more | Listening rather than performing |

Hover over the badge to see the split between the network and the audio interface, for example *8 ms of network + 11 ms of audio interface*. The audio part is twice the buffer period. If it shows `latency unknown`, the engine has not opened an audio device yet.

## The engine in the header

Once a project is open, the header shows the engine to the right of the project name:

- **The engine switch** shows the engine's name. Click it to open a menu with every engine that answers, each with its latency. Pick another engine to move the project to it. Your files stay where they are, on the server, so you lose only the playing position. Engines that someone else is using show **in use** and cannot be picked.
- **A warning triangle** next to the name means the engine is missing plugins the project uses. Hover to read, for example, *80% compatible — this engine is missing: Diva, Valhalla Room*.
- **The connection light** is green while the browser is connected to the engine and red while it is not.
- **The latency badge** shows the band and milliseconds for this session.
- **The load figure** shows the engine's memory and how much of each audio block its plugins use.

When you open a project, k7 shows progress screens while the engine gets ready: *Copying the project to the engine* (the audio files are copied to the engine's machine first, so nothing is downloaded while it plays), *Loading the project on the engine* (the engine builds the song and loads its plugins) and *Warming up* (the engine plays the first half-second silently, so the first real play starts at once).

## One project, one engine

An engine plays one project at a time for one person, and a project plays on one engine at a time. Both rules hold across people and across browser tabs:

- If someone opens your project somewhere else, your tab stops driving it and shows **This project was opened elsewhere**. Nothing you saved is lost. Open it again from Projects to take it back.
- If someone opens a different project on the same engine, your tab shows **Another project took this engine**. Open your project again, or pick a different engine so both keep running.

The newcomer always wins. A tab that goes to sleep releases its engine, so an idle tab does not keep a machine busy.

## Sampled instruments

Some modules use large sampled instruments, such as a grand piano or a drum kit. These are not part of the engine download. The supervisor fetches them on request and keeps them in the engine's state folder. If a project names an instrument that is not on the engine's machine, k7 says so: *This module needs the sampled instrument "…", which is not on the engine's machine.* Download it from the desktop app's engine page (**Engine Settings…** in the desktop app menu), or list it under `instruments` in the command-line engine's config file. The instruments available are:

| Name | Label | Download size |
|---|---|---|
| `salamander-grand-piano` | Salamander Grand Piano | about 750 MB |
| `nbo-stereo-rhodes` | Stereo Rhodes | about 74 MB |
| `nbo-wurt` | Wurlitzer | about 8 MB |
| `lithalean-clavinet` | Clavinet | about 57 MB |
| `naked-drums` | Naked Drums | about 1.3 GB |
| `big-rusty-drums` | Big Rusty Drums | about 620 MB |
| `retro-drums` | Retro Drums | about 2 MB |

Each download is checked against a fixed checksum before it is unpacked. Downloads run one at a time.

## Where to go next

- [The command-line engine](./81-command-line-engine.md)
- [K7 DAW, k7 on your computer](./82-desktop-app.md)
- [The Android app](./83-android-app.md)
- [Audio interfaces, MIDI and plugins](./84-audio-midi-plugins.md)
- [Reaching an engine](./85-connecting-machines.md)
- [Machine jobs](./86-server-engine-and-jobs.md)
- [Troubleshooting engines](./87-troubleshooting.md)
