mod common;
mod dict;
mod load;
mod save;
mod types;

pub use common::BfresError;

use indexmap::IndexMap;
use std::io::{self, Cursor, ErrorKind, Write};

use byteorder::{LittleEndian, WriteBytesExt};

use types::ResFileData;

/// Parsed Switch BFRES container.
#[derive(Debug, Clone)]
pub struct ResFile {
    inner: ResFileData,
}

impl ResFile {
    pub fn load(data: Vec<u8>) -> io::Result<Self> {
        if data.len() < 4 || &data[0..4] != b"FRES" {
            return Err(io::Error::new(ErrorKind::InvalidData, "expected FRES magic"));
        }
        Ok(Self {
            inner: load::load_from_bytes(&data)?,
        })
    }

    pub fn save(&self) -> io::Result<Vec<u8>> {
        Ok(save::save_to_bytes(&self.inner))
    }

    pub fn canonicalize(data: &[u8]) -> io::Result<Vec<u8>> {
        let file = Self::load(data.to_vec())?;
        file.save()
    }

    pub fn export_single_model(source: &[u8], model_index: usize) -> io::Result<Vec<u8>> {
        export_single_model(source, model_index)
    }

    pub fn as_bytes(&self) -> io::Result<Vec<u8>> {
        self.save()
    }
}

/// Find the descriptor table index for a primitive ID.
pub fn descriptor_index_for_id(
    descriptors: &[crate::ptcl_file::PrimitiveDescriptor],
    id: u64,
) -> Option<usize> {
    if id == 0 || id == u64::MAX {
        return None;
    }
    descriptors.iter().position(|descriptor| descriptor.id == id)
}

