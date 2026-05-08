// On partage le module icon avec le crate principal pour ne pas dupliquer la
// logique de dessin du logo.
#[path = "src/icon.rs"]
mod icon;

use std::path::Path;

fn main() {
    let assets_dir = Path::new("assets");
    if !assets_dir.exists() {
        let _ = std::fs::create_dir_all(assets_dir);
    }
    let ico_path = assets_dir.join("icon.ico");
    let _ = generate_ico(&ico_path);

    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set("ProductName", "NyxWhisper");
        res.set(
            "FileDescription",
            "NyxWhisper - Dictée vocale française locale",
        );
        res.set("CompanyName", "NyxWhisper");
        res.set("LegalCopyright", "MIT");
        if ico_path.exists() {
            res.set_icon(ico_path.to_str().unwrap());
        }
        let _ = res.compile();
    }

    println!("cargo:rerun-if-changed=src/icon.rs");
    println!("cargo:rerun-if-changed=build.rs");
}

fn generate_ico(path: &Path) -> std::io::Result<()> {
    let mut dir = ico::IconDir::new(ico::ResourceType::Icon);
    for &size in icon::ICO_SIZES {
        let rgba = icon::n_grunge_rgba(size);
        let img = ico::IconImage::from_rgba_data(size, size, rgba);
        let entry = ico::IconDirEntry::encode(&img)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        dir.add_entry(entry);
    }
    let file = std::fs::File::create(path)?;
    dir.write(file)?;
    Ok(())
}
