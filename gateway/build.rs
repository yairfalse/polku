fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Central proto repo is at ../../proto/ relative to gateway/
    let proto_root = "../../proto";

    // Tell Cargo to rerun if the proto files change
    println!("cargo:rerun-if-changed={proto_root}/polku/v1/gateway.proto");

    // Compile polku proto - it's now self-contained
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir("src/proto")
        .compile_protos(
            &[format!("{proto_root}/polku/v1/gateway.proto")],
            &[proto_root],
        )?;

    Ok(())
}
