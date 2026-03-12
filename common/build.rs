use std::io::Result;

fn main() -> Result<()> {
    #[cfg(feature = "std")]
    prost_build::compile_protos(&["src/protos/telemetry.proto"], &["src/protos/"])?;
    Ok(())
}
