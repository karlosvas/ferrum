use {colored::Colorize, std::process::Command};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|a| a.as_str()) {
        Some("compile-web") => compile_web(),
        _ => eprintln!("Uso cargo xtask <comando>"),
    }
}

fn compile_web() {
    let status = Command::new("wasm-pack")
        .args(["build", "--target", "web", "--out-dir", "./www/public/pkg"])
        .status()
        .expect("wasm not found");

    assert!(status.success(), "wasm-pack failed");

    #[cfg(target_os = "windows")]
    {
        if !std::path::Path::new("www/public/res").exists() {
            Command::new("cmd")
                .args(["/C", "mklink /D www\\public\\res ..\\..\\res"])
                .status()
                .expect("mklink failed, do you need admin permision");
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if !std::path::Path::new("www/public/res").exists() {
            std::os::unix::fs::symlink("../../res", "www/public/res")
                .expect("mklink failed, do you need admin permision");
        }
    }

    println!("{}", "✓ WSM compiled succesfuly and linked res".green());
}
