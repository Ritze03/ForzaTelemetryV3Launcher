#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod theme;

use eframe::egui;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender};

const REPO_URL: &str = "https://github.com/Ritze03/ForzaTelemetryV3.git";
const APP_DIR: &str = "ForzaTelemetryV3Launcher";

fn data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join(APP_DIR)
}
fn repo_dir() -> PathBuf {
    data_dir().join("repo")
}
fn log_path() -> PathBuf {
    data_dir().join("run.log")
}
fn config_path() -> PathBuf {
    data_dir().join("config.txt")
}

/// Returns stdout on success, stderr (or spawn error) on failure.
fn run_git(args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

fn parse_branches(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter(|l| !l.contains("->")) // skip the "origin/HEAD -> origin/master" pointer
        .map(|l| l.trim_start_matches('*').trim().trim_start_matches("origin/").to_string())
        .filter(|l| !l.is_empty() && l != "HEAD")
        .collect()
}

/// Saved branch if it still exists, else first of master/main that exists, else first branch.
fn pick_default_branch(saved: Option<&str>, branches: &[String]) -> String {
    if let Some(s) = saved {
        if branches.iter().any(|b| b == s) {
            return s.to_string();
        }
    }
    for fallback in ["master", "main"] {
        if branches.iter().any(|b| b == fallback) {
            return fallback.to_string();
        }
    }
    branches.first().cloned().unwrap_or_default()
}

/// (branch, release?) read from config; None if missing/malformed.
fn load_config() -> (Option<String>, bool) {
    match fs::read_to_string(config_path()) {
        Ok(s) => {
            let mut lines = s.lines();
            let branch = lines.next().map(|l| l.trim().to_string()).filter(|b| !b.is_empty());
            let release = lines.next().map(|l| l.trim() == "release").unwrap_or(false);
            (branch, release)
        }
        Err(_) => (None, false),
    }
}

fn save_config(branch: &str, release: bool) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&path, format!("{branch}\n{}\n", if release { "release" } else { "debug" }));
}

/// Download fraction from a git `--progress` segment, e.g. "Receiving objects:  45% (…)".
/// Only the "Receiving objects" phase (the actual download) drives the bar, so it fills
/// once 0→100 instead of resetting through git's server-side counting phases.
fn git_download_pct(seg: &str) -> Option<f32> {
    if !seg.contains("Receiving objects") {
        return None;
    }
    let p = seg.find('%')?;
    let digits: String = seg[..p].chars().rev().take_while(|c| c.is_ascii_digit()).collect();
    let n: f32 = digits.chars().rev().collect::<String>().parse().ok()?;
    Some((n / 100.0).clamp(0.0, 1.0))
}

