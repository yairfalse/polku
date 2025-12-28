fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Central proto repo is at ../../proto/ relative to gateway/
    let proto_root = "../../proto";

    // Tell Cargo to rerun if the proto files change
    println!("cargo:rerun-if-changed={proto_root}/ahti/v1/events.proto");
    println!("cargo:rerun-if-changed={proto_root}/polku/v1/gateway.proto");

    // Compile ahti protos first
    tonic_build::configure()
        .build_server(false)
        .build_client(false)
        .out_dir("src/proto")
        .compile_protos(&[format!("{proto_root}/ahti/v1/events.proto")], &[proto_root])?;

    // Compile polku protos with extern_path to reference ahti types
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir("src/proto")
        .extern_path(".ahti.v1", "crate::proto::ahti")
        .compile_protos(
            &[format!("{proto_root}/polku/v1/gateway.proto")],
            &[proto_root],
        )?;

    Ok(())
}
