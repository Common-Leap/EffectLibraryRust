fn main() {
    use std::fs;
    let eff_data = fs::read("/home/leap/Workshop/EffectLibraryRust/EFF and baseline/ef_samus.eff").unwrap();
    let namco = effect_library::NamcoEffectFile::load(&eff_data).unwrap();
    
    if let Some(ptcl) = &namco.ptcl_file {
        println!("Shader info: {:?}", ptcl.shader_info.is_some());
        if let Some(shader_info) = &ptcl.shader_info {
            println!("  Variations: {}", shader_info.variations.len());
            println!("  Binary data: {} bytes", shader_info.binary_data.as_ref().map(|d| d.len()).unwrap_or(0));
        }
        println!("Primitive info: {:?}", ptcl.primitive_info.is_some());
        if let Some(prim_info) = &ptcl.primitive_info {
            println!("  Descriptors: {}", prim_info.descriptors.len());
            println!("  Binary data: {} bytes", prim_info.binary_data.as_ref().map(|d| d.len()).unwrap_or(0));
        }
    }
}
