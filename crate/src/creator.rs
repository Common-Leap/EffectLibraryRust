use crate::namco_file::NamcoEffectFile;
use crate::ptcl_file::PtclFile;
use std::fs;
use std::path::Path;

pub struct Creator;

impl Creator {
    /// Create a PTCL file from a directory structure
    pub fn create_ptcl_from_folder(folder: &str) -> std::io::Result<PtclFile> {
        let folder_path = Path::new(folder);
        let base_ptcl_path = folder_path.join("Base.ptcl");
        if !base_ptcl_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Base.ptcl not found",
            ));
        }

        let base_data = fs::read(&base_ptcl_path)?;
        PtclFile::load(&base_data)
    }

    /// Create a NAMCO effect file from a directory structure
    pub fn create_namco_from_folder(folder: &str) -> std::io::Result<NamcoEffectFile> {
        let folder_path = Path::new(folder);
        let base_ptcl_path = folder_path.join("Base.ptcl");
        if !base_ptcl_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Base.ptcl not found",
            ));
        }

        let base_data = fs::read(&base_ptcl_path)?;
        let ptcl = PtclFile::load(&base_data)?;

        let namco_json_path = folder_path.join("NamcoFile.json");
        if namco_json_path.exists() {
            let content = fs::read_to_string(&namco_json_path)?;
            NamcoEffectFile::from_json(&content, ptcl)
        } else {
            Ok(NamcoEffectFile::new(ptcl))
        }
    }
}
