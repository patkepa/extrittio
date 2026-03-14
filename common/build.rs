use std::io::Result;

fn main() -> Result<()> {
    // Note: #[cfg(feature = "...")] is evaluated for the *build script* binary,
    // not the crate being built, so it would never see crate features.
    // Use the CARGO_FEATURE_<NAME> env var instead.
    if std::env::var("CARGO_FEATURE_STD").is_ok() {
        prost_build::compile_protos(&["src/protos/telemetry.proto"], &["src/protos/"])?;
    }
    Ok(())
}