/// Run a git command with `--progress`, streaming download percentage to `on_pct`.
/// git writes progress to stderr, updating in place with `\r`, so we split on `\r`/`\n`.
fn run_git_progress(args: &[&str], on_pct: &mut impl FnMut(f32)) -> Result<(), String> {
    use std::io::{BufReader, Read};
    let mut child = Command::new("git")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to run git: {e}"))?;
    let stderr = child.stderr.take().ok_or("no git stderr pipe")?;
    let mut reader = BufReader::new(stderr); // buffers, so byte-at-a-time reads are cheap
    let (mut all, mut seg, mut b) = (String::new(), String::new(), [0u8; 1]);
    while let Ok(1) = reader.read(&mut b) {
        let c = b[0] as char;
        all.push(c);
        if c == '\r' || c == '\n' {
            if let Some(f) = git_download_pct(&seg) {
                on_pct(f);
            }
            seg.clear();
        } else {
            seg.push(c);
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(all.trim().to_string()); // stderr holds git's error message
    }
    Ok(())
}

/// Clone if missing, optionally fetch, then list branches. Runs on a background
/// thread, reporting clone/fetch download progress via `on_pct`. `fetch` is skipped
/// on startup so the launcher reaches the menu fast; "Launch" fetches on demand.
fn update_repo(fetch: bool, mut on_pct: impl FnMut(f32)) -> Result<Vec<String>, String> {
    let repo = repo_dir();
    if let Some(parent) = repo.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let repo_str = repo.to_string_lossy().into_owned();
    if repo.join(".git").exists() {
        if fetch {
            run_git_progress(&["-C", &repo_str, "fetch", "--prune", "--progress"], &mut on_pct)?;
        }
    } else {
        // first run: clone brings branches with it
        run_git_progress(&["clone", "--progress", REPO_URL, &repo_str], &mut on_pct)?;
    }
    // remote branches, so newly pushed ones show up without a local checkout first
    let listing = run_git(&["-C", &repo_str, "branch", "-r"])?;
    Ok(parse_branches(&listing))
}

/// Delete the local repository clone. Next launch re-clones from scratch.
fn delete_repo() -> Result<(), String> {
    let repo = repo_dir();
    if repo.exists() {
        fs::remove_dir_all(&repo).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn open_log() -> Result<File, String> {
    if let Some(parent) = log_path().parent() {
        let _ = fs::create_dir_all(parent);
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
        .map_err(|e| e.to_string())
}

/// Pull the built binary's path out of a `cargo build --message-format=json` line.
/// Returns None for library artifacts (`"executable":null`) and non-artifact lines.
fn parse_executable(line: &str) -> Option<String> {
    let key = "\"executable\":\"";
    let start = line.find(key)? + key.len();
    let rest = &line[start..];
    let end = rest.find('"')?; // a file path won't contain a literal quote
    Some(rest[..end].replace("\\\\", "\\")) // unescape JSON backslashes (Windows paths)
}

/// Progress fraction denominator when we've never built this profile before.
/// ponytail: ~a clean-build crate count; [`save_build_units`] overwrites it with
/// the real number after the first build, so this only matters once.
const DEFAULT_UNITS: u32 = 280;

fn units_path(release: bool) -> PathBuf {
    data_dir().join(if release { "units_release.txt" } else { "units_debug.txt" })
}
/// Crates compiled in the largest build seen so far, if any.
fn cached_units(release: bool) -> Option<u32> {
    fs::read_to_string(units_path(release)).ok()?.trim().parse().ok().filter(|&n| n > 0)
}
/// Denominator for the progress bar: the cached max, or a default before the first build.
fn total_units(release: bool) -> u32 {
    cached_units(release).unwrap_or(DEFAULT_UNITS)
}
/// Remember the biggest build we've seen, so the bar's denominator tracks worst case.
fn save_build_units(release: bool, n: u32) {
    let best = n.max(cached_units(release).unwrap_or(0));
    if best > 0 {
        let _ = fs::write(units_path(release), best.to_string());
    }
}

/// (optionally fetch) + checkout + cargo build, blocking until it finishes. Streams
/// cargo's stderr to run.log and calls `on_progress(compiled_crate_count)` as each
/// crate compiles. Returns the path to the produced binary.
fn build_streaming(
    branch: &str,
    release: bool,
    update: bool,
    mut on_progress: impl FnMut(u32),
) -> Result<String, String> {
    use std::io::{BufRead, BufReader, Write};
    let repo = repo_dir();
    let repo_str = repo.to_string_lossy().into_owned();
    if update {
        run_git(&["-C", &repo_str, "fetch", "--prune"])?;
    }
    // -B creates-or-resets the local branch to match the remote (handles newly pushed branches
    // and stale local ones). ponytail: repo is consume-only, so discarding local state is fine.
    let start = format!("origin/{branch}");
    run_git(&["-C", &repo_str, "checkout", "-B", branch, &start])?;

    let mut log = open_log()?;
    let mut cmd = Command::new("cargo");
    cmd.arg("build");
    if release {
        cmd.arg("--release");
    }
    // cargo writes its status ("Compiling foo …") and diagnostics to stderr; stdout is empty
    // for a plain build. Pipe stderr so we can count crates as they finish.
    let mut child = cmd
        .current_dir(&repo)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    let stderr = child.stderr.take().ok_or("no stderr pipe")?;
    let mut compiled = 0u32;
    for line in BufReader::new(stderr).lines() {
        let Ok(line) = line else { break };
        let _ = writeln!(log, "{line}");
        if line.trim_start().starts_with("Compiling ") {
            compiled += 1;
            on_progress(compiled);
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("build failed (exit {status}); see {}", log_path().display()));
    }
    save_build_units(release, compiled);
    locate_exe(&repo, release)
}

/// Build on a background thread, reporting progress + completion over `tx`.
fn run_build(branch: String, release: bool, update: bool, ctx: egui::Context, tx: Sender<BuildMsg>) {
    let res = build_streaming(&branch, release, update, |n| {
        let _ = tx.send(BuildMsg::Progress(n));
        ctx.request_repaint();
    });
    let _ = tx.send(BuildMsg::Done(res));
    ctx.request_repaint();
}

/// Ask cargo for the binary path (cached/instant after the real build above).
fn locate_exe(repo: &Path, release: bool) -> Result<String, String> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build").arg("--message-format=json");
    if release {
        cmd.arg("--release");
    }
    let out = cmd.current_dir(repo).output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .rev()
        .find_map(parse_executable)
        .ok_or_else(|| "cargo build produced no executable".to_string())
}

/// Spawn the built binary detached, output to run.log. Returns on spawn error only.
/// Runs the exe directly instead of `cargo run`, so no `cargo` console window appears
/// on Windows (CREATE_NO_WINDOW) — see issue #1.
fn spawn_run(exe: &str) -> Result<(), String> {
    let log = open_log()?;
    let log_err = log.try_clone().map_err(|e| e.to_string())?;
    let mut cmd = Command::new(exe);
    cmd.current_dir(repo_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err));

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: run a console app with its console window hidden.
        cmd.creation_flags(0x0800_0000);
    }

    cmd.spawn().map_err(|e| e.to_string())?;
    // ponytail: child outlives the launcher on normal exit; no setsid needed for a windowed app.
    Ok(())
}

/// Centred busy view: spinner, a title-sized caption, and an optional progress bar
/// (with a `NN%` label). `frac` is `None` while there's nothing to measure yet.
fn progress_view(ui: &mut egui::Ui, title: &str, frac: Option<f32>) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.28);
        ui.spinner();
        ui.add_space(6.0);
        // Same size as the title-bar wordmark.
        ui.label(egui::RichText::new(title).color(theme::ACCENT).size(16.0).strong());
        if let Some(f) = frac {
            let f = f.min(0.99); // hold at 99% until the operation actually finishes
            ui.add_space(10.0);
            // Paint the % centred over the bar ourselves — ProgressBar's own text is
            // left-anchored, and white reads on both the accent fill and the dark track.
            let bar = ui.add(
                egui::ProgressBar::new(f)
                    .desired_width(ui.available_width() * 0.8)
                    .fill(theme::ACCENT),
            );
            ui.painter().text(
                bar.rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{}%", (f * 100.0).round() as u32),
                egui::FontId::proportional(12.0),
                egui::Color32::WHITE,
            );
        }
    });
}

