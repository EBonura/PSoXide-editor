//! Run the `cook_check` tool of `psxed-mcp` on one or more projects and return
//! as soon as each answer arrives.
//!
//! The old shell driver held the server's stdin open with a fixed sleep, so
//! every project cost the full sleep (91 s each) whatever the cook took. This
//! speaks the same JSON-RPC lines, waits for each reply, then closes stdin, so
//! a project costs exactly its cook time. The reply line is the one the shell
//! driver printed (the `"id":2` response), byte for byte.
//!
//! ```text
//! cargo run --release -p psxed-mcp --example cook-check -- \
//!     --bin target/release/psxed-mcp --scratch /tmp/cc-scratch \
//!     --out-dir /tmp/cc-out projects/grid-dungeon projects/grid-cave
//! cargo run --release -p psxed-mcp --example cook-check -- --all
//! ```
//!
//! Flags: `--bin PATH` (default `$CARGO_TARGET_DIR/release/psxed-mcp`, else
//! `target/release/psxed-mcp`), `--scratch DIR` (default `<tmp>/cook-check`),
//! `--out-dir DIR` (also write `cc-<project>.json` per project, the same file
//! the shell driver's callers redirected into), `--all` (every directory under
//! `projects/` that holds a `project.ron`), `--timeout SECS` (per project,
//! default 900). Positional arguments are project directories.
//!
//! Output: `== <project>: <reply line>` on stdout per project, in the order
//! given. Exit status is 1 when any project produced no reply (server died or
//! timed out); a reply that reports a failed check is still a reply, so the
//! caller reads the JSON, exactly as before.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// The replies of one server, line by line, from a reader thread.
struct Server {
    child: Child,
    lines: Receiver<String>,
}

impl Server {
    fn start(
        bin: &Path,
        project: &Path,
        scratch: &Path,
        stderr: std::fs::File,
    ) -> Result<Self, String> {
        let mut child = Command::new(bin)
            .arg("--project")
            .arg(project)
            .arg("--scratch")
            .arg(scratch)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(stderr)
            .spawn()
            .map_err(|error| format!("spawn {}: {error}", bin.display()))?;
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let (sender, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        Ok(Self { child, lines })
    }

    fn send(&mut self, line: &str) -> Result<(), String> {
        let stdin = self.child.stdin.as_mut().ok_or("stdin already closed")?;
        writeln!(stdin, "{line}")
            .and_then(|()| stdin.flush())
            .map_err(|error| format!("write to server: {error}"))
    }

    /// The first reply line carrying `"id":<id>`, or an error at the deadline
    /// or when the server closes its output first.
    fn reply(&self, id: u32, deadline: Instant) -> Result<String, String> {
        let needle = format!("\"id\":{id}");
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(line) if line.contains(&needle) => return Ok(line),
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => return Err("timed out".to_string()),
                Err(RecvTimeoutError::Disconnected) => {
                    return Err("the server closed its output before replying".to_string());
                }
            }
        }
    }

    /// Close stdin so the server exits on its own; kill it if it lingers.
    fn finish(mut self) {
        drop(self.child.stdin.take());
        let grace = Instant::now() + Duration::from_secs(5);
        while Instant::now() < grace {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        // Only this driver's own child, never matched by name.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

const INITIALIZE: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"cc","version":"1"}}}"#;
const INITIALIZED: &str = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
const COOK_CHECK: &str = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"cook_check","arguments":{}}}"#;

fn cook_check(
    bin: &Path,
    project: &Path,
    scratch: &Path,
    timeout: Duration,
) -> Result<String, String> {
    let name = project
        .file_name()
        .map_or_else(|| "project".into(), |n| n.to_string_lossy().into_owned());
    let stderr = std::fs::File::create(scratch.join(format!("{name}.stderr.log")))
        .map_err(|error| format!("stderr log: {error}"))?;
    let mut server = Server::start(bin, project, scratch, stderr)?;
    let deadline = Instant::now() + timeout;
    let result = (|| {
        server.send(INITIALIZE)?;
        server.reply(1, deadline)?;
        server.send(INITIALIZED)?;
        server.send(COOK_CHECK)?;
        server.reply(2, deadline)
    })();
    server.finish();
    result
}

fn main() {
    let mut bin: Option<PathBuf> = None;
    let mut scratch = std::env::temp_dir().join("cook-check");
    let mut out_dir: Option<PathBuf> = None;
    let mut all = false;
    let mut timeout = 900u64;
    let mut projects: Vec<PathBuf> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .unwrap_or_else(|| panic!("{name} needs a value"))
        };
        match arg.as_str() {
            "--bin" => bin = Some(PathBuf::from(value("--bin"))),
            "--scratch" => scratch = PathBuf::from(value("--scratch")),
            "--out-dir" => out_dir = Some(PathBuf::from(value("--out-dir"))),
            "--timeout" => timeout = value("--timeout").parse().expect("timeout is seconds"),
            "--all" => all = true,
            flag if flag.starts_with("--") => panic!("unknown flag {flag}"),
            dir => projects.push(PathBuf::from(dir)),
        }
    }
    if all {
        let mut found: Vec<PathBuf> = std::fs::read_dir("projects")
            .expect("read projects/")
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.join("project.ron").is_file())
            .collect();
        found.sort();
        projects.extend(found);
    }
    assert!(
        !projects.is_empty(),
        "no projects: pass directories or --all"
    );
    let bin = bin.unwrap_or_else(|| {
        std::env::var_os("CARGO_TARGET_DIR")
            .map_or_else(|| PathBuf::from("target"), PathBuf::from)
            .join("release/psxed-mcp")
    });
    assert!(
        bin.is_file(),
        "{} is not a file; build psxed-mcp first",
        bin.display()
    );
    std::fs::create_dir_all(&scratch).expect("create scratch directory");
    if let Some(dir) = &out_dir {
        std::fs::create_dir_all(dir).expect("create output directory");
    }
    let started = Instant::now();
    let mut failures = 0u32;
    for project in &projects {
        let name = project
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let begun = Instant::now();
        match cook_check(&bin, project, &scratch, Duration::from_secs(timeout)) {
            Ok(line) => {
                println!("== {name}: {line}");
                if let Some(dir) = &out_dir {
                    std::fs::write(dir.join(format!("cc-{name}.json")), format!("{line}\n"))
                        .expect("write result");
                }
            }
            Err(error) => {
                failures += 1;
                println!("== {name}: NO REPLY ({error})");
            }
        }
        eprintln!("cook-check {name}: {:.1}s", begun.elapsed().as_secs_f64());
    }
    eprintln!("cook-check total {:.1}s", started.elapsed().as_secs_f64());
    if failures > 0 {
        std::process::exit(1);
    }
}
