use {
    anyhow::{Ok, Result},
    fs_extra::{copy_items, dir::CopyOptions},
    std::{env, path::PathBuf},
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn main() -> Result<()> {
    let root = workspace_root();
    println!("cargo:rerun-if-changed={}", root.join("res").display());

    let target: String = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    if target != "wasm32" {
        let out_dir: String = env::var("OUT_DIR")?;

        let mut copy_options: CopyOptions = CopyOptions::new();
        copy_options.overwrite = true;
        let paths_to_copy = vec![root.join("crates/ferrum/res")];
        let paths_to_copy: Vec<&std::path::Path> =
            paths_to_copy.iter().map(|p| p.as_path()).collect();
        copy_items(&paths_to_copy, &out_dir, &copy_options)?;

        if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
            #[cfg(target_os = "windows")]
            {
                use winres::WindowsResource;
                let mut res: WindowsResource = winres::WindowsResource::new();
                res.set_icon(root.join("crates/demo/assets/logo.ico").to_str().unwrap());
                res.compile()?;
            }
        }
    }

    Ok(())
}
