use std::{fs, path::Path};

fn compile_shader(compiler: &mut shaderc::Compiler, shader: &str)
/* -> Result<Vec<u32>, ()> */
{
    let shader = Path::new(shader);
    let mut spv_extension = shader.extension().unwrap().to_str().unwrap().to_owned();
    spv_extension.push_str(".spv");
    let out_spv = shader.with_extension(spv_extension);

    if out_spv.exists() {
        println!("spv: {}", out_spv.to_str().unwrap());
        let spv_name = out_spv
            .to_str()
            .expect("Failed to get `out_spv` path as str");
        let spv_modification_date = out_spv
            .metadata()
            .unwrap_or_else(|_| panic!("Failed to get metadata of {}", spv_name))
            .modified()
            .unwrap_or_else(|_| panic!("Failed to get modification timestamp of {}", spv_name));
        let shader_name = shader.to_str().expect("Failed to get `shader` path as str");
        let src_modification_date = shader
            .metadata()
            .unwrap_or_else(|_| panic!("Failed to get metadata of {}", shader_name))
            .modified()
            .unwrap_or_else(|_| panic!("Failed to get modification timestamp of {}", shader_name));
        if spv_modification_date >= src_modification_date {
            return /* Err(()) */;
        }
    }

    let src = fs::read_to_string(&shader).unwrap();
    let extension = shader.extension().unwrap().to_str().unwrap();
    let shader_kind = match extension {
        "vert" => shaderc::ShaderKind::DefaultVertex,
        "frag" => shaderc::ShaderKind::DefaultFragment,
        _ => shaderc::ShaderKind::InferFromSource,
    };
    let spirv = compiler
        .compile_into_spirv(&src, shader_kind, shader.to_str().unwrap(), "main", None)
        .unwrap();
    fs::write(out_spv, spirv.as_binary_u8()).unwrap();
    // Ok(spirv.as_binary().to_vec())  // To get code to load into program.
}

fn main() {
    let mut compiler = shaderc::Compiler::new().expect("Failed to initialize glslc.");

    {
        let shader = "src/vkrs/shaders/shader.vert";
        compile_shader(&mut compiler, shader);
        println!("cargo:rerun-if-changed={}", shader);
    }

    {
        let shader = "src/vkrs/shaders/shader.frag";
        compile_shader(&mut compiler, shader);
        println!("cargo:rerun-if-changed={}", shader);
    }
}
