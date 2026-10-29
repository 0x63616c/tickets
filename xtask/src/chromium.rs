//! Isolated browser launches. Retries end before any check assertions run.
use std::{
    fs,
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tungstenite::{WebSocket, stream::MaybeTlsStream};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

pub(crate) struct Process {
    child: Child,
    profile: PathBuf,
}
impl Drop for Process {
    fn drop(&mut self) {
        // Chromium has renderer/utility children; stop our private process group too.
        #[cfg(unix)]
        let _ = Command::new("/bin/kill")
            .args(["-KILL", "--", &format!("-{}", self.child.id())])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.profile);
    }
}
fn endpoint(contents: &str) -> Option<String> {
    let mut lines = contents.lines();
    let port = lines.next()?.parse::<u16>().ok()?;
    let path = lines.next()?;
    (port != 0 && path.starts_with("/devtools/browser/"))
        .then(|| format!("ws://127.0.0.1:{port}{path}"))
}
fn tail(path: &Path) -> String {
    let text = fs::read_to_string(path).unwrap_or_default();
    text.lines()
        .rev()
        .take(30)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n")
}
pub(crate) fn launch(root: &Path, name: &str, chrome: &Path) -> Result<(Process, Socket)> {
    launch_with(root, name, chrome, Duration::from_secs(30), 3)
}
fn launch_with(
    root: &Path,
    name: &str,
    chrome: &Path,
    timeout: Duration,
    attempts: usize,
) -> Result<(Process, Socket)> {
    let target = root.join("target");
    fs::create_dir_all(&target)?;
    let mut nonce = [0u8; 8];
    getrandom::getrandom(&mut nonce).map_err(|_| "randomness unavailable")?;
    let run = u64::from_ne_bytes(nonce);
    let mut errors = Vec::new();
    for attempt in 1..=attempts {
        let stem = format!("{name}-chromium-{run:x}-attempt-{attempt}");
        let profile = target.join(&stem);
        fs::create_dir(&profile)?;
        let log = target.join(format!("{stem}.log"));
        let result = (|| -> Result<(Process, Socket)> {
            let mut command = Command::new(chrome);
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                command.process_group(0);
            }
            let child = command
                .args([
                    "--headless=new",
                    "--no-sandbox",
                    "--no-proxy-server",
                    "--disable-gpu",
                    "--hide-scrollbars",
                    "--remote-debugging-port=0",
                    "--remote-allow-origins=*",
                    "--no-first-run",
                    "--no-default-browser-check",
                    "--disable-dev-shm-usage",
                ])
                .arg(format!("--user-data-dir={}", profile.display()))
                .arg("about:blank")
                .stdout(Stdio::null())
                .stderr(fs::File::create(&log)?)
                .spawn()?;
            // Own the child immediately: every error path kills and reaps it.
            let mut process = Process {
                child,
                profile: profile.clone(),
            };
            let started = Instant::now();
            let mut last_error = String::from("waiting for complete DevToolsActivePort");
            loop {
                if let Some(status) = process.child.try_wait()? {
                    return Err(format!("Chromium exited: {status}").into());
                }
                if let Ok(contents) = fs::read_to_string(profile.join("DevToolsActivePort"))
                    && let Some(url) = endpoint(&contents)
                {
                    let address = url
                        .strip_prefix("ws://")
                        .unwrap()
                        .split('/')
                        .next()
                        .unwrap();
                    let connected = (|| -> Result<Socket> {
                        let stream =
                            TcpStream::connect_timeout(&address.parse()?, Duration::from_secs(1))?;
                        stream.set_read_timeout(Some(Duration::from_secs(1)))?;
                        stream.set_write_timeout(Some(Duration::from_secs(1)))?;
                        let (socket, _) =
                            tungstenite::client(url.as_str(), MaybeTlsStream::Plain(stream))?;
                        if let MaybeTlsStream::Plain(stream) = socket.get_ref() {
                            stream.set_read_timeout(Some(Duration::from_secs(20)))?;
                            stream.set_write_timeout(Some(Duration::from_secs(20)))?;
                        }
                        Ok(socket)
                    })();
                    match connected {
                        Ok(socket) => return Ok((process, socket)),
                        Err(e) => last_error = e.to_string(),
                    }
                }
                if started.elapsed() >= timeout {
                    return Err(format!(
                        "DevTools startup timed out after {timeout:?}: {last_error}"
                    )
                    .into());
                }
                thread::sleep(Duration::from_millis(100));
            }
        })();
        match result {
            Ok(browser) => return Ok(browser),
            Err(error) => {
                let _ = fs::remove_dir_all(&profile);
                let detail = format!(
                    "attempt {attempt}: {error}; log: {}\n{}",
                    log.display(),
                    tail(&log)
                );
                eprintln!("{detail}");
                errors.push(detail);
            }
        }
    }
    Err(format!(
        "Chromium did not start after {attempts} attempts:\n{}",
        errors.join("\n")
    )
    .into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn failed_launches_retry_with_fresh_profiles_and_keep_logs() {
        use std::os::unix::fs::PermissionsExt;
        let mut nonce = [0; 8];
        getrandom::getrandom(&mut nonce).unwrap();
        let root = std::env::temp_dir().join(format!(
            "tickets-launch-test-{:x}",
            u64::from_ne_bytes(nonce)
        ));
        fs::create_dir(&root).unwrap();
        let fake = root.join("chrome");
        fs::write(
            &fake,
            "#!/bin/sh\necho \"$@\" >&2\necho SingletonLock-test >&2\nexit 23\n",
        )
        .unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();
        let error = launch_with(&root, "test", &fake, Duration::from_millis(300), 3)
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("after 3 attempts"));
        assert!(error.contains("SingletonLock-test"));
        assert!(error.contains("23"));
        let logs: Vec<_> = fs::read_dir(root.join("target"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(logs.len(), 3); // All three profiles were removed, logs retained.
        for (index, log) in logs.iter().enumerate() {
            assert_eq!(log.extension().unwrap(), "log");
            let content = fs::read_to_string(log).unwrap();
            assert!(content.contains("--remote-debugging-port=0"));
            for other in &logs[index + 1..] {
                assert_ne!(content, fs::read_to_string(other).unwrap());
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn timeout_kills_and_reaps_child() {
        use std::os::unix::fs::PermissionsExt;
        let mut nonce = [0; 8];
        getrandom::getrandom(&mut nonce).unwrap();
        let root = std::env::temp_dir().join(format!(
            "tickets-timeout-test-{:x}",
            u64::from_ne_bytes(nonce)
        ));
        fs::create_dir(&root).unwrap();
        let fake = root.join("chrome");
        fs::write(&fake, "#!/bin/sh\necho $$ >&2\nexec sleep 60\n").unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();
        let started = Instant::now();
        let error = launch_with(&root, "timeout", &fake, Duration::from_millis(150), 1)
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(5));
        let log = fs::read_dir(root.join("target"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let pid = fs::read_to_string(log).unwrap();
        assert!(
            !Command::new("/bin/kill")
                .args(["-0", pid.trim()])
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success()
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn waits_for_complete_port_file() {
        assert!(endpoint("").is_none());
        assert!(endpoint("1234\n").is_none());
        assert!(endpoint("0\n/devtools/browser/id\n").is_none());
        assert_eq!(
            endpoint("1234\n/devtools/browser/id\n").unwrap(),
            "ws://127.0.0.1:1234/devtools/browser/id"
        );
    }
}
