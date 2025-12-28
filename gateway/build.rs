fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Tell Cargo to rerun if the proto files change
    println!("cargo:rerun-if-changed=../proto/v1/gateway.proto");

    // Compile the proto files
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir("src/proto")
        .compile_protos(&["../proto/v1/gateway.proto"], &["../proto"])?;

    Ok(())
}
