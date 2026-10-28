mod api_smoke;
mod auth;
mod auth_verify;
mod autofill;
#[cfg(not(target_arch = "wasm32"))]
mod import_check;
#[cfg(not(target_arch = "wasm32"))]
mod import_github;
#[cfg(not(target_arch = "wasm32"))]
mod live;
#[cfg(not(target_arch = "wasm32"))]
mod login;
#[cfg(not(target_arch = "wasm32"))]
mod mirror_check;
#[cfg(not(target_arch = "wasm32"))]
mod screenshots;
mod skill_check;

use std::{env, error::Error, path::Path, process::Command};

#[cfg(not(target_arch = "wasm32"))]
mod contrast;
#[cfg(not(target_arch = "wasm32"))]
mod design;
#[cfg(not(target_arch = "wasm32"))]
mod first_run;
#[cfg(not(target_arch = "wasm32"))]
mod polish;
#[cfg(not(target_arch = "wasm32"))]
mod projects;

type Result = std::result::Result<(), Box<dyn Error>>;

fn run(directory: &Path, program: &str, args: &[&str]) -> Result {
    println!("Running: {program} {}", args.join(" "));
    let status = Command::new(program)
        .args(args)
        .current_dir(directory)
        // Trunk expects a boolean; some CI/agent environments set NO_COLOR=1.
        .env("NO_COLOR", "true")
        .env(
            "CARGO_TARGET_DIR",
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("target"),
        )
        .status()?;
    if !status.success() {
        return Err(format!("{program} exited with {status}").into());
    }
    Ok(())
}

fn build(root: &Path) -> Result {
    run(
        &root.join("crates/web"),
        "trunk",
        &["build", "--release", "--locked"],
    )?;
    run(
        &root.join("crates/worker"),
        "worker-build",
        &["--release", "--locked"],
    )
}

fn main() -> Result {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let args: Vec<String> = env::args().skip(1).collect();
    let task = args.first().cloned();
    let max_args = if task.as_deref() == Some("migrate") {
        2
    } else {
        1
    };
    if args.len() > max_args && task.as_deref() != Some("import-github") {
        return Err("expected exactly one task: dev, build, migrate, ci, api-smoke, skill-check, live-check, import-github, import-check, auth-hash, auth-bench, auth-verify, auth-screenshots".into());
    }
    match task.as_deref() {
        #[cfg(not(target_arch = "wasm32"))]
        Some("import-github") => import_github::cli(&args[1..]),
        #[cfg(not(target_arch = "wasm32"))]
        Some("import-check") => api_smoke::import_check(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("mirror-check") => api_smoke::mirror_check(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("live-check") => api_smoke::live(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("login-check") => api_smoke::login(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("login-before") => login::before(root),
        Some("api-smoke") => api_smoke::run(root, false),
        Some("skill-check") => api_smoke::run(root, true),
        #[cfg(not(target_arch = "wasm32"))]
        Some("auth-screenshots") => screenshots::run(),
        Some("auth-verify") => auth_verify::run(),
        Some("auth-hash") => auth::hash(),
        Some("auth-bench") => auth::bench(),
        #[cfg(not(target_arch = "wasm32"))]
        Some("design-check") => design::check(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("board-before") => design::board_before(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("board-check") => api_smoke::board(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("list-check") => api_smoke::list(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("detail-check") => api_smoke::detail(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("polish-check") => api_smoke::polish(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("contrast-check") => api_smoke::contrast(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("projects-check") => api_smoke::projects(root),
        Some("first-run-check") => api_smoke::first_run(root),
        Some("build") => build(root),
        Some("dev") => {
            build(root)?;
            run(root, "wrangler", &["dev", "--local"])
        }
        Some("migrate") => match args.get(1).map(String::as_str) {
            None => run(
                root,
                "wrangler",
                &["d1", "migrations", "apply", "DB", "--local"],
            ),
            // Production only; requires Cloudflare credentials in the environment.
            Some("--remote") => run(
                root,
                "wrangler",
                &[
                    "d1",
                    "migrations",
                    "apply",
                    "DB",
                    "--remote",
                    "--env",
                    "production",
                ],
            ),
            Some(other) => Err(format!("unknown migrate flag {other}; expected --remote").into()),
        },
        Some("autofill-check") => autofill::check(root),        Some("ci") => {
            autofill::check(root)?;
            run(root, "cargo", &["fmt", "--all", "--check"])?;
            run(root, "cargo", &["build", "--workspace", "--locked"])?;
            run(
                root,
                "cargo",
                &[
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--locked",
                    "--",
                    "-D",
                    "warnings",
                ],
            )?;
            run(
                root,
                "cargo",
                &[
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--locked",
                    "--target",
                    "wasm32-unknown-unknown",
                    "--",
                    "-D",
                    "warnings",
                ],
            )?;
            run(root, "cargo", &["test", "--workspace", "--locked"])
        }
        _ => {
            Err("usage: cargo xtask <dev|build|migrate [--remote]|ci|api-smoke|skill-check|live-check|import-github --repo <owner/repo> --project <KEY> [--url <base-url>] [--allow-remote] [--dry-run]|import-check|auth-hash|auth-bench|auth-verify|auth-screenshots|design-check|board-before|board-check|list-check|detail-check|first-run-check>".into())
        }
    }
}
