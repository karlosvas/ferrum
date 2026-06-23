use {
    anyhow::{Ok, Result},
    fs_extra::{copy_items, dir::CopyOptions},
    std::{env, path::PathBuf},
};

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    // In workspace: manifest_dir is crates/ferrum/, root is ../../
    // Published standalone: manifest_dir is the package root itself
    let candidate = manifest_dir.join("../..");
    if let std::result::Result::Ok(p) = candidate.canonicalize() {
        p
    } else {
        manifest_dir
    }
}

fn main() -> Result<()> {
    let root = workspace_root();
    let res_path = root.join("crates/ferrum/res");
    println!("cargo:rerun-if-changed={}", res_path.display());

    let target: String = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    if target != "wasm32" && res_path.exists() {
        let out_dir: String = env::var("OUT_DIR")?;

        let mut copy_options: CopyOptions = CopyOptions::new();
        copy_options.overwrite = true;
        let paths_to_copy = vec![res_path.as_path()];
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
