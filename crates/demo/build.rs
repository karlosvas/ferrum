use {
    anyhow::{Error, Result, bail},
    fs_extra::{copy_items, dir::CopyOptions},
    std::{
        env,
        path::{Path, PathBuf},
    },
};

fn main() -> Result<(), Error> {
    let res_path: PathBuf = PathBuf::from("res");
    println!("cargo:rerun-if-changed={}", res_path.display());

    if !res_path.exists() {
        bail!("res directory not found at: {}", res_path.display());
    }

    if std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default() != "wasm32" {
        let out_dir: String = env::var("OUT_DIR")?;

        let mut copy_options: CopyOptions = CopyOptions::new();
        copy_options.overwrite = true;

        let paths_to_copy: Vec<&Path> = vec![res_path.as_path()];
        copy_items(&paths_to_copy, &out_dir, &copy_options)?;

        if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
            #[cfg(target_os = "windows")]
            {
                use winres::WindowsResource;
                let mut res = WindowsResource::new();
                res.set_icon("assets/logo.ico");
                res.compile()?;
            }
        }
    }

    Ok(())
}
