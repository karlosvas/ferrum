use {
    anyhow::{Ok, Result},
    fs_extra::{copy_items, dir::CopyOptions},
    std::env,
};

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=res/*");

    let out_dir: String = env::var("OUT_DIR")?;
    let mut copy_options: CopyOptions = CopyOptions::new();
    copy_options.overwrite = true;
    let mut paths_to_copy: Vec<&str> = Vec::new();
    paths_to_copy.push("res/");
    copy_items(&paths_to_copy, out_dir, &copy_options);

    Ok(())
}
