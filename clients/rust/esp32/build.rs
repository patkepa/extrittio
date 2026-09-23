fn main() {
    println!("cargo:rerun-if-env-changed=EXTRITTIO_DEVICE_CONTRACT");
    let source = std::env::var_os("EXTRITTIO_DEVICE_CONTRACT")
        .expect("Set EXTRITTIO_DEVICE_CONTRACT to the provisioned device contract JSON response");
    let source = std::fs::canonicalize(source).expect("Cannot locate provisioned contract");
    println!("cargo:rerun-if-changed={}", source.display());
    let bytes = std::fs::read(source).expect("Cannot read provisioned contract");
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(output.join("device-contract.json"), bytes).expect("Cannot embed contract");
    embuild::espidf::sysenv::output();
}
