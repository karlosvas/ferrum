use {
    anyhow::Result,
    colored::Colorize,
    std::process::{Command, ExitStatus},
};

fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|a| a.as_str()) {
        Some("web") => compile_web()?,
        Some("rpi") => compile_rpi()?,
        Some("run") => run_app()?,
        _ => anyhow::bail!("Uso cargo xtask <comando>"),
    }

    Ok(())
}

fn compile_web() -> Result<()> {
    let status = Command::new("wasm-pack")
        .args([
            "build",
            "crates/engine",
            "--target",
            "web",
            "--out-dir",
            "./www/public/pkg",
        ])
        .status()?;

    anyhow::ensure!(status.success(), "wasm-pack failed");
    tracing::info!("{}", "✓ WSM compiled succesfuly".green());

    Ok(())
}

fn setup_rpi() -> Result<()> {
    compile_rpi()?;
    connect_rpi()?;

    Ok(())
}

fn connect_rpi() -> Result<()> {
    let user: String = std::env::var("RPI_USER")?;
    let host: String = std::env::var("RPI_HOST")?;

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
    if let Err(e) = setup_rpi() {
        tracing::warn!("RPI setup skipped (opcional): {e}");
    }

    let status: ExitStatus = Command::new("cargo")
        .args(["run", "-p", "engine"])
        .status()?;

    anyhow::ensure!(status.success(), "engine failed");

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
