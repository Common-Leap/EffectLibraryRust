#!/usr/bin/env python3
"""Generate emitter_write.rs from read() implementations in emitter.rs."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EMITTER = ROOT / "src" / "emitter.rs"
OUT = ROOT / "src" / "emitter" / "emitter_write.rs"

HEADER = """//! Auto-generated write implementations mirroring emitter read order.
//! Regenerate: python3 crate/scripts/generate_emitter_write.py

use super::*;
use byteorder::{LittleEndian, WriteBytesExt};
use std::io::{self, Write};

pub trait WriterExt: Write {
    fn write_u8(&mut self, v: u8) -> io::Result<()> {
        self.write_all(&[v])
    }
    fn write_i16_le(&mut self, v: i16) -> io::Result<()> {
        self.write_i16::<LittleEndian>(v).map(|_| ())
    }
    fn write_u16_le(&mut self, v: u16) -> io::Result<()> {
        self.write_u16::<LittleEndian>(v).map(|_| ())
    }
    fn write_i32_le(&mut self, v: i32) -> io::Result<()> {
        self.write_i32::<LittleEndian>(v).map(|_| ())
    }
    fn write_u32_le(&mut self, v: u32) -> io::Result<()> {
        self.write_u32::<LittleEndian>(v).map(|_| ())
    }
    fn write_i64_le(&mut self, v: i64) -> io::Result<()> {
        self.write_i64::<LittleEndian>(v).map(|_| ())
    }
    fn write_u64_le(&mut self, v: u64) -> io::Result<()> {
        self.write_u64::<LittleEndian>(v).map(|_| ())
    }
    fn write_f32_le(&mut self, v: f32) -> io::Result<()> {
        self.write_f32::<LittleEndian>(v).map(|_| ())
    }
    fn write_bytes(&mut self, data: &[u8]) -> io::Result<()> {
        self.write_all(data)
    }
    fn write_fixed_string(&mut self, s: &str, len: usize) -> io::Result<()> {
        let mut buf = vec![0u8; len];
        let bytes = s.as_bytes();
        let copy_len = bytes.len().min(len);
        buf[..copy_len].copy_from_slice(&bytes[..copy_len]);
        self.write_all(&buf)
    }
}

impl<W: Write> WriterExt for W {}

fn write_bool_u8<W: WriterExt>(writer: &mut W, v: bool) -> io::Result<()> {
    writer.write_u8(v as u8)
}

