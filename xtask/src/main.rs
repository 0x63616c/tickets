use std::{env, error::Error, path::Path, process::Command};

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
    let mut args = env::args().skip(1);
    let task = args.next();
    if args.next().is_some() {
        return Err("expected exactly one task: dev, build, migrate, ci".into());
    }
    match task.as_deref() {
        Some("build") => build(root),
        Some("dev") => {
            build(root)?;
            run(root, "wrangler", &["dev", "--local"])
        }
        Some("migrate") => run(
            root,
            "wrangler",
            &["d1", "migrations", "apply", "DB", "--local"],
        ),
        Some("ci") => {
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
        _ => Err("usage: cargo xtask <dev|build|migrate|ci>".into()),
    }
}
