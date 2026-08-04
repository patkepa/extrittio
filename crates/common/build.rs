#[cfg(feature = "std")]
fn main() -> std::io::Result<()> {
    prost_build::compile_protos(&["src/protos/telemetry.proto"], &["src/protos/"])?;
    Ok(())
}

#[cfg(not(feature = "std"))]
fn main() {}
