use std::io::Result;

fn main() -> Result<()> {
    prost_build::compile_protos(&["src/protos/telemetry.proto"], &["src/protos/"])?;
    Ok(())
}
