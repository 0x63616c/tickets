//! A fresh, migrated, healthy local Worker with no authentication or domain writes.
//! Wrangler is local-substitutable: tests inject private executables and deadlines.
use serde_json::Value;
use sha2::Sha256;
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub(crate) enum Build {
    Full,
    Reuse,
    WorkerOnly,
}
pub(crate) struct LaunchOptions {
    pub build: Build,
    pub baseline_assets: Option<PathBuf>,
    pub port: u16,
    pub mirror_fixture: bool,
}
impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            build: if std::env::var("TICKETS_SKIP_BUILD").as_deref() == Ok("1") {
                Build::Reuse
            } else {
                Build::Full
            },
            baseline_assets: std::env::var_os("TICKETS_BEFORE_ASSETS").map(PathBuf::from),
            port: 8795,
            mirror_fixture: false,
        }
    }
}
struct Adapter {
    executable: PathBuf,
    health_timeout: Duration,
    port_timeout: Duration,
}
pub(crate) struct LocalWorker {
    child: Child,
    url: String,
    password: String,
    directory: PathBuf,
    #[cfg(not(target_arch = "wasm32"))]
    state: PathBuf,
    #[cfg(not(target_arch = "wasm32"))]
    config: PathBuf,
    log: PathBuf,
}
impl Drop for LocalWorker {
    fn drop(&mut self) {
        // The group was created exclusively for this child. Kill descendants too,
        // including on startup errors, then reap the direct child.
        #[cfg(unix)]
        let _ = Command::new("/bin/kill")
            .args(["-KILL", "--", &format!("-{}", self.child.id())])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl LocalWorker {
    pub(crate) fn start(root: &Path, options: LaunchOptions) -> Result<Self> {
        Self::start_with(
            root,
            options,
            Adapter {
                executable: "wrangler".into(),
                health_timeout: Duration::from_secs(45),
                port_timeout: Duration::from_secs(60),
            },
        )
    }
    pub(crate) fn url(&self) -> &str {
        &self.url
    }
    pub(crate) fn password(&self) -> &str {
        &self.password
    }
    pub(crate) fn directory(&self) -> &Path {
        &self.directory
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn state(&self) -> &Path {
        &self.state
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn config(&self) -> &Path {
        &self.config
    }
    pub(crate) fn log(&self) -> &Path {
        &self.log
    }

    fn start_with(root: &Path, options: LaunchOptions, adapter: Adapter) -> Result<Self> {
        if matches!(options.build, Build::Reuse) {
            // Explicitly reuse assets built before a sequence of browser-only checks.
            if !root.join("crates/web/dist/index.html").is_file()
                || !root.join("crates/worker/build/worker/shim.mjs").is_file()
            {
                return Err("TICKETS_SKIP_BUILD requires an existing web and Worker build".into());
            }
        } else if matches!(options.build, Build::WorkerOnly) {
            // Baseline web assets are already built; only the local Worker is needed.
            super::run(
                &root.join("crates/worker"),
                "worker-build",
                &["--release", "--locked"],
            )?;
        } else {
            super::build(root)?;
        }
        let mut random = [0u8; 24];
        getrandom::getrandom(&mut random).map_err(|_| "randomness unavailable")?;
        let password = tickets_core::auth::hex(&random);
        let dir = root
            .join("target")
            .join(format!("api-smoke-{}", &password[..12]));
        fs::create_dir_all(&dir)?;
        let mut digest = [0u8; 32];
        pbkdf2::pbkdf2_hmac::<Sha256>(
            password.as_bytes(),
            &random[..16],
            tickets_core::auth::PASSWORD_ITERATIONS,
            &mut digest,
        );
        fs::write(
            dir.join(".dev.vars"),
            format!(
                "BOOTSTRAP_USERNAME=fixture\nBOOTSTRAP_PASSWORD_HASH=\"pbkdf2-sha256${}${}${}\"\n",
                tickets_core::auth::PASSWORD_ITERATIONS,
                tickets_core::auth::hex(&random[..16]),
                tickets_core::auth::hex(&digest)
            ),
        )?;
        let config = dir.join("wrangler.toml");
        fs::write(
            &config,
            format!(
                "name='tickets-api-smoke'\nmain='{}'\ncompatibility_date='2026-10-27'\n[assets]\ndirectory='{}'\nbinding='ASSETS'\nnot_found_handling='none'\nrun_worker_first=true\n[[d1_databases]]\nbinding='DB'\ndatabase_name='api-smoke'\ndatabase_id='00000000-0000-0000-0000-000000000000'\nmigrations_dir='{}'\n",
                root.join("crates/worker/build/worker/shim.mjs").display(),
                options
                    .baseline_assets
                    .clone()
                    .unwrap_or_else(|| root.join("crates/web/dist"))
                    .display(),
                root.join("migrations").display()
            ),
        )?;
        use std::io::Write;
        fs::OpenOptions::new().append(true).open(&config)?.write_all(b"\n[[durable_objects.bindings]]\nname='BOARD_ROOMS'\nclass_name='BoardRoom'\n[[migrations]]\ntag='v1-board-room'\nnew_sqlite_classes=['BoardRoom']\n")?;
        if options.mirror_fixture {
            fs::OpenOptions::new()
                .append(true)
                .open(&config)?
                .write_all(b"\n[vars]\nGITHUB_PR_OWNERS='0x63616c'\n")?;
            fs::OpenOptions::new()
                .append(true)
                .open(dir.join(".dev.vars"))?
                .write_all(b"GITHUB_WEBHOOK_SECRET=mirror-fixture\n")?;
        }
        let state = dir.join("state");
        let migration_log = dir.join("migrations.log");
        let migration_output = fs::File::create(&migration_log)?;
        let status = Command::new(&adapter.executable)
            .args(["d1", "migrations", "apply", "DB", "--local", "--config"])
            .arg(&config)
            .arg("--persist-to")
            .arg(&state)
            .stdin(Stdio::null())
            .stdout(migration_output.try_clone()?)
            .stderr(migration_output)
            .status()?;
        if !status.success() {
            return Err(format!("local migrations failed; see {}", migration_log.display()).into());
        }
        let log_path = dir.join("worker.log");
        let log = fs::File::create(&log_path)?;
        let mut server_command = Command::new(&adapter.executable);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            server_command.process_group(0);
        }
        let port = options.port;
        let mut port_available = false;
        let port_started = Instant::now();
        while port_started.elapsed() < adapter.port_timeout {
            if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
                port_available = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        if !port_available {
            return Err(format!(
                "Local check port {port} is busy; retry after the other check finishes"
            )
            .into());
        }

        let child = server_command
            .args([
                "dev",
                "--local",
                "--ip",
                "127.0.0.1",
                "--port",
                &port.to_string(),
                "--test-scheduled",
                "--inspector-port",
                "0",
                "--config",
            ])
            .arg(&config)
            .arg("--persist-to")
            .arg(&state)
            .stdout(log.try_clone()?)
            .stderr(log)
            .stdin(Stdio::null())
            .spawn()?;

        // Own immediately so every subsequent failure tears down the child.
        let mut worker = Self {
            child,
            url: format!("http://127.0.0.1:{port}"),
            password,
            directory: dir,
            #[cfg(not(target_arch = "wasm32"))]
            state,
            #[cfg(not(target_arch = "wasm32"))]
            config,
            log: log_path,
        };
        let started = Instant::now();
        loop {
            if let Some(status) = worker.child.try_wait()? {
                return Err(
                    format!("Worker exited: {status}; see {}", worker.log().display()).into(),
                );
            }
            // Startup performs health reads only, after successful local migrations.
            let response = Command::new("curl")
                .args([
                    "--silent",
                    "--show-error",
                    "--max-time",
                    "10",
                    "-w",
                    "\n%{http_code}",
                ])
                .arg(format!("{}/api/health", worker.url()))
                .output()?;
            if response.status.success() {
                let output = String::from_utf8_lossy(&response.stdout);
                if let Some((body, "200")) = output.rsplit_once('\n')
                    && serde_json::from_str::<Value>(body).is_ok()
                    && worker.child.try_wait()?.is_none()
                {
                    return Ok(worker);
                }
            }
            if started.elapsed() >= adapter.health_timeout {
                return Err(
                    format!("Worker health timed out; see {}", worker.log().display()).into(),
                );
            }
            thread::sleep(Duration::from_millis(300));
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        os::unix::fs::PermissionsExt,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
    };

    static FIXTURES: Mutex<()> = Mutex::new(());

    struct Fixture {
        _serial: std::sync::MutexGuard<'static, ()>,
        root: PathBuf,
    }
    impl Fixture {
        fn new(script: &str) -> Self {
            let serial = FIXTURES.lock().unwrap();
            let mut random = [0; 8];
            getrandom::getrandom(&mut random).unwrap();
            let root = std::env::temp_dir().join(format!(
                "tickets-worker-test-{:x}",
                u64::from_ne_bytes(random)
            ));
            fs::create_dir(&root).unwrap();
            for file in [
                "crates/web/dist/index.html",
                "crates/worker/build/worker/shim.mjs",
            ] {
                let file = root.join(file);
                fs::create_dir_all(file.parent().unwrap()).unwrap();
                fs::write(file, "").unwrap();
            }
            let executable = root.join("wrangler");
            fs::write(
                &executable,
                format!(
                    "#!/bin/sh\ncd \"$(dirname \"$0\")\"\nprintf '%s\\n' \"$*\" >> calls\n{script}"
                ),
            )
            .unwrap();
            fs::set_permissions(executable, fs::Permissions::from_mode(0o700)).unwrap();
            Self {
                root,
                _serial: serial,
            }
        }
        fn start(&self, port: u16, mirror: bool) -> Result<LocalWorker> {
            LocalWorker::start_with(
                &self.root,
                LaunchOptions {
                    build: Build::Reuse,
                    baseline_assets: Some(self.root.join("baseline")),
                    port,
                    mirror_fixture: mirror,
                },
                Adapter {
                    executable: self.root.join("wrangler"),
                    health_timeout: Duration::from_millis(700),
                    port_timeout: Duration::from_secs(2),
                },
            )
        }
        fn run_dir(&self) -> PathBuf {
            fs::read_dir(self.root.join("target"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    fn free_port() -> u16 {
        TcpListener::bind(("127.0.0.1", 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }
    fn assert_reaped(pid: &str) {
        assert!(
            !Command::new("/bin/kill")
                .args(["-0", pid.trim()])
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success()
        );
    }

    #[test]
    fn failed_migration_never_launches_and_retains_diagnostics() {
        let fixture = Fixture::new("echo migration-failed >&2\nexit 23\n");
        let error = fixture.start(free_port(), false).err().unwrap().to_string();
        let log = fixture.run_dir().join("migrations.log");
        assert!(error.contains(&log.display().to_string()));
        assert!(
            fs::read_to_string(log)
                .unwrap()
                .contains("migration-failed")
        );
        let calls = fs::read_to_string(fixture.root.join("calls")).unwrap();
        assert_eq!(calls.lines().count(), 1);
        assert!(calls.starts_with("d1 migrations apply DB --local"));
        assert!(!fixture.run_dir().join("worker.log").exists());
    }

    #[test]
    fn early_exit_identifies_retained_log_and_reaps_child() {
        let fixture = Fixture::new(
            "if [ \"$1\" = d1 ]; then exit 0; fi\necho $$ > worker.pid\necho worker-exited >&2\nexit 23\n",
        );
        let error = fixture.start(free_port(), false).err().unwrap().to_string();
        let log = fixture.run_dir().join("worker.log");
        assert!(error.contains("Worker exited:"));
        assert!(error.contains("23"));
        assert!(error.contains(&log.display().to_string()));
        assert!(fs::read_to_string(log).unwrap().contains("worker-exited"));
        assert_reaped(&fs::read_to_string(fixture.root.join("worker.pid")).unwrap());
    }

    #[test]
    fn timeout_reaps_owned_child_and_leaves_unrelated_process_alive() {
        let fixture = Fixture::new(
            "if [ \"$1\" = d1 ]; then exit 0; fi\necho $$ > worker.pid\nexec sleep 60\n",
        );
        let mut unrelated = Command::new("sleep").arg("60").spawn().unwrap();
        let error = fixture.start(free_port(), false).err().unwrap().to_string();
        let unrelated_alive = unrelated.try_wait().unwrap().is_none();
        unrelated.kill().unwrap();
        unrelated.wait().unwrap();
        assert!(unrelated_alive);
        assert!(error.contains("health timed out"), "{error}");
        assert!(error.contains(&fixture.run_dir().join("worker.log").display().to_string()));
        assert_reaped(&fs::read_to_string(fixture.root.join("worker.pid")).unwrap());
    }

    #[test]
    fn ready_session_only_reads_health_after_migration_and_owns_its_group() {
        let fixture = Fixture::new(
            "if [ \"$1\" = d1 ]; then touch migrated; exit 0; fi\ntest -f migrated || exit 24\necho $$ > worker.pid\nsleep 60 &\necho $! > descendant.pid\ntouch launched\nwait\n",
        );
        let port = free_port();
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let responder = {
            let root = fixture.root.clone();
            let stop = stop.clone();
            let requests = requests.clone();
            thread::spawn(move || {
                while !root.join("launched").exists() {
                    if stop.load(Ordering::SeqCst) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(5));
                }
                assert!(root.join("migrated").exists());
                let listener = TcpListener::bind(("127.0.0.1", port)).unwrap();
                listener.set_nonblocking(true).unwrap();
                while !stop.load(Ordering::SeqCst) {
                    if let Ok((mut stream, _)) = listener.accept() {
                        stream
                            .set_read_timeout(Some(Duration::from_secs(1)))
                            .unwrap();
                        let mut buffer = [0; 4096];
                        let count = stream.read(&mut buffer).unwrap();
                        requests
                            .lock()
                            .unwrap()
                            .push(String::from_utf8_lossy(&buffer[..count]).to_string());
                        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").unwrap();
                    } else {
                        thread::sleep(Duration::from_millis(5));
                    }
                }
            })
        };
        let result = fixture.start(port, true);
        stop.store(true, Ordering::SeqCst);
        responder.join().unwrap();
        let worker = result.unwrap();
        assert_eq!(worker.url(), format!("http://127.0.0.1:{port}"));
        assert!(worker.state().ends_with("state"));
        assert_eq!(worker.password().len(), 48);
        let vars = fs::read_to_string(worker.directory().join(".dev.vars")).unwrap();
        assert!(!vars.contains(worker.password()));
        assert!(vars.contains("GITHUB_WEBHOOK_SECRET=mirror-fixture"));
        let config = fs::read_to_string(worker.config()).unwrap();
        assert!(config.contains(&fixture.root.join("baseline").display().to_string()));
        assert!(config.contains("GITHUB_PR_OWNERS='0x63616c'"));
        let requests = requests.lock().unwrap();
        assert!(!requests.is_empty());
        for request in requests.iter() {
            assert!(request.starts_with("GET /api/health HTTP/1.1"));
            assert!(!request.to_lowercase().contains("authorization:"));
        }
        let pid = fs::read_to_string(fixture.root.join("worker.pid")).unwrap();
        let descendant = fs::read_to_string(fixture.root.join("descendant.pid")).unwrap();
        let log = worker.log().to_path_buf();
        let calls = fs::read_to_string(fixture.root.join("calls")).unwrap();
        let calls: Vec<_> = calls.lines().collect();
        assert_eq!(calls.len(), 2);
        assert!(calls[0].starts_with("d1 migrations apply DB --local"));
        assert!(calls[1].starts_with("dev --local --ip 127.0.0.1 --port "));
        for call in &calls {
            assert!(call.contains(&worker.config().display().to_string()));
            assert!(call.contains(&worker.state().display().to_string()));
        }
        let mut unrelated = Command::new("sleep").arg("60").spawn().unwrap();
        drop(worker);
        let unrelated_alive = unrelated.try_wait().unwrap().is_none();
        unrelated.kill().unwrap();
        unrelated.wait().unwrap();
        assert!(unrelated_alive);
        assert_reaped(&pid);
        // An orphan may briefly remain a zombie until the system reaps it.
        if let Ok(stat) = fs::read_to_string(format!("/proc/{}/stat", descendant.trim())) {
            assert_eq!(stat.split_whitespace().nth(2), Some("Z"));
        }
        assert!(log.exists());
        assert!(fixture.run_dir().join(".dev.vars").exists());
    }

    #[test]
    fn reuse_validates_both_artifacts_before_running_wrangler() {
        let fixture = Fixture::new("exit 99\n");
        for file in [
            "crates/web/dist/index.html",
            "crates/worker/build/worker/shim.mjs",
        ] {
            let path = fixture.root.join(file);
            fs::remove_file(&path).unwrap();
            let error = fixture.start(free_port(), false).err().unwrap().to_string();
            assert!(error.contains("requires an existing web and Worker build"));
            assert!(!fixture.root.join("calls").exists());
            fs::write(path, "").unwrap();
        }
    }
}
