use {
    anyhow::{Ok, Result},
    colored::Colorize,
    std::{
        path::PathBuf,
        process::{Command, ExitStatus},
    },
    which::which,
};

fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|a| a.as_str()) {
        Some("web") => compile_web()?,
        Some("deploy") => deploy_web()?,
        Some("rpi") => compile_rpi()?,
        Some("run") => run_app()?,
        Some("demo") => setup_demo()?,
        _ => anyhow::bail!("Use: cargo xtask <web,deploy,rpi,run,demo>"),
    }

    Ok(())
}

fn compile_web() -> Result<()> {
    anyhow::ensure!(which("wasm-pack").is_ok(), "wasm-pack is not installed");

    let out_dir: PathBuf = std::env::current_dir()?.join("www/public/pkg");
    let status: ExitStatus = Command::new("wasm-pack")
        .args([
            "build",
            "crates/engine",
            "--target",
            "web",
            "--out-dir",
            out_dir.to_str().unwrap(),
        ])
        .status()?;

    anyhow::ensure!(status.success(), "wasm-pack failed");
    tracing::info!("{}", "✓ WSM compiled succesfuly".green());

    Ok(())
}

fn deploy_web() -> Result<()> {
    anyhow::ensure!(which("vercel").is_ok(), "vercel CLI is not installed");

    compile_web()?;

    let status: ExitStatus = Command::new("vercel")
        .args(["deploy", "--prod", "--yes", "--archive=tgz"])
        .status()?;

    anyhow::ensure!(status.success(), "vercel deploy failed");
    tracing::info!("{}", "✓ Deployed to Vercel".green());

    Ok(())
}

fn setup_rpi() -> Result<()> {
    compile_rpi()?;
    connect_rpi()?;

    Ok(())
}

fn connect_rpi() -> Result<()> {
    anyhow::ensure!(which("scp").is_ok(), "scp is not installed");
    anyhow::ensure!(which("ssh").is_ok(), "ssh is not installed");

    let user: String = std::env::var("RPI_USER")
        .map_err(|_| anyhow::anyhow!("RPI_USER not set (define it in .env)"))?;
    let host: String = std::env::var("RPI_HOST")
        .map_err(|_| anyhow::anyhow!("RPI_HOST not set (define it in .env)"))?;

    let dest: String = format!("{}@{}:~/rpi", user, host);

    let status: ExitStatus = Command::new("scp")
        .args(["target/aarch64-unknown-linux-gnu/release/rpi", &dest])
        .status()?;

    anyhow::ensure!(status.success(), "scp deploy failed");
    tracing::info!("{}", "✓ Deployed to Raspberry Pi".green());

    let remote_cmd: String = "chmod +x ~/rpi && ~/rpi &".to_string();
    let status: ExitStatus = Command::new("ssh")
        .args([format!("{}@{}", user, host), remote_cmd])
        .status()?;

    anyhow::ensure!(status.success(), "ssh exec failed");
    tracing::info!("{}", "✓ Running on Pi".green());

    Ok(())
}

fn run_app() -> Result<()> {
    if let Err(e) = setup_demo() {
        tracing::warn!("RPI setup skipped (opcional): {e}");
    }

    let status: ExitStatus = Command::new("cargo")
        .args(["run", "-p", "engine"])
        .status()?;

    anyhow::ensure!(status.success(), "engine failed");

    Ok(())
}

fn setup_demo() -> Result<()> {
    // Initialize code in pi
    setup_rpi()?;

    let status: ExitStatus = Command::new("cargo").args(["run", "-p", "demo"]).status()?;

    anyhow::ensure!(status.success(), "demo failed");

    Ok(())
}

fn compile_rpi() -> Result<()> {
    if cfg!(target_os = "windows") {
        let wsl_user: String = std::env::var("WSL_USER")?;
        let cargo_path: String = format!("/home/{}/.cargo/bin/cargo", wsl_user);
        let status: ExitStatus = Command::new("wsl")
            .args([
                "-d",
                "Ubuntu",
                "-u",
                wsl_user.as_str(),
                "--",
                cargo_path.as_str(),
                "xtask",
                "rpi",
            ])
            .status()?;

        anyhow::ensure!(status.success(), "wsl xtask rpi failed");
        return Ok(());
    }

    anyhow::ensure!(
        which("cross").is_ok(),
        "cross is not installed: cargo install cross (requires Docker/Podman)"
    );

    let status: ExitStatus = Command::new("cross")
        .args([
            "build",
            "--manifest-path",
            "crates/rpi/Cargo.toml",
            "--release",
            "--target",
            "aarch64-unknown-linux-gnu",
        ])
        .status()?;

    anyhow::ensure!(status.success(), "cross build failed");
    tracing::info!("{}", "✓ Compiled for aarch64".green());

    Ok(())
}
