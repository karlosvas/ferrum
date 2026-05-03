use {
    colored::Colorize,
    std::process::{Command, ExitStatus},
};

fn main() {
    dotenvy::dotenv().ok();

    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|a| a.as_str()) {
        Some("web") => compile_web(),
        Some("rpi") => compile_rpi(),
        Some("vercel-deploy") => compile_and_publish_web(),
        Some("run") => run_all(),
        _ => eprintln!("Uso cargo xtask <comando>"),
    }
}

fn compile_web() {
    let status = Command::new("wasm-pack")
        .args([
            "build",
            "crates/engine",
            "--target",
            "web",
            "--out-dir",
            "./www/public/pkg",
        ])
        .status()
        .expect(&"wasm-pack not found".red().to_string());

    assert!(status.success(), "wasm-pack failed");
    println!("{}", "✓ WSM compiled succesfuly".green());
}

fn compile_and_publish_web() {
    compile_web();

    let status: ExitStatus = Command::new("cmd")
        .args(["/C", "vercel deploy --yes"])
        .status()
        .expect(&"vercel command not found".red().to_string());
    assert!(status.success(), "vercel deploy failed");

    println!("{}", "✓ Deployed to Vercel".green());
}

fn compile_rpi() {
    let wsl_user: String = std::env::var("WSL_USER").unwrap_or("karlos".into());
    let cargo_path: String = format!("/home/{}/.cargo/bin/cargo", wsl_user);

    if cfg!(target_os = "windows") {
        let status = Command::new("wsl")
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
            .status()
            .expect("wsl not found");
        assert!(status.success(), "wsl xtask rpi failed");
        return;
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
        .status()
        .expect(&"cross not found".red().to_string());
    assert!(status.success(), "cross build failed");
    println!("{}", "✓ Compiled for aarch64".green());

    let user: String = std::env::var("PI_USER").expect("PI_USER not set");
    let host: String = std::env::var("PI_HOST").expect("PI_HOST not set");

    let dest: String = format!("{}@{}:~/rpi", user, host);

    let status = Command::new("scp")
        .args(["target/aarch64-unknown-linux-gnu/release/rpi", &dest])
        .status()
        .expect(&"scp failed".red().to_string());
    assert!(status.success(), "scp deploy failed");
    println!("{}", "✓ Deployed to Raspberry Pi".green());

    let remote_cmd = "chmod +x ~/rpi && ~/rpi &".to_string();
    let status = Command::new("ssh")
        .args([format!("{}@{}", user, host), remote_cmd])
        .status()
        .expect(&"ssh failed".red().to_string());
    assert!(status.success(), "ssh exec failed");
    println!("{}", "✓ Running on Pi".green());
}

fn run_all() {
    compile_rpi();

    let status = Command::new("cargo")
        .args(["run", "-p", "engine"])
        .status()
        .expect(&"cargo run failed".red().to_string());
    assert!(status.success(), "engine failed");
}
