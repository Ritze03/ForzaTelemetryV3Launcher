# ForzaTelemetryV3 Launcher

A tiny cross-platform (Linux/Windows) GUI launcher for
[ForzaTelemetryV3](https://github.com/Ritze03/ForzaTelemetryV3) built with [egui](https://github.com/emilk/egui).

## What it does

1. On start, shows **Updating..** while it clones (first run) or `git pull`s the tool repo.
2. Lets you pick a **branch** and **Release / Debug** run mode on one page.
3. On **Launch**: checks out the branch, runs `cargo build` (shows **Building..**), then
   spawns `cargo run` detached and quits the launcher.

Your last branch + run mode are remembered. If the saved branch no longer exists, it falls
back to `master`, then `main`, then the first available branch.

## Quick start (Windows)

1. Install the prerequisites with [winget](https://learn.microsoft.com/windows/package-manager/):

   ```powershell
   winget install Rustlang.Rustup
   winget install Git.Git
   ```

   Restart the terminal afterwards so `cargo` and `git` are on `PATH`.

2. Download `ForzaTelemetryV3Launcher-windows.exe` from the
   [releases page](https://github.com/Ritze03/ForzaTelemetryV3Launcher/releases) and run it.

On Linux, install `rust`/`cargo` and `git` via your package manager, then grab
`ForzaTelemetryV3Launcher-linux` from the same [releases page](https://github.com/Ritze03/ForzaTelemetryV3Launcher/releases).

## Paths

Everything lives in the OS data dir (`~/.local/share/ForzaTelemetryV3Launcher` on Linux,
`%APPDATA%\ForzaTelemetryV3Launcher` on Windows):

- `repo/` — the cloned ForzaTelemetryV3 working copy
- `config.txt` — last branch + run mode
- `run.log` — build & run output

## Requirements

`git` and `cargo` must be on `PATH`.

## Build from source

```sh
cargo run                                    # debug
cargo build --release                        # native release
cargo build --release --target x86_64-pc-windows-gnu   # cross-compile to Windows
```
