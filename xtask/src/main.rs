use {
    colored::Colorize,
    std::process::{Command, ExitStatus},
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|a| a.as_str()) {
        Some("web") => compile_web(),
        Some("rpi") => compile_rpi(),
        Some("vercel-deploy") => compile_and_publish_web(),
        _ => eprintln!("Uso cargo xtask <comando>"),
    }
}

fn compile_web() {
    let status = Command::new("wasm-pack")
        .args(["build", "--target", "web", "--out-dir", "./www/public/pkg"])
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
    let status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--target",
            "aarch64-unknown-linux-gnu",
        ])
        .status()
        .expect(&"cargo build failed".red().to_string());
    assert!(status.success(), "cargo build failed");
    println!("{}", "✓ Compiled for aarch64".green());

    let host: String = std::env::var("RPI_HOST").expect("RPI_HOST not set");
    let path: String = std::env::var("RPI_PATH").expect("RPI_PATH not set");
    let dest: String = format!("{}:{}", host, path);
    let status: ExitStatus = Command::new("scp")
        .args([
            "target/aarch64-unknown-linux-gnu/release/ferrum",
            dest.as_str(),
        ])
        .status()
        .expect(&"scp failed".red().to_string());
    assert!(status.success(), "scp deploy failed");
    println!("{}", "✓ Deployed to Raspberry Pi".green());
}