"""

SKIP_TYPES = {"EmitterData", "CombinedEmitterCombinerV40"}


def extract_impl_blocks(text: str) -> list[tuple[str, str, str]]:
    pattern = re.compile(
        r"impl (?P<type>\w+) \{[^}]*?pub fn read(?:<[^>]*>)?\([^)]*\)[^{]*\{",
        re.DOTALL,
    )
    blocks = []
    for m in pattern.finditer(text):
        type_name = m.group("type")
        start = m.end() - 1
        depth = 0
        i = start
        while i < len(text):
            if text[i] == "{":
                depth += 1
            elif text[i] == "}":
                depth -= 1
                if depth == 0:
                    body = text[start + 1 : i]
                    blocks.append((type_name, body, m.group(0)))
                    break
            i += 1
    return blocks


def parse_ok_fields(body: str) -> list[tuple[str, str]]:
    ok_match = re.search(r"Ok\(\s*\w+\s*\{", body)
    if not ok_match:
        return []
    start = ok_match.end()
    depth = 1
    i = start
    while i < len(body) and depth:
        if body[i] == "{":
            depth += 1
        elif body[i] == "}":
            depth -= 1
        i += 1
    struct_body = body[start : i - 1]
    fields = []
    for part in struct_body.split(","):
        part = part.strip()
        if not part or part.startswith("//"):
            continue
        if ":" not in part:
            continue
        name, expr = part.split(":", 1)
        name = name.strip()
        expr = expr.strip()
        fields.append((name, expr))
    return fields


def expr_to_write(name: str, expr: str, type_name: str, version_param: str | None) -> str | None:
    v = version_param or "version"
    if "reader.read_u32_le()" in expr and "!= 0" not in expr:
        if "if " in expr:
            cond = expr.split("if ", 1)[1].split("{", 1)[0].strip()
            return f"if {cond} {{ writer.write_u32_le(self.{name})?; }}"
        return f"writer.write_u32_le(self.{name})?;"
    if "reader.read_i32_le()" in expr:
        if "if " in expr:
            cond = expr.split("if ", 1)[1].split("{", 1)[0].strip()
            return f"if {cond} {{ writer.write_i32_le(self.{name})?; }}"
        return f"writer.write_i32_le(self.{name})?;"
    if "reader.read_i16_le()" in expr:
        if "if " in expr:
            cond = expr.split("if ", 1)[1].split("{", 1)[0].strip()
            inner = f"writer.write_i16_le(self.{name})?"
            if ".transpose()" in expr or "Option" in expr:
                return f"if {cond} {{ if let Some(v) = self.{name} {{ writer.write_i16_le(v)?; }} }}"
            return f"if {cond} {{ {inner}; }}"
        return f"writer.write_i16_le(self.{name})?;"
    if "reader.read_u64_le()" in expr:
        if "if " in expr:
            cond = expr.split("if ", 1)[1].split("{", 1)[0].strip()
            if ".transpose()" in expr:
                return f"if {cond} {{ if let Some(v) = self.{name} {{ writer.write_u64_le(v)?; }} }}"
            return f"if {cond} {{ writer.write_u64_le(self.{name})?; }}"
        return f"writer.write_u64_le(self.{name})?;"
    if "reader.read_f32_le()" in expr:
        if "for v in &mut arr" in expr or "for _ in 0.." in expr:
            return f"for v in &self.{name}.as_ref().unwrap_or(&[0.0; 16]) {{ writer.write_f32_le(*v)?; }}"
        return f"writer.write_f32_le(self.{name})?;"
    if "reader.read_u8()" in expr and "!= 0" in expr:
        return f"write_bool_u8(writer, self.{name})?;"
    if "reader.read_u8()" in expr and "ColorType::from_u8" in expr:
        return f"writer.write_u8(self.{name}.as_u8())?;"
    if "reader.read_u8()" in expr and "WrapMode::from_u8" in expr:
        return f"writer.write_u8(self.{name}.as_u8())?;"
    if "reader.read_u8()" in expr:
        if ".transpose()" in expr:
            cond = expr.split(".then", 1)[0].strip()
            return f"if {cond} {{ if let Some(v) = self.{name} {{ writer.write_u8(v)?; }} }}"
        return f"writer.write_u8(self.{name})?;"
    if "reader.read_bytes(" in expr:
        m = re.search(r"read_bytes\(([^)]+)\)", expr)
        if m:
            size = m.group(1)
            if size == "16":
                return f"writer.write_bytes(&self.{name})?;"
            return f"writer.write_bytes(&self.{name})?;"
    if "reader.read_string(" in expr:
        return None
    m = re.search(r"(\w+)::read\(reader(?:,\s*version)?\)", expr)
    if m:
        sub = m.group(1)
        if ".transpose()" in expr:
            cond = expr.split(".then", 1)[0].strip()
            return f"if {cond} {{ self.{name}.as_ref().unwrap().write(writer, {v})?; }}"
        return f"self.{name}.write(writer, {v})?;"
    if "for _ in 0..5" in expr:
        return (
            f"if let Some(values) = &self.{name} {{ "
            f"for v in values {{ writer.write_u32_le(*v)?; }} }}"
        )
    return None


def generate_type_write(type_name: str, body: str) -> str | None:
    if type_name in SKIP_TYPES:
        return None
    version_param = "version" if ", version: u16" in body or ", version)" in body else None
    sig_version = ", version: u16" if version_param else ""
    fields = parse_ok_fields(body)
    if not fields:
        return None
    lines = [f"impl {type_name} {{"]
    lines.append(f"    pub fn write<W: WriterExt>(&self, writer: &mut W{sig_version}) -> io::Result<()> {{")
    for name, expr in fields:
        w = expr_to_write(name, expr, type_name, version_param)
        if w is None:
            if "reader.read_string" in expr:
                continue
            print(f"WARN: {type_name}.{name}: {expr[:80]}")
            continue
        lines.append(f"        {w}")
    lines.append("        Ok(())")
    lines.append("    }")
    lines.append("}")
    return "\n".join(lines)


def emitter_data_write() -> str:
    return """
impl EmitterData {
    pub fn write(&self, version: u16) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        let has_lt_40 = version_check(Some((VersionCompare::Less, 40)), version);
        let has_eq_36 = version_check(Some((VersionCompare::Equals, 36)), version);
        let has_gt_40 = version_check(Some((VersionCompare::Greater, 40)), version);
        let has_ge_36 = version_check(Some((VersionCompare::GreaterOrEqual, 36)), version);
        let has_ge_22 = version_check(Some((VersionCompare::GreaterOrEqual, 22)), version);

        buf.write_u32_le(self.flag)?;
        buf.write_u32_le(self.random_seed)?;
        buf.write_u32_le(self.padding1)?;
        buf.write_u32_le(self.padding2)?;

        if has_lt_40 {
            buf.write_fixed_string(self.name.as_deref().unwrap_or(""), 64)?;
        } else {
            buf.write_fixed_string(self.namev40.as_deref().unwrap_or(""), 96)?;
        }

        self.emitter_static.write(&mut buf, version)?;
        self.emitter_info.write(&mut buf)?;
        self.child_inheritance.write(&mut buf, version)?;
        self.emission.write(&mut buf)?;
        self.shape_info.write(&mut buf, version)?;
        self.render_state.write(&mut buf)?;
        self.particle_data.write(&mut buf, version)?;