/// A radio option stacked over a dim one-line description.
fn radio_desc(ui: &mut egui::Ui, value: &mut bool, this: bool, label: &str, desc: &str) {
    ui.horizontal(|ui| {
        ui.radio_value(value, this, label);
        ui.label(egui::RichText::new(desc).size(11.0).color(theme::DIM));
    });
}

/// Messages from the build thread to the UI.
enum BuildMsg {
    Progress(u32), // crates compiled so far
    Done(Result<String, String>),
}

/// Messages from the update (clone/fetch) thread to the UI.
enum UpdateMsg {
    Progress(f32), // download fraction 0..1
    Done(Result<Vec<String>, String>),
}

enum State {
    Updating { rx: Receiver<UpdateMsg>, frac: f32 },
    Ready { branches: Vec<String> },
    Building { rx: Receiver<BuildMsg>, compiled: u32, total: u32 },
    Error(String),
}

struct App {
    state: State,
    ctx: egui::Context,
    selected_branch: String,
    release: bool,
    saved_branch: Option<String>,
}

fn spawn_update(ctx: egui::Context) -> Receiver<UpdateMsg> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        // startup: list local branches, don't wait on a fetch (only clones if missing)
        let res = update_repo(false, |f| {
            let _ = tx.send(UpdateMsg::Progress(f));
            ctx.request_repaint();
        });
        let _ = tx.send(UpdateMsg::Done(res));
        ctx.request_repaint();
    });
    rx
}