/// Export one model from an embedded multi-model BFRES blob.
pub fn export_single_model(source: &[u8], model_index: usize) -> io::Result<Vec<u8>> {
    let (_source_meta, model_name, model) = load::load_for_export(source, model_index)?;
    let mut output = ResFileData {
        name: model_name.clone(),
        version_major: 5,
        version_minor: 0,
        version_minor2: 3,
        alignment: 0x0C,
        flag: 0,
        block_offset: 0,
        target_address_size: 0,
        external_flag: 0,
        reserve10: 0,
        data_alignment_override: 0,
        models: IndexMap::new(),
        external_files: IndexMap::new(),
        string_table_strings: Vec::new(),
        preserve_shader_param_headers: false,
    };
    output.models.insert(model.name.clone(), model);
    Ok(save::save_to_bytes(&output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;


    #[test]
    fn export_3856198108_material_sampler_count() {
        let eff = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../References/effect/stage/zelda_gerudo/ef_zelda_gerudo.eff");
        let cs = Path::new(
            "/tmp/ef_zelda_gerudo/P_GerudoIceMagicA/ice1_add1/3856198108.bfres",
        );
        if !eff.exists() || !cs.exists() {
            return;
        }
        let data = std::fs::read(&eff).expect("read eff");
        let namco = crate::NamcoEffectFile::load(&data).expect("load namco");
        let pi = namco.ptcl_file.as_ref().expect("ptcl").primitive_info.as_ref().expect("pi");
        let source = pi.binary_data.as_ref().expect("source");
        let id = 3856198108u64;
        let idx = descriptor_index_for_id(&pi.descriptors, id).expect("idx");
        let a = load::load_for_export(source, idx).expect("load a");
        let b = load::load_for_export(source, idx).expect("load b");
        let a_mat = a.2.materials.values().next().expect("mat a");
        let b_mat = b.2.materials.values().next().expect("mat b");
        assert_eq!(a_mat.samplers.len(), b_mat.samplers.len(), "load must be deterministic");
        for (name, mat) in &a.2.materials {
            eprintln!(
                "mat {name}: samplers={} texture_refs={} shader_params={}",
                mat.samplers.len(),
                mat.texture_refs.len(),
                mat.shader_params.len()
            );
        }
        let exported = export_single_model(source, idx).expect("export");
        let expected = std::fs::read(cs).expect("read cs");
        assert_eq!(exported, expected, "3856198108.bfres must match C#");
    }

    #[test]
    fn export_ef_mario_primitive_matches_csharp() {
        let eff = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../References/effect/fighter/mario/ef_mario.eff");
        if !eff.exists() {
            return;
        }

        let data = std::fs::read(&eff).expect("read eff");
        let namco = crate::NamcoEffectFile::load(&data).expect("load namco");
        let ptcl = namco.ptcl_file.as_ref().expect("ptcl");
        let primitive_info = ptcl.primitive_info.as_ref().expect("primitive_info");
        let source = primitive_info
            .binary_data
            .as_ref()
            .expect("primitive_info.binary_data");
        assert!(source.starts_with(b"FRES"));

        let id = 2005374961u64;
        let index = descriptor_index_for_id(&primitive_info.descriptors, id).expect("descriptor");
        let exported = export_single_model(source, index).expect("export");
        assert!(exported.starts_with(b"FRES"));
        let version = u32::from_le_bytes(exported[8..12].try_into().unwrap());
        assert_eq!(version, 0x00050003);

        let csharp = Path::new("/tmp/ef_mario_csharp/ef_mario/P_MarioFinalBg/line1/2005374961.bfres");
        if csharp.exists() {
            let expected = std::fs::read(csharp).expect("read csharp bfres");
            if exported == expected {
                return;
            }
            eprintln!(
                "bfres export differs from C#: rust {} bytes, csharp {} bytes",
                exported.len(),
                expected.len()
            );
        }
    }

    #[test]
    fn export_ef_fox_primitive_1034549434_matches_csharp() {
        let eff = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../References/effect/fighter/fox/ef_fox.eff");
        let csharp = Path::new(
            "/tmp/ef_fox_csharp/ef_fox/P_FoxFinalExplosion/light1_g/1034549434.bfres",
        );
        if !eff.exists() || !csharp.exists() {
            return;
        }

        let data = std::fs::read(&eff).expect("read eff");
        let namco = crate::NamcoEffectFile::load(&data).expect("load namco");
        let ptcl = namco.ptcl_file.as_ref().expect("ptcl");
        let primitive_info = ptcl.primitive_info.as_ref().expect("primitive_info");
        let source = primitive_info
            .binary_data
            .as_ref()
            .expect("primitive_info.binary_data");

        let id = 1034549434u64;
        let index = descriptor_index_for_id(&primitive_info.descriptors, id).expect("descriptor");
        let exported = export_single_model(source, index).expect("export");
        let expected = std::fs::read(csharp).expect("read csharp bfres");

        assert_eq!(exported, expected, "1034549434.bfres must match C# byte-for-byte");
        let buffer_offset = u32::from_le_bytes(exported[0x1e0..0x1e4].try_into().unwrap());
        assert_eq!(buffer_offset, 0xe8, "FVTX buffer_offset must include 8-byte index padding");
    }

    #[test]
    fn batch_export_bfres_all_reference_eff_files() {
        let specs = [
            (
                "../References/effect/fighter/mario/ef_mario.eff",
                "/tmp/ef_mario_csharp/ef_mario",
            ),
            (
                "../References/effect/fighter/fox/ef_fox.eff",
                "/tmp/ef_fox_csharp/ef_fox",
            ),
            (
                "../References/effect/fighter/kirby/ef_kirby.eff",
                "/home/leap/Workshop/EffectLibraryRust/crate/target/batch_verify_fresh/csharp/kirby/ef_kirby",
            ),
            (
                "../References/effect/fighter/pikachu/ef_pikachu.eff",
                "/home/leap/Workshop/EffectLibraryRust/crate/target/batch_verify_fresh/csharp/pikachu/ef_pikachu",
            ),
            (
                "../References/effect/pokemon/lunala/ef_lunala.eff",
                "/home/leap/Workshop/EffectLibraryRust/crate/target/batch_verify_fresh/csharp/lunala/ef_lunala",
            ),
        ];

        for (eff_rel, cs_root) in specs {
            let eff = Path::new(env!("CARGO_MANIFEST_DIR")).join(eff_rel);
            let cs_root = Path::new(cs_root);
            if !eff.exists() || !cs_root.is_dir() {
                continue;
            }

            let data = std::fs::read(&eff).expect("read eff");
            let namco = crate::NamcoEffectFile::load(&data).expect("load namco");
            let ptcl = namco.ptcl_file.as_ref().expect("ptcl");
            let primitive_info = ptcl.primitive_info.as_ref().expect("primitive_info");
            let source = primitive_info
                .binary_data
                .as_ref()
                .expect("primitive_info.binary_data");

            let mut identical = 0usize;
            let mut compared = 0usize;
            for descriptor in &primitive_info.descriptors {
                if descriptor.id == 0 || descriptor.id == u64::MAX {
                    continue;
                }
                let Some(index) =
                    descriptor_index_for_id(&primitive_info.descriptors, descriptor.id)
                else {
                    continue;
                };
                let exported = export_single_model(source, index).expect("export");
                let Some(expected) = find_csharp_bfres(cs_root, descriptor.id) else {
                    continue;
                };
                compared += 1;
                if exported == expected {
                    identical += 1;
                } else {
                    panic!(
                        "bfres mismatch in {} for id {} ({} bytes vs {} bytes)",
                        eff_rel,
                        descriptor.id,
                        exported.len(),
                        expected.len()
                    );
                }
            }
            assert!(compared > 0, "no csharp reference bfres found for {eff_rel}");
            assert_eq!(
                identical, compared,
                "all exported bfres must match C# for {eff_rel}"
            );
        }
    }

    #[test]
    fn batch_export_size_check_ef_mario() {
        let eff = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../References/effect/fighter/mario/ef_mario.eff");
        let cs_root = Path::new("/tmp/ef_mario_csharp/ef_mario");
        if !eff.exists() || !cs_root.is_dir() {
            return;
        }
        let data = std::fs::read(&eff).expect("read eff");
        let namco = crate::NamcoEffectFile::load(&data).expect("load namco");
        let ptcl = namco.ptcl_file.as_ref().expect("ptcl");
        let primitive_info = ptcl.primitive_info.as_ref().expect("primitive_info");
        let source = primitive_info
            .binary_data
            .as_ref()
            .expect("primitive_info.binary_data");

        let mut identical = 0usize;
        let mut compared = 0usize;
        for descriptor in &primitive_info.descriptors {
            if descriptor.id == 0 || descriptor.id == u64::MAX {
                continue;
            }
            let Some(index) = descriptor_index_for_id(&primitive_info.descriptors, descriptor.id)
            else {
                continue;
            };
            let exported = export_single_model(source, index).expect("export");
            let cs_path = find_csharp_bfres(cs_root, descriptor.id);
            if let Some(expected) = cs_path {
                compared += 1;
                if exported == expected {
                    identical += 1;
                } else {
                    eprintln!(
                        "bfres mismatch id={}: rust {} bytes, csharp {} bytes, diffs={}",
                        descriptor.id,
                        exported.len(),
                        expected.len(),
                        exported
                            .iter()
                            .zip(expected.iter())
                            .filter(|(a, b)| a != b)
                            .count()
                    );
                }
            }
        }
        eprintln!("bfres batch: {identical}/{compared} byte-identical to C#");
        assert!(compared > 0, "no csharp reference bfres found");
        assert_eq!(identical, compared, "all exported bfres must match C# byte-for-byte");
    }

    fn find_csharp_bfres(root: &Path, id: u64) -> Option<Vec<u8>> {
        let name = format!("{id}.bfres");
        for entry in walkdir_simple(root) {
            if entry.ends_with(&name) {
                return std::fs::read(entry).ok();
            }
        }
        None
    }

    fn walkdir_simple(dir: &Path) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(path) = stack.pop() {
            let Ok(read_dir) = std::fs::read_dir(&path) else {
                continue;
            };
            for entry in read_dir.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    out.push(p);
                }
            }
        }
        out
    }

    #[test]
    fn canonicalize_embedded_multi_roundtrip() {
        let path = Path::new("/tmp/embedded_multi.bfres");
        if !path.exists() {
            return;
        }
        let data = std::fs::read(path).expect("read embedded_multi");
        let file = ResFile::load(data.clone()).expect("load");
        assert_eq!(file.inner.models.len(), 27);

        let saved = file.save().expect("save");
        assert!(saved.starts_with(b"FRES"));
        assert!(saved.len() > data.len() / 2);

        let roundtrip = ResFile::load(saved.clone()).expect("reload");
        assert_eq!(roundtrip.inner.models.len(), 27);
        let _saved2 = roundtrip.save().expect("save again");
    }
}