        if has_lt_40 && !has_ge_36 {
            if let Some(EmitterCombinerVariant::Legacy(c)) = &self.combiner {
                c.write(&mut buf)?;
            }
        } else if has_eq_36 {
            if let Some(EmitterCombinerVariant::V36(c)) = &self.combiner {
                c.write(&mut buf)?;
            }
        } else if has_gt_40 {
            if let Some(EmitterCombinerVariant::V40(c)) = &self.combiner {
                c.write_combiner_body(&mut buf, version)?;
            }
        }

        self.shader_references.write(&mut buf, version)?;
        self.action.write(&mut buf, version)?;

        if has_gt_40 {
            buf.write_fixed_string(self.depth_mode.as_deref().unwrap_or(""), 16)?;
            buf.write_fixed_string(self.pass_info.as_deref().unwrap_or(""), 52)?;
        }

        self.particle_velocity.write(&mut buf)?;

        if has_ge_36 {
            if let Some(values) = &self.unknown_v36 {
                for v in values {
                    buf.write_f32_le(*v)?;
                }
            } else {
                for _ in 0..4 {
                    buf.write_f32_le(0.0)?;
                }
            }
        }

        self.particle_color.write(&mut buf)?;
        self.particle_scale.write(&mut buf)?;
        if let Some(fluc) = &self.particle_fluctuation {
            fluc.write(&mut buf)?;
        } else {
            ParticleFlucInfo::default_write(&mut buf)?;
        }

        if let Some(s) = &self.sampler0 { s.write(&mut buf, version)?; }
        if let Some(s) = &self.sampler1 { s.write(&mut buf, version)?; }
        if let Some(s) = &self.sampler2 { s.write(&mut buf, version)?; }
        if has_gt_40 {
            if let Some(s) = &self.sampler3 { s.write(&mut buf, version)?; }
            if let Some(s) = &self.sampler4 { s.write(&mut buf, version)?; }
            if let Some(s) = &self.sampler5 { s.write(&mut buf, version)?; }
        }

        if let Some(a) = &self.texture_anim0 { a.write(&mut buf, version)?; }
        if let Some(a) = &self.texture_anim1 { a.write(&mut buf, version)?; }
        if let Some(a) = &self.texture_anim2 { a.write(&mut buf, version)?; }
        if has_gt_40 {
            if let Some(a) = &self.texture_anim3 { a.write(&mut buf, version)?; }
            if let Some(a) = &self.texture_anim4 { a.write(&mut buf, version)?; }
            if let Some(a) = &self.texture_anim5 { a.write(&mut buf, version)?; }
        }

        if has_ge_22 {
            let reserved = if self.reserved.len() >= 0x40 {
                &self.reserved[..0x40]
            } else {
                &self.reserved
            };
            if reserved.len() < 0x40 {
                buf.write_bytes(reserved)?;
                buf.write_bytes(&vec![0u8; 0x40 - reserved.len()])?;
            } else {
                buf.write_bytes(reserved)?;
            }
        }

        Ok(buf)
    }
}

impl CombinedEmitterCombinerV40 {
    pub fn write_combiner_body<W: WriterExt>(&self, writer: &mut W, version: u16) -> io::Result<()> {
        writer.write_u8(self.color_combiner_process)?;
        writer.write_u8(self.alpha_combiner_process)?;
        writer.write_u8(self.texture1_color_blend)?;
        writer.write_u8(self.texture2_color_blend)?;
        writer.write_u8(self.primitive_color_blend)?;
        writer.write_u8(self.texture1_alpha_blend)?;
        writer.write_u8(self.texture2_alpha_blend)?;
        writer.write_u8(self.primitive_alpha_blend)?;
        writer.write_u8(self.tex_color0_input_type)?;
        writer.write_u8(self.tex_color1_input_type)?;
        writer.write_u8(self.tex_color2_input_type)?;
        writer.write_u8(self.tex_alpha0_input_type)?;
        writer.write_u8(self.tex_alpha1_input_type)?;
        writer.write_u8(self.tex_alpha2_input_type)?;
        writer.write_u8(self.primitive_color_input_type)?;
        writer.write_u8(self.primitive_alpha_input_type)?;
        if version >= 50 {
            writer.write_i16_le(self.padding.unwrap_or(0))?;
            writer.write_u32_le(self.padding2_opt.unwrap_or(0))?;
            writer.write_u32_le(self.padding3.unwrap_or(0))?;
        }
        Ok(())
    }
}

impl ParticleFlucInfo {
    pub fn default_write<W: WriterExt>(writer: &mut W) -> io::Result<()> {
        for _ in 0..8 {
            writer.write_u8(0)?;
        }
        writer.write_u32_le(0)?;
        Ok(())
    }
}
"""


def main() -> None:
    text = EMITTER.read_text()
    blocks = extract_impl_blocks(text)
    parts = [HEADER]
    for type_name, body, _ in blocks:
        gen = generate_type_write(type_name, body)
        if gen:
            parts.append(gen)
            parts.append("")
    parts.append(emitter_data_write())
    OUT.write_text("\n".join(parts))
    print(f"Wrote {OUT} ({len(parts)} sections from {len(blocks)} read impls)")


if __name__ == "__main__":
    main()