impl App {
    /// Start a build on a background thread. `update` fetches latest first.
    fn start_build(&mut self, update: bool) {
        save_config(&self.selected_branch, self.release);
        let (tx, rx) = std::sync::mpsc::channel();
        let branch = self.selected_branch.clone();
        let release = self.release;
        let ctx = self.ctx.clone();
        std::thread::spawn(move || run_build(branch, release, update, ctx, tx));
        self.state = State::Building { rx, compiled: 0, total: total_units(release) };
    }
}

impl App {
    fn new(ctx: egui::Context) -> Self {
        theme::install_fonts(&ctx);
        theme::apply(&ctx);
        let (saved_branch, release) = load_config();
        let state = State::Updating { rx: spawn_update(ctx.clone()), frac: 0.0 };
        Self {
            state,
            ctx,
            selected_branch: String::new(),
            release,
            saved_branch,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Drain update (clone/fetch) progress without holding a borrow across the mutation.
        if let State::Updating { rx, .. } = &self.state {
            let mut latest = None;
            let mut done = None;
            loop {
                match rx.try_recv() {
                    Ok(UpdateMsg::Progress(f)) => latest = Some(f),
                    Ok(UpdateMsg::Done(res)) => {
                        done = Some(res);
                        break;
                    }
                    Err(_) => break,
                }
            }
            if let (Some(f), State::Updating { frac, .. }) = (latest, &mut self.state) {
                *frac = f;
            }
            if let Some(res) = done {
                match res {
                    Ok(branches) => {
                        self.selected_branch =
                            pick_default_branch(self.saved_branch.as_deref(), &branches);
                        self.state = State::Ready { branches };
                    }
                    Err(e) => self.state = State::Error(e),
                }
            }
        }

        // Drain build progress without holding a borrow across the state mutation.
        if let State::Building { rx, .. } = &self.state {
            let mut latest = None;
            let mut done = None;
            loop {
                match rx.try_recv() {
                    Ok(BuildMsg::Progress(n)) => latest = Some(n),
                    Ok(BuildMsg::Done(res)) => {
                        done = Some(res);
                        break;
                    }
                    Err(_) => break, // empty or disconnected
                }
            }
            if let (Some(n), State::Building { compiled, .. }) = (latest, &mut self.state) {
                *compiled = n;
            }
            if let Some(res) = done {
                self.state = match res {
                    Ok(exe) => match spawn_run(&exe) {
                        Ok(()) => std::process::exit(0),
                        Err(e) => State::Error(e),
                    },
                    Err(e) => State::Error(e),
                };
            }
        }

        let head = egui::Frame::side_top_panel(&ctx.style()).fill(theme::HEAD);
        egui::TopBottomPanel::top("title_bar").frame(head).show(ctx, |ui| {
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new("Forza Telemetry V3 Launcher")
                    .color(theme::ACCENT)
                    .size(16.0)
                    .strong(),
            );
            ui.add_space(2.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| match &self.state {
            // Only show the bar once a download actually reports progress (a cached repo
            // that skips fetching finishes near-instantly with nothing to measure).
            State::Updating { frac, .. } => {
                progress_view(ui, "UPDATING", (*frac > 0.0).then_some(*frac));
            }
            State::Building { compiled, total, .. } => {
                progress_view(ui, "BUILDING", Some(*compiled as f32 / *total as f32));
            }
            State::Error(e) => {
                ui.label(theme::section_label("Update failed"));
                ui.add_space(4.0);
                ui.colored_label(theme::DANGER, e);
            }
            State::Ready { branches } => {
                let branches = branches.clone();
                ui.spacing_mut().item_spacing.y = 0.0; // card() owns the 8px inter-card gap
                // No extra top space: the panel's own inner margin already spaces the card
                // from the top, matching the (equal) left/right margins.

                theme::card(ui, "Configuration", |ui| {
                    ui.label("Branch:");
                    egui::ComboBox::from_id_salt("branch")
                        .selected_text(&self.selected_branch)
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for b in &branches {
                                ui.selectable_value(&mut self.selected_branch, b.clone(), b);
                            }
                        });

                    ui.add_space(4.0);
                    ui.label("Run type:");
                    radio_desc(ui, &mut self.release, false, "Debug", "Launches quicker");
                    radio_desc(ui, &mut self.release, true, "Release", "Performs better");
                });

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    // Delete sizes to its text (pinned right); Launch fills the rest (left).
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let del = ui.add(theme::danger_button("Re-init Repository"));
                        if del.clicked() {
                            match delete_repo() {
                                Ok(()) => {
                                    self.state = State::Updating {
                                        rx: spawn_update(self.ctx.clone()),
                                        frac: 0.0,
                                    }
                                }
                                Err(e) => self.state = State::Error(e),
                            }
                        }
                        let size = egui::vec2(ui.available_width(), del.rect.height());
                        if ui.add_sized(size, theme::primary_button("Launch")).clicked() {
                            self.start_build(true);
                        }
                    });
                });
            }
        });
    }
}

