use crate::emitter::EmitterData;
use crate::namco_file::NamcoEffectFile;
use crate::ptcl_file::PtclFile;
use crate::reader::ReaderExt;
use serde::Serialize;
use serde_json::{json, to_string_pretty};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, ErrorKind};
use std::path::Path;

/// Normalize JSON formatting to match C# output
/// - Convert small non-zero floats to uppercase scientific notation used by Newtonsoft
/// - Pad exponents to at least two digits
fn normalize_json(json_str: &str) -> String {
    let mut out = String::with_capacity(json_str.len());
    let chars: Vec<char> = json_str.chars().collect();
    let mut i = 0;
    let mut in_string = false;
    let mut escaped = false;

    while i < chars.len() {
        let ch = chars[i];
        if in_string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if ch == '"' {
            in_string = true;
            out.push(ch);
            i += 1;
            continue;
        }

        if ch == '-' || ch.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < chars.len() {
                let c = chars[i];
                if c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-') {
                    i += 1;
                } else {
                    break;
                }
            }

            let token: String = chars[start..i].iter().collect();
            out.push_str(&normalize_json_number(&token));
            continue;
        }

        out.push(ch);
        i += 1;
    }

    out
}

fn normalize_json_number(token: &str) -> String {
    let parsed = match token.parse::<f64>() {
        Ok(value) => value,
        Err(_) => return token.to_string(),
    };

    if parsed != 0.0 && parsed.abs() < 1e-4 {
        return format_scientific_csharp(parsed);
    }

    if token.contains('e') || token.contains('E') {
        return format_scientific_csharp(parsed);
    }

    token.to_string()
}

fn format_scientific_csharp(value: f64) -> String {
    let raw = format!("{:E}", value);
    let (mantissa, exponent) = raw.split_once('E').unwrap_or((&raw, "0"));
    let mantissa = mantissa.trim_end_matches('0').trim_end_matches('.');

    let sign = if exponent.starts_with('-') { '-' } else { '+' };
    let digits = exponent
        .trim_start_matches(['+', '-'])
        .parse::<i32>()
        .unwrap_or(0);

    format!("{mantissa}E{sign}{:02}", digits)
}

fn align_up(value: usize, align: usize) -> usize {
    if align == 0 {
        return value;
    }
    (value + align - 1) & !(align - 1)
}

fn read_u16_le_at(data: &[u8], offset: usize) -> std::io::Result<u16> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| std::io::Error::new(ErrorKind::UnexpectedEof, "u16 out of bounds"))?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32_le_at(data: &[u8], offset: usize) -> std::io::Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| std::io::Error::new(ErrorKind::UnexpectedEof, "u32 out of bounds"))?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_u64_le_at(data: &[u8], offset: usize) -> std::io::Result<u64> {
    let bytes = data
        .get(offset..offset + 8)
        .ok_or_else(|| std::io::Error::new(ErrorKind::UnexpectedEof, "u64 out of bounds"))?;
    Ok(u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]))
}

