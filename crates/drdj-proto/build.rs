use std::{env, error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let schema = "proto/drdj.proto";
    println!("cargo:rerun-if-changed={schema}");
    let out = PathBuf::from(env::var("OUT_DIR")?);
    let descriptor = out.join("descriptor.bin");
    let mut config = prost_build::Config::new();
    config
        .protoc_executable(protoc_bin_vendored::protoc_bin_path()?)
        .file_descriptor_set_path(&descriptor)
        .compile_well_known_types()
        .extern_path(".google.protobuf", "::pbjson_types")
        .compile_protos(
            &[schema],
            &[PathBuf::from("proto"), protoc_bin_vendored::include_path()?],
        )?;
    pbjson_build::Builder::new()
        .register_descriptors(&fs::read(descriptor)?)?
        .build(&[".drdj.v1"])?;
    // pbjson-build 0.7 emits a redundant borrow in its field-name diagnostic.
    // Correct the generated expression rather than suppressing Clippy warnings.
    let serde_path = out.join("drdj.v1.serde.rs");
    let generated = fs::read_to_string(&serde_path)?;
    fs::write(serde_path, generated.replace("&FIELDS)", "FIELDS)"))?;
    Ok(())
}
