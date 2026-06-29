#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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

/// Clone-or-pull, then list branches. Runs on a background thread.
fn update_repo() -> Result<Vec<String>, String> {
    let repo = repo_dir();
    if let Some(parent) = repo.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let repo_str = repo.to_string_lossy().into_owned();
    if repo.join(".git").exists() {
        run_git(&["-C", &repo_str, "fetch", "--prune"])?;
    } else {
        run_git(&["clone", REPO_URL, &repo_str])?;
    }
    // remote branches, so newly pushed ones show up without a local checkout first
    let listing = run_git(&["-C", &repo_str, "branch", "-r"])?;
    Ok(parse_branches(&listing))
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

/// checkout + cargo build, blocking until the build finishes. Output to run.log.
/// Returns the path to the produced binary.
fn build(branch: &str, release: bool) -> Result<String, String> {
    let repo = repo_dir();
    let repo_str = repo.to_string_lossy().into_owned();
    // -B creates-or-resets the local branch to match the remote (handles newly pushed branches
    // and stale local ones). ponytail: repo is consume-only, so discarding local state is fine.
    let start = format!("origin/{branch}");
    run_git(&["-C", &repo_str, "checkout", "-B", branch, &start])?;

    let log = open_log()?;
    let log_err = log.try_clone().map_err(|e| e.to_string())?;
    let mut cmd = Command::new("cargo");
    cmd.arg("build");
    if release {
        cmd.arg("--release");
    }
    let status = cmd
        .current_dir(&repo)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err))
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("build failed (exit {status}); see {}", log_path().display()));
    }
    locate_exe(&repo, release)
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

enum State {
    Updating(Receiver<Result<Vec<String>, String>>),
    Ready { branches: Vec<String> },
    Building(Receiver<Result<String, String>>),
    Error(String),
}

struct App {
    state: State,
    ctx: egui::Context,
    selected_branch: String,
    release: bool,
    saved_branch: Option<String>,
}

fn spawn_update(ctx: egui::Context) -> Receiver<Result<Vec<String>, String>> {
    let (tx, rx): (Sender<_>, Receiver<_>) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(update_repo());
        ctx.request_repaint();
    });
    rx
}

impl App {
    fn new(ctx: egui::Context) -> Self {
        let (saved_branch, release) = load_config();
        let state = State::Updating(spawn_update(ctx.clone()));
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
        match &self.state {
            State::Updating(rx) => {
                if let Ok(result) = rx.try_recv() {
                    match result {
                        Ok(branches) => {
                            self.selected_branch =
                                pick_default_branch(self.saved_branch.as_deref(), &branches);
                            self.state = State::Ready { branches };
                        }
                        Err(e) => self.state = State::Error(e),
                    }
                }
            }
            State::Building(rx) => {
                if let Ok(result) = rx.try_recv() {
                    match result {
                        Ok(exe) => match spawn_run(&exe) {
                            Ok(()) => std::process::exit(0),
                            Err(e) => self.state = State::Error(e),
                        },
                        Err(e) => self.state = State::Error(e),
                    }
                }
            }
            _ => {}
        }

        egui::CentralPanel::default().show(ctx, |ui| match &self.state {
            State::Updating(_) => {
                ui.heading("Updating..");
                ui.spinner();
            }
            State::Building(_) => {
                ui.heading("Building..");
                ui.spinner();
            }
            State::Error(e) => {
                ui.heading("Update failed");
                ui.colored_label(egui::Color32::LIGHT_RED, e);
            }
            State::Ready { branches } => {
                let branches = branches.clone();
                ui.heading("ForzaTelemetryV3 Launcher");
                ui.add_space(8.0);

                ui.label("Branch:");
                egui::ComboBox::from_id_salt("branch")
                    .selected_text(&self.selected_branch)
                    .show_ui(ui, |ui| {
                        for b in &branches {
                            ui.selectable_value(&mut self.selected_branch, b.clone(), b);
                        }
                    });

                ui.add_space(8.0);
                ui.label("Run type:");
                ui.radio_value(&mut self.release, false, "Debug");
                ui.radio_value(&mut self.release, true, "Release");

                ui.add_space(12.0);
                if ui.button("Launch").clicked() {
                    save_config(&self.selected_branch, self.release);
                    let (tx, rx): (Sender<_>, Receiver<_>) = std::sync::mpsc::channel();
                    let branch = self.selected_branch.clone();
                    let release = self.release;
                    let ctx = self.ctx.clone();
                    std::thread::spawn(move || {
                        let _ = tx.send(build(&branch, release));
                        ctx.request_repaint();
                    });
                    self.state = State::Building(rx);
                }
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([290.0, 190.0])
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
    use super::{parse_executable, pick_default_branch};

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