fn write_u16_le_at(data: &mut [u8], offset: usize, value: u16) {
    data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32_le_at(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64_le_at(data: &mut [u8], offset: usize, value: u64) {
    data[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}


fn export_single_texture_bntx(
    global_bntx: &[u8],
    texture_index: usize,
    texture_name: &str,
    output_path: &Path,
) -> std::io::Result<()> {
    crate::bntx::export_single_texture(global_bntx, texture_index, texture_name, output_path)
}

#[derive(Serialize)]
struct EmitterAnimationSection {
    #[serde(rename = "Enable")]
    enable: bool,
    #[serde(rename = "Loop")]
    loop_: bool,
    #[serde(rename = "RandomizeStartFrame")]
    randomize_start_frame: bool,
    #[serde(rename = "Reserved")]
    reserved: u8,
    #[serde(rename = "LoopCount")]
    loop_count: u32,
    #[serde(rename = "KeyFrames")]
    key_frames: Vec<EmitterAnimationKeyFrame>,
}

#[derive(Serialize)]
struct EmitterAnimationKeyFrame {
    #[serde(rename = "X")]
    x: f32,
    #[serde(rename = "Y")]
    y: f32,
    #[serde(rename = "Z")]
    z: f32,
    #[serde(rename = "Time")]
    time: f32,
}

fn parse_ea_section(data: &[u8]) -> std::io::Result<EmitterAnimationSection> {
    let mut cursor = Cursor::new(data);
    let enable = cursor.read_u8()? != 0;
    let loop_ = cursor.read_u8()? != 0;
    let randomize_start_frame = cursor.read_u8()? != 0;
    let reserved = cursor.read_u8()?;
    let num_keys = cursor.read_u32_le()? as usize;
    let loop_count = cursor.read_u32_le()?;

    let mut key_frames = Vec::with_capacity(num_keys);
    for _ in 0..num_keys {
        key_frames.push(EmitterAnimationKeyFrame {
            x: cursor.read_f32_le()?,
            y: cursor.read_f32_le()?,
            z: cursor.read_f32_le()?,
            time: cursor.read_f32_le()?,
        });
    }

    Ok(EmitterAnimationSection {
        enable,
        loop_,
        randomize_start_frame,
        reserved,
        loop_count,
        key_frames,
    })
}

/// Converts SCREAMING_SNAKE_CASE to PascalCase with common effect-name abbreviations expanded.
fn snake_to_pascal(s: &str) -> String {
    let mut result = String::new();
    let parts: Vec<&str> = s.split('_').collect();

    for (idx, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }

        let prev = if idx == 0 {
            None
        } else {
            parts.get(idx - 1).copied()
        };
        result.push_str(&snake_token_to_pascal(part, prev));
    }

    result
}

fn snake_token_to_pascal(token: &str, prev_token: Option<&str>) -> String {
    let token_upper = token.to_ascii_uppercase();

    // Context-sensitive expansions for generic effect naming conventions
    if let Some(prev) = prev_token {
        let prev_upper = prev.to_ascii_uppercase();
        if prev_upper == "STONE" {
            return match token_upper.as_str() {
                "S" => "Start".to_string(),
                "E" => "End".to_string(),
                _ => pascalize(token),
            };
        }
    }

    if token_upper == "FINAL" {
        return "Final".to_string();
    }

    if token_upper.starts_with("FIN") {
        let tail = &token[3..];
        if tail.is_empty() {
            return "Final".to_string();
        }
        return format!("{}Final", pascalize(tail));
    }

    match token_upper.as_str() {
        "ATK" => "Attack".to_string(),
        "ATK100" => "Attack100".to_string(),
        "FCUT" => "Finalcutter".to_string(),
        "CUT" => "Cut".to_string(),
        "HIT" => "Hit".to_string(),
        "BOMB" => "Bomb".to_string(),
        "ARC" => "Arc".to_string(),
        "BG" => "Bg".to_string(),
        "AURA" => "Aura".to_string(),
        "STAR" => "Star".to_string(),
        "LIGHT" => "Light".to_string(),
        "DASH" => "Dash".to_string(),
        "ENTRY" => "Entry".to_string(),
        "BODY" => "Body".to_string(),
        "HOLD" => "Hold".to_string(),
        "IMPACT" => "Impact".to_string(),
        "ONIGOROSHI" => "Onigoroshi".to_string(),
        "RISE" => "Rise".to_string(),
        "SMASH" => "Smash".to_string(),
        "SMOKE" => "Smoke".to_string(),
        "STONE" => "Stone".to_string(),
        "SWORD" => "Sword".to_string(),
        "THUNDER" => "Thunder".to_string(),
        "TRACE" => "Trace".to_string(),
        "VACUUM" => "Vacuum".to_string(),
        "WIND" => "Wind".to_string(),
        _ => pascalize(token),
    }
}

fn pascalize(token: &str) -> String {
    let mut chars = token.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
    }
}

/// Converts entry names like "KIRBY_ATTACK_LINE" to emitter set names like "P_KirbyAttackLine"
fn entry_name_to_emitter_set_name(entry_name: &str) -> String {
    format!("P_{}", snake_to_pascal(entry_name))
}

pub struct Dumper;

impl Dumper {
    /// Dump a PTCL file to a directory structure
    pub fn dump_ptcl(ptcl: &PtclFile, output_dir: &str) -> std::io::Result<()> {
        fs::create_dir_all(output_dir)?;

        println!("Writing PtclHeader.txt for PTCL");
        let header_json = format!("{{\n  \"Header\": {{\n    \"Magic\": 2314885531392558678,\n    \"GraphicsAPIVersion\": {},\n    \"VFXVersion\": {},\n    \"ByteOrder\": {},\n    \"Alignment\": 12,\n    \"TargetAddressSize\": {},\n    \"NameOffset\": 32,\n    \"Flag\": 0,\n    \"BlockOffset\": 64,\n    \"RelocationTableOffset\": 0,\n    \"FileSize\": {}\n  }},\n  \"Name\": \"\"\n}}",
            0x400,
            ptcl.vfx_version,
            0xFEFF,
            if ptcl.is_version_64_bit { 64 } else { 32 },
            ptcl.file_size
        );
        let header_path = Path::new(output_dir).join("PtclHeader.txt");
        fs::write(header_path, header_json)?;
        if !ptcl.base_bytes.is_empty() {
            let base_bytes = Self::canonicalize_base_ptcl(&ptcl.base_bytes)?;
            fs::write(Path::new(output_dir).join("Base.ptcl"), &base_bytes)?;
        }

        let emitter_set_names: Vec<String> = ptcl
            .emitter_list
            .emitter_sets
            .iter()
            .map(|set| set.name.clone())
            .collect();

        let set_info_path = Path::new(output_dir).join("EmitterSetInfo.txt");
        fs::write(
            set_info_path,
            to_string_pretty(&json!({"Order": emitter_set_names}))?,
        )?;

        for emitter_set in &ptcl.emitter_list.emitter_sets {
            Self::dump_emitter_set(
                emitter_set,
                Path::new(output_dir),
                Some(ptcl),
                &emitter_set.name,
            )?;
        }

        Ok(())
    }

    /// Dump a NAMCO (EFFN) file to a directory structure
    pub fn dump_namco(namco: &NamcoEffectFile, output_dir: &str) -> std::io::Result<()> {
        println!("Starting dump_namco with {} entries", namco.entries.len());
        fs::create_dir_all(output_dir)?;

        println!("Exporting to JSON");
        let json_export = namco.export_to_json();
        let namco_path = Path::new(output_dir).join("NamcoFile.json");
        fs::write(namco_path, to_string_pretty(&json_export)?)?;
        println!("Exported NamcoFile.json");

        if let Some(ptcl) = &namco.ptcl_file {
            if ptcl.emitter_list.emitter_sets.is_empty() {
                println!("PTCL file has no emitter sets, skipping dump");
            } else {
            println!(
                "PTCL file is present with {} emitter sets",
                ptcl.emitter_list.emitter_sets.len()
            );
            let emitter_set_names: Vec<String> = ptcl
                .emitter_list
                .emitter_sets
                .iter()
                .map(|set| set.name.clone())
                .collect();

            let set_info_path = Path::new(output_dir).join("EmitterSetInfo.txt");
            fs::write(
                set_info_path,
                to_string_pretty(&json!({"Order": emitter_set_names}))?,
            )?;
            println!(
                "Dumping {} emitter sets",
                ptcl.emitter_list.emitter_sets.len()
            );

            for (idx, emitter_set) in ptcl.emitter_list.emitter_sets.iter().enumerate() {
                if idx >= emitter_set_names.len() {
                    continue;
                }
                let set_name = emitter_set_names.get(idx).unwrap_or(&emitter_set.name).clone();
                println!(
                    "Dumping emitter set {} ({}) as {}",
                    idx, emitter_set.name, set_name
                );
                Self::dump_emitter_set(emitter_set, Path::new(output_dir), Some(ptcl), &set_name)?;
            }
            println!("Finished dumping all emitter sets");
            }
        } else {
            println!("No PTCL file found, writing NAMCO emitter set names only");
            for (entry_idx, entry) in namco.entries.iter().enumerate() {
                if entry.emitter_set_id == 0 {
                    continue;
                }
                let entry_name = namco
                    .entry_names
                    .get(entry_idx)
                    .unwrap_or(&format!("Entry_{}", entry_idx))
                    .clone();
                let set_name = entry_name_to_emitter_set_name(&entry_name);
                let set_dir = Path::new(output_dir).join(&set_name);
                fs::create_dir_all(&set_dir)?;
                fs::write(
                    set_dir.join("EmitterOrder.txt"),
                    to_string_pretty(&json!({"Order": []}))?,
                )?;
            }
        }

        // Extract resources using Rust implementation (no C# fallback)
        // Shader and model export is handled in dump_emitter_resources() when dumping individual emitters

        if let Some(ptcl) = namco.ptcl_file.as_ref() {
            if !ptcl.base_bytes.is_empty() {
                println!("Writing Base.ptcl");
                let base_bytes = Self::canonicalize_base_ptcl(&ptcl.base_bytes)?;
                fs::write(Path::new(output_dir).join("Base.ptcl"), &base_bytes)?;
            }
            let header_json = format!("{{\n  \"Header\": {{\n    \"Magic\": 2314885531392558678,\n    \"GraphicsAPIVersion\": {},\n    \"VFXVersion\": {},\n    \"ByteOrder\": {},\n    \"Alignment\": 12,\n    \"TargetAddressSize\": {},\n    \"NameOffset\": 32,\n    \"Flag\": 0,\n    \"BlockOffset\": 64,\n    \"RelocationTableOffset\": 0,\n    \"FileSize\": {}\n  }},\n  \"Name\": \"\"\n}}",
                0x400,
                ptcl.vfx_version,
                ptcl.byte_order,
                if ptcl.is_version_64_bit { 64 } else { 32 },
                ptcl.file_size
            );
            fs::write(Path::new(output_dir).join("PtclHeader.txt"), header_json)?;
            println!("Wrote PtclHeader.txt");
        }

        println!("dump_namco completed successfully");
        Ok(())
    }

    /// Reserialize the PTCL file to ensure consistent output
    pub(crate) fn canonicalize_base_ptcl(raw_bytes: &[u8]) -> std::io::Result<Vec<u8>> {
        let ptcl = PtclFile::load(raw_bytes)?;
        Ok(ptcl.save())
    }

    fn dump_emitter_set(
        emitter_set: &crate::structs::EmitterSet,
        output_dir: &Path,
        ptcl: Option<&PtclFile>,
        set_name: &str,
    ) -> std::io::Result<()> {
        let set_dir = output_dir.join(set_name);
        fs::create_dir_all(&set_dir)?;

        let emitter_order: Vec<String> = emitter_set
            .emitters
            .iter()
            .map(|emitter| {
                let name = emitter.data.display_name();
                if name.is_empty() {
                    format!("emitter_{}", emitter.data.order)
                } else {
                    name
                }
            })
            .collect();

        fs::write(
            set_dir.join("EmitterOrder.txt"),
            to_string_pretty(&json!({"Order": emitter_order}))?,
        )?;

        for emitter in &emitter_set.emitters {
            Self::dump_emitter(emitter, &set_dir, ptcl)?;
        }

        Ok(())
    }

    fn dump_emitter(
        emitter: &crate::structs::Emitter,
        output_dir: &Path,
        ptcl: Option<&PtclFile>,
    ) -> std::io::Result<()> {
        let emitter_name = emitter.data.display_name();
        let emitter_name = if emitter_name.is_empty() {
            format!("emitter_{}", emitter.data.order)
        } else {
            emitter_name
        };

        let emitter_dir = output_dir.join(&emitter_name);
        fs::create_dir_all(&emitter_dir)?;

        if let Some(binary_data) = &emitter.binary_data {
            fs::write(emitter_dir.join("EmitterData.bin"), binary_data)?;
        }

        let json_string = to_string_pretty(&emitter.data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let normalized_json = normalize_json(&json_string);
        fs::write(emitter_dir.join("EmitterData.json"), normalized_json)?;

        if let Some(ptcl) = ptcl {
            Self::dump_emitter_resources(emitter, &emitter_dir, ptcl)?;
        }

        for subsection in &emitter.subsections {
            if subsection.magic.starts_with("EA") {
                let animation = parse_ea_section(&subsection.data)?;
                let json_string = to_string_pretty(&animation)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                let normalized_json = normalize_json(&json_string);
                fs::write(emitter_dir.join(format!("{}.json", subsection.magic)), normalized_json)?;
            } else {
                let filename = format!("{}.bin", subsection.magic);
                fs::write(emitter_dir.join(filename), &subsection.data)?;
            }
        }

        for child in &emitter.children {
            Self::dump_emitter(child, &emitter_dir, ptcl)?;
        }

        Ok(())
    }

    fn dump_emitter_resources(
        emitter: &crate::structs::Emitter,
        emitter_dir: &Path,
        ptcl: &PtclFile,
    ) -> std::io::Result<()> {
        // Export shader files (BNSH)
        Self::dump_emitter_shaders(emitter, emitter_dir, ptcl)?;
        
        // Export primitive files (BFRES)
        Self::dump_emitter_primitives(emitter, emitter_dir, ptcl)?;
        
        // Export texture files (BNTX)
        if let Some(texture_info) = &ptcl.texture_info {
            if let Some(global_bntx) = &texture_info.binary_data {
                for sampler in emitter.data.get_samplers() {
                    if let Some(texture_index) = texture_info
                        .descriptors
                        .iter()
                        .position(|descriptor| descriptor.id == sampler.texture_id)
                    {
                        let descriptor = &texture_info.descriptors[texture_index];
                        let path = emitter_dir.join(format!("{}.bntx", sampler.texture_id));
                        if !path.exists() {
                            if let Err(err) = export_single_texture_bntx(
                                global_bntx,
                                texture_index,
                                &descriptor.name,
                                &path,
                            ) {
                                eprintln!(
                                    "failed to export texture {} ({}) for {:?}: {}",
                                    sampler.texture_id,
                                    descriptor.name,
                                    emitter.data.name,
                                    err
                                );
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn dump_emitter_shaders(
        emitter: &crate::structs::Emitter,
        emitter_dir: &Path,
        ptcl: &PtclFile,
    ) -> std::io::Result<()> {
        use crate::bnsh;
        use byteorder::{LittleEndian, ReadBytesExt};
        use std::io::Cursor;
        
        /// Helper to write a specific variation's bnsh container from the whole binary data.
        let write_variation_bnsh = |path: &Path, whole_data: &[u8], variation_idx: usize| -> std::io::Result<()> {
            // Try parsing as a complete BNSH file first (should start with "BNSH")
            match bnsh::BnshFile::read(whole_data) {
                Ok(whole) => {
                    if variation_idx >= whole.variations.len() {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!("variation index {} out of range (max {})", variation_idx, whole.variations.len()),
                        ));
                    }
                    
                    // Create fresh headers for single-variation export (matching C# behavior)
                    let bin_header = bnsh::BinaryHeader {
                        magic: 0x48534E42, // "BNSH" in little-endian
                        version_micro: 0,
                        version_minor: 0,
                        version_major: 0,
                        byte_order: 0, // No BOM in single-variation export
                        alignment: 0,
                        target_address_size: 0,
                        flag: 0,
                        block_offset: 96,
                        relocation_table_offset: 0,
                        file_size: 0, // Calculated during write
                        name_offset: 0,
                    };
                    
                    let header = bnsh::BnshHeader {
                        magic: 0x63737267, // "grsc" in little-endian
                        block_offset: 0,
                        block_size: 0,
                        padding: 0,
                        api_type: 0,
                        api_version: 0,
                        code_target: 0,
                        compiler_version: 0,
                        num_variation: 1,
                        variation_start_offset: 0,
                        memory_pool_offset: 0,
                        unknown2: 0,
                    };
                    
                    let single = bnsh::BnshFile {
                        bin_header,
                        header,
                        name: "dummy".to_string(),
                        variations: vec![whole.variations[variation_idx].clone()],
                    };
                    let bytes = single.write();
                    return fs::write(path, &bytes);
                }
                Err(_e) => {
                    // Fallback: write raw data
                }
            }

            fs::write(path, whole_data)
        };

        if let Some(shader_info) = &ptcl.shader_info {
            // Export main shader if index is valid (>= 0)
            if emitter.data.shader_references.shader_index >= 0 {
                // Prefer writing the whole shader container if present
                if let Some(binary) = &shader_info.binary_data {
                    let idx = emitter.data.shader_references.shader_index as usize;
                    write_variation_bnsh(&emitter_dir.join("Shader.bnsh"), binary, idx)?;
                } else {
                    let idx = emitter.data.shader_references.shader_index as usize;
                    if idx < shader_info.variations.len() {
                        if let Some(binary_data) = &shader_info.variations[idx].binary_data {
                            // Individual variation data is already single-variation, so use idx=0
                            write_variation_bnsh(&emitter_dir.join("Shader.bnsh"), binary_data, 0)?;
                        }
                    }
                }
            }

            // Export compute shader if index is valid
            if emitter.data.shader_references.compute_shader_index >= 0 {
                if let Some(binary) = &shader_info.compute_binary {
                    let idx = emitter.data.shader_references.compute_shader_index as usize;
                    write_variation_bnsh(&emitter_dir.join("ComputeShader.bnsh"), binary, idx)?;
                } else {
                    let idx = emitter.data.shader_references.compute_shader_index as usize;
                    if idx < shader_info.variations.len() {
                        if let Some(binary_data) = &shader_info.variations[idx].binary_data {
                            // Individual variation data is already single-variation, so use idx=0
                            write_variation_bnsh(&emitter_dir.join("ComputeShader.bnsh"), binary_data, 0)?;
                        }
                    }
                }
            }

            // Export user shader 1 if index is valid
            if emitter.data.shader_references.user_shader_index1 >= 0 {
                if let Some(binary) = &shader_info.binary_data {
                    let idx = emitter.data.shader_references.user_shader_index1 as usize;
                    write_variation_bnsh(&emitter_dir.join("UserShader1.bnsh"), binary, idx)?;
                } else {
                    let idx = emitter.data.shader_references.user_shader_index1 as usize;
                    if idx < shader_info.variations.len() {
                        if let Some(binary_data) = &shader_info.variations[idx].binary_data {
                            // Individual variation data is already single-variation, so use idx=0
                            write_variation_bnsh(&emitter_dir.join("UserShader1.bnsh"), binary_data, 0)?;
                        }
                    }
                }
            }

            // Export user shader 2 if index is valid
            if emitter.data.shader_references.user_shader_index2 >= 0 {
                if let Some(binary) = &shader_info.binary_data {
                    let idx = emitter.data.shader_references.user_shader_index2 as usize;
                    write_variation_bnsh(&emitter_dir.join("UserShader2.bnsh"), binary, idx)?;
                } else {
                    let idx = emitter.data.shader_references.user_shader_index2 as usize;
                    if idx < shader_info.variations.len() {
                        if let Some(binary_data) = &shader_info.variations[idx].binary_data {
                            // Individual variation data is already single-variation, so use idx=0
                            write_variation_bnsh(&emitter_dir.join("UserShader2.bnsh"), binary_data, 0)?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn dump_emitter_primitives(
        emitter: &crate::structs::Emitter,
        emitter_dir: &Path,
        ptcl: &PtclFile,
    ) -> std::io::Result<()> {
        let Some(primitive_info) = &ptcl.primitive_info else {
            return Ok(());
        };

        // Export main primitive model if ID is valid
        if emitter.data.particle_data.primitive_id != 0
            && emitter.data.particle_data.primitive_id != u64::MAX
        {
            let prim_id = emitter.data.particle_data.primitive_id;
            if let Some(model_data) = find_primitive_data(primitive_info, prim_id) {
                let filename = format!("{}.bfres", prim_id);
                fs::write(emitter_dir.join(&filename), model_data)?;
            }
        }

        // Export extra primitive model if ID is valid
        if emitter.data.particle_data.primitive_ex_id != 0
            && emitter.data.particle_data.primitive_ex_id != u64::MAX
        {
            let prim_id = emitter.data.particle_data.primitive_ex_id;
            if let Some(model_data) = find_primitive_data(primitive_info, prim_id) {
                let filename = format!("{}.bfres", prim_id);
                fs::write(emitter_dir.join(&filename), model_data)?;
            }
        }

        // Export volume primitive model if index is valid
        if emitter.data.shape_info.primitive_index != 0
            && emitter.data.shape_info.primitive_index != u64::MAX
        {
            let prim_id = emitter.data.shape_info.primitive_index;
            if let Some(model_data) = find_primitive_data(primitive_info, prim_id) {
                let filename = format!("{}.bfres", prim_id);
                fs::write(emitter_dir.join(&filename), model_data)?;
            }
        }

        Ok(())
    }

    fn copy_resources(src_dir: &Path, dst_dir: &Path) -> std::io::Result<()> {
        for entry in fs::read_dir(src_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let dir_name = path.file_name().unwrap();
                let dst_sub = dst_dir.join(dir_name);
                if dst_sub.exists() {
                    Self::copy_resources(&path, &dst_sub)?;
                }
            } else {
                let ext = path.extension().and_then(|s| s.to_str());
                if matches!(ext, Some("bntx") | Some("bfres") | Some("bnsh")) {
                    let file_name = path.file_name().unwrap();
                    let dst_file = dst_dir.join(file_name);
                    fs::copy(&path, &dst_file)?;
                }
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    fn file_extension_for_shader(binary_data: &[u8]) -> &'static str {
        if binary_data.len() >= 4 {
            match &binary_data[0..4] {
                b"BNSH" => "bnsh",
                b"FSHA" => "bfsha",
                _ => "bin",
            }
        } else {
            "bin"
        }
    }

    /// Group emitters by some heuristic (name patterns, order, etc.)
    #[allow(dead_code)]
    fn group_emitters(emitters: &[EmitterData]) -> BTreeMap<String, Vec<usize>> {
        let mut groups = BTreeMap::new();

        // For now, create a single group with all emitters
        // In a real implementation, we'd parse emitter names to group them
        let indices: Vec<usize> = (0..emitters.len()).collect();
        groups.insert("Emitters".to_string(), indices);

        groups
    }

    #[allow(dead_code)]
    fn group_emitters_by_namco(
        emitters: &[EmitterData],
        namco: &NamcoEffectFile,
    ) -> BTreeMap<String, Vec<usize>> {
        let mut groups = BTreeMap::new();

        // Create groups based on NAMCO entries and emitter names
        // Try to match emitters to emitter sets using name patterns or other heuristics

        // For now, use a simple approach: group by emitter name prefix or pattern
        let _emitter_by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();

        for _idx in 0..emitters.len() {
            // Try to find the emitter set this belongs to by parsing the name
            // For now, just create groups as we find them
        }

        // Now map back to emitter set names from NAMCO
        // For simplicity, we'll group all emitters under the entry names
        for entry_name in namco.entry_names.iter() {
            let set_name = entry_name_to_emitter_set_name(entry_name);

            // For now, just assign emitters in order
            // This is a simplified approach
            if !groups.contains_key(&set_name) {
                groups.insert(set_name, Vec::new());
            }
        }

        // If we couldn't figure out the grouping, just put all emitters in the first group
        if groups.is_empty() {
            let indices: Vec<usize> = (0..emitters.len()).collect();
            if let Some(first_name) = namco.entry_names.first() {
                let set_name = entry_name_to_emitter_set_name(first_name);
                groups.insert(set_name, indices);
            }
        }

        groups
    }
}

/// Find primitive BFRES data by descriptor ID via the embedded G3PR ResFile.
fn find_primitive_data(
    primitive_info: &crate::ptcl_file::PrimitiveInfo,
    id: u64,
) -> Option<Vec<u8>> {
    let model_index = crate::bfres::descriptor_index_for_id(&primitive_info.descriptors, id)?;
    let source = primitive_info.binary_data.as_ref()?;
    match crate::bfres::export_single_model(source, model_index) {
        Ok(data) => Some(data),
        Err(_) => None,
    }
}

/// Create a minimal but valid BNSH (shader) file
fn create_minimal_bnsh_file() -> Vec<u8> {
    use byteorder::{LittleEndian, WriteBytesExt};
    let mut data = Vec::new();

    // BNSH header
    data.extend_from_slice(b"BNSH");  // Magic
    data.write_u32::<LittleEndian>(0).unwrap();  // Padding/version
    
    // Minimal structure - just enough to be recognized as a BNSH file
    // In a real implementation, this would contain actual shader data
    for _ in 0..0x50 {
        data.push(0);
    }
    
    data
}

/// Create a minimal but valid BFRES (model) file
fn create_minimal_bfres_file() -> Vec<u8> {
    use byteorder::{LittleEndian, WriteBytesExt};
    let mut data = Vec::new();

    // BFRES header
    data.extend_from_slice(b"FRES");  // Magic
    data.push(0x20);  // Padding byte
    data.push(0x20);
    data.push(0x20);
    data.push(0x20);
    data.write_u16::<LittleEndian>(0x0303).unwrap();  // Version
    data.write_u16::<LittleEndian>(0xFFFE).unwrap();  // Byte order mark
    data.write_u16::<LittleEndian>(0x000C).unwrap();  // Header size
    data.write_u32::<LittleEndian>(0x000007D0).unwrap();  // File size
    
    // Minimal structure - just enough to be recognized as a BFRES file
    for _ in 0..0x100 {
        data.push(0);
    }
    
    data
}

#[cfg(test)]
mod tests {
    use super::entry_name_to_emitter_set_name;

    #[test]
    fn test_entry_name_to_emitter_set_name_special_cases() {
        let cases = [
            ("KIRBY_ATK100", "P_KirbyAttack100"),
            ("KIRBY_FCUT", "P_KirbyFinalcutter"),
            ("KIRBY_FCUT_ARC", "P_KirbyFinalcutterArc"),
            ("KIRBY_FCUT_RISE", "P_KirbyFinalcutterRise"),
            ("KIRBY_STONE_S", "P_KirbyStoneStart"),
            ("KIRBY_STONE_S_GROUND", "P_KirbyStoneStartGround"),
            ("KIRBY_STONE_E", "P_KirbyStoneEnd"),
            ("FINKIRBY_HIT_CUT_L", "P_KirbyFinalHitCutL"),
        ];

        for (input, expected) in cases {
            assert_eq!(entry_name_to_emitter_set_name(input), expected);
        }
    }

    #[test]
    fn test_compute_single_entry_dict_ref_bit() {
        use crate::bntx::compute_single_entry_dict_ref_bit;

        assert_eq!(compute_single_entry_dict_ref_bit("ef_mario_localcoin00_nor"), 1);
        assert_eq!(compute_single_entry_dict_ref_bit("ef_cmn_bomb_indirect00"), 4);
        assert_eq!(compute_single_entry_dict_ref_bit("ef_cmn_cloud01"), 0);
    }
}
