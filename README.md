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
