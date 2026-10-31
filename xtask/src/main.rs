#[cfg(not(target_arch = "wasm32"))]
mod api_docs;
mod api_smoke;
mod auth;
mod auth_verify;
mod autofill;
#[cfg(not(target_arch = "wasm32"))]
mod board_loading;
#[cfg(not(target_arch = "wasm32"))]
mod cards;
#[cfg(not(target_arch = "wasm32"))]
mod cleanup;
#[cfg(not(target_arch = "wasm32"))]
mod component;
#[cfg(not(target_arch = "wasm32"))]
mod d1_usage;
mod design_tokens;
#[cfg(not(target_arch = "wasm32"))]
mod fonts;
#[cfg(not(target_arch = "wasm32"))]
mod github_pr_check;
#[cfg(not(target_arch = "wasm32"))]
mod home;
#[cfg(not(target_arch = "wasm32"))]
mod home_loading;
#[cfg(not(target_arch = "wasm32"))]
mod import_check;
#[cfg(not(target_arch = "wasm32"))]
mod import_github;
#[cfg(not(target_arch = "wasm32"))]
mod infra;
#[cfg(not(target_arch = "wasm32"))]
mod live;
#[cfg(not(target_arch = "wasm32"))]
mod loading;
mod local_worker;
#[cfg(not(target_arch = "wasm32"))]
mod login;
#[cfg(not(target_arch = "wasm32"))]
mod mirror_check;
#[cfg(not(target_arch = "wasm32"))]
mod perf;
#[cfg(not(target_arch = "wasm32"))]
mod perf_browser;

#[cfg(not(target_arch = "wasm32"))]
mod mission;
#[cfg(not(target_arch = "wasm32"))]
mod navigation;
#[cfg(not(target_arch = "wasm32"))]
mod page_layout;
#[cfg(not(target_arch = "wasm32"))]
mod pickup;
#[cfg(not(target_arch = "wasm32"))]
mod screenshots;
#[cfg(not(target_arch = "wasm32"))]
mod settings;
mod skill_check;
#[cfg(not(target_arch = "wasm32"))]
mod tokens;

use std::{env, error::Error, path::Path, process::Command};

#[cfg(not(target_arch = "wasm32"))]
mod chromium;
#[cfg(not(target_arch = "wasm32"))]
mod contrast;
#[cfg(not(target_arch = "wasm32"))]
mod design;
#[cfg(not(target_arch = "wasm32"))]
mod first_run;
#[cfg(not(target_arch = "wasm32"))]
mod palette;
#[cfg(not(target_arch = "wasm32"))]
mod polish;
#[cfg(not(target_arch = "wasm32"))]
mod project_repos;
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
        .status()?;
    if !status.success() {
        return Err(format!("{program} exited with {status}").into());
    }
    Ok(())
}

fn web_dependencies(root: &Path) -> Result {
    run(root, "npm", &["ci", "--prefix", "crates/web"])
}

fn build(root: &Path) -> Result {
    web_dependencies(root)?;
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
    // Cargo shares executable artifacts across issue worktrees; resolve the
    // invoking workspace at runtime rather than retaining a compiled-in path.
    let directory = env::current_dir()?;
    let root = directory
        .ancestors()
        .find(|path| path.join("wrangler.toml").is_file() && path.join("crates/web").is_dir())
        .ok_or("run cargo xtask inside the Tickets workspace")?;
    let args: Vec<String> = env::args().skip(1).collect();
    let task = args.first().cloned();
    let max_args = if task.as_deref() == Some("migrate") {
        2
    } else {
        1
    };
    if args.len() > max_args
        && !matches!(
            task.as_deref(),
            Some("import-github" | "component-shot" | "check")
        )
    {
        return Err("expected exactly one task: dev, build, migrate, ci, api-smoke, skill-check, live-check, import-github, import-check, auth-hash, auth-bench, auth-verify, auth-screenshots".into());
    }
    match task.as_deref() {
        #[cfg(not(target_arch = "wasm32"))]
        Some("perf-check") => perf::check(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("perf-browser-check") => api_smoke::perf(root),
        Some("check") if args.get(1).map(String::as_str) == Some("design-tokens") => design_tokens::check(root, &args[2..]),
        #[cfg(not(target_arch = "wasm32"))]
        Some("loading-check") => loading::check(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("home-shot") => home::shots(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("page-layout-check") => page_layout::check(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("home-loading-check") => api_smoke::home_loading(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("tokens-shot") => tokens::shots(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("settings-shot") => settings::shots(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("font-check") => fonts::check(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("settings-check") => settings::check(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("navigation-check") => navigation::check(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("avatar-check") => settings::avatar_check(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("card-shots") => cards::shots(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("component-shot") => component::run(root,&args[1..]),
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
        #[cfg(not(target_arch = "wasm32"))]
        Some("pickup-check") => api_smoke::pickup(root),
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
        Some("board-loading-check") => api_smoke::board_loading(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("board-check") => api_smoke::board(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("list-check") => api_smoke::list(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("detail-check") => api_smoke::detail(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("polish-check") => api_smoke::polish(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("palette-check") => api_smoke::palette(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("contrast-check") => api_smoke::contrast(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("projects-check") => api_smoke::projects(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("project-repos-check") => project_repos::check(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("d1-usage-check") => d1_usage::check(),
        #[cfg(not(target_arch = "wasm32"))]
        Some("api-docs-check") => api_docs::check(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("infra-check") => infra::check(root),
        Some("mission-check") => api_smoke::mission(root),
        #[cfg(not(target_arch = "wasm32"))]
        Some("first-run-check") => first_run::check(root),
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
            web_dependencies(root)?;
            autofill::check(root)?;
            design_tokens::check(root, &[])?;
            run(root, "node", &["--test", "crates/web/lint/value-policy.test.mjs"])?;
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
            Err("usage: cargo xtask <check design-tokens [--staged] [--css] [--report]|dev|build|migrate [--remote]|ci|infra-check|d1-usage-check|api-smoke|skill-check|live-check|import-github --repo <owner/repo> --project <KEY> [--url <base-url>] [--allow-remote] [--dry-run]|import-check|auth-hash|auth-bench|auth-verify|auth-screenshots|design-check|board-before|board-check|list-check|detail-check|first-run-check>".into())
        }
    }
}
