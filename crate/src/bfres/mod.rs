mod common;
mod dict;
pub mod load;
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
    export_single_model_with_session(&mut None, source, model_index)
}

/// Export one model, reusing a session when exporting multiple models from the same blob.
pub fn export_single_model_with_session<'a>(
    session: &mut Option<load::ResExportSession<'a>>,
    source: &'a [u8],
    model_index: usize,
) -> io::Result<Vec<u8>> {
    let needs_new_session = match session {
        Some(existing) => existing.ctx.reader.data.as_ptr() != source.as_ptr(),
        None => true,
    };
    if needs_new_session {
        *session = Some(load::ResExportSession::open(source).map_err(io::Error::from)?);
    }

    let (_, model_name, model) = session
        .as_mut()
        .expect("session initialized")
        .export_model(model_index)
        .map_err(io::Error::from)?;

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