/// --last-config: update, build, and launch the last-configured branch + run type, no GUI.
fn run_last_config() -> Result<(), String> {
    let (saved, release) = load_config();
    let branches = update_repo(true, |_| {})?; // headless auto-launch: refresh before building
    let branch = pick_default_branch(saved.as_deref(), &branches);
    let exe = build_streaming(&branch, release, false, |_| {})?;
    spawn_run(&exe)
}

fn main() -> eframe::Result<()> {
    if std::env::args().any(|a| a == "--last-config") {
        if let Err(e) = run_last_config() {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([320.0, 244.0])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "ForzaTelemetryV3 Launcher",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc.egui_ctx.clone())))),
    )
}

#[cfg(test)]
mod tests {
    use super::{git_download_pct, parse_executable, pick_default_branch};

    #[test]
    fn git_progress_parsing() {
        // real "Receiving objects" segment drives the bar
        assert_eq!(git_download_pct("Receiving objects:  99% (1715/1720), 338 MiB"), Some(0.99));
        assert_eq!(git_download_pct("Receiving objects: 100% (1720/1720), done."), Some(1.0));
        // other phases are ignored so the bar doesn't reset
        assert_eq!(git_download_pct("remote: Counting objects:  50% (5/10)"), None);
        assert_eq!(git_download_pct("Resolving deltas:  30% (30/100)"), None);
        assert_eq!(git_download_pct("Cloning into 'repo'..."), None);
    }

    #[test]
    fn executable_parsing() {
        // a bin artifact
        let line = r#"{"reason":"compiler-artifact","target":{"kind":["bin"]},"executable":"/home/u/repo/target/release/app"}"#;
        assert_eq!(parse_executable(line).as_deref(), Some("/home/u/repo/target/release/app"));
        // windows path with escaped backslashes
        let win = r#"{"executable":"C:\\repo\\target\\release\\app.exe"}"#;
        assert_eq!(parse_executable(win).as_deref(), Some(r"C:\repo\target\release\app.exe"));
        // library artifact -> no executable
        assert_eq!(parse_executable(r#"{"executable":null}"#), None);
        // unrelated line
        assert_eq!(parse_executable(r#"{"reason":"build-finished"}"#), None);
    }

    #[test]
    fn default_branch_logic() {
        let branches = vec!["master".to_string(), "main".to_string(), "dev".to_string()];
        // saved exists
        assert_eq!(pick_default_branch(Some("dev"), &branches), "dev");
        // saved missing -> master
        assert_eq!(pick_default_branch(Some("gone"), &branches), "master");
        // no master -> main
        let no_master = vec!["main".to_string(), "dev".to_string()];
        assert_eq!(pick_default_branch(Some("gone"), &no_master), "main");
        // none of the above -> first branch
        let neither = vec!["dev".to_string(), "feature".to_string()];
        assert_eq!(pick_default_branch(Some("gone"), &neither), "dev");
    }
}