/// Legacy helper kept for existing tests/API surface.
pub struct BfresFile {
    magic: String,
    version: u32,
    models: Vec<(String, Vec<u8>)>,
}

impl BfresFile {
    pub fn new(magic: &str, version: u32) -> Self {
        Self {
            magic: magic.to_string(),
            version,
            models: Vec::new(),
        }
    }

    pub fn add_model(&mut self, name: &str, data: Vec<u8>) -> &mut Self {
        self.models.push((name.to_string(), data));
        self
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());

        write!(cursor, "{}", self.magic).unwrap();
        cursor.write_u32::<LittleEndian>(self.version).unwrap();

        let num_models = self.models.len() as u32;
        cursor.write_u32::<LittleEndian>(num_models).unwrap();

        for (name, data) in &self.models {
            let name_len = name.len() as u16;
            cursor.write_u16::<LittleEndian>(name_len).unwrap();
            cursor.write_all(name.as_bytes()).unwrap();

            if name_len % 2 != 0 {
                cursor.write_u8(0).unwrap();
            }

            let data_size = data.len() as u32;
            cursor.write_u32::<LittleEndian>(data_size).unwrap();

            if name_len % 2 != 0 {
                cursor.write_u8(0).unwrap();
            }

            cursor.write_all(data).unwrap();

            let current_pos = cursor.position() as usize;
            if current_pos % 16 != 0 {
                let padding_needed = 16 - (current_pos % 16);
                for _ in 0..padding_needed {
                    cursor.write_u8(0).unwrap();
                }
            }
        }

        let file_size = cursor.position() as u32;
        cursor.set_position(4);
        cursor.write_u32::<LittleEndian>(file_size).unwrap();

        cursor.into_inner()
    }
}

