use std::path::Path;

// Билд .proto файлов и копирование папки models/ в папку билда
fn main() -> Result<(), Box<dyn std::error::Error>>
{
    let protoc = protoc_bin_vendored::protoc_bin_path()?;

    unsafe
    {
        std::env::set_var("PROTOC", protoc);
    }
    tonic_build::compile_protos("proto/restore.proto")?;

    copy_models();
    println!("cargo:rerun-if-changed=proto/restore.proto");
    println!("cargo:rerun-if-changed=models");

    Ok(())
}

fn copy_models()
{
    let out = match std::env::var("OUT_DIR")
    {
        Ok(v) => v,
        Err(_) => return
    };

    let target_dir = match Path::new(&out).ancestors().nth(3)
    {
        Some(d) => d,
        None => return
    };

    let src = Path::new("models");
    if !src.is_dir()
    {
        return;
    }

    let _ = copy_dir(src, &target_dir.join("models"));
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()>
{
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)?
    {
        let entry = entry?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir()
        {
            copy_dir(&path, &target)?;
        }
        else
        {
            std::fs::copy(&path, &target)?;
        }
    }
    Ok(())
}
