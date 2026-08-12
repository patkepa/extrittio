use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_EMBEDDED_UI");
    if std::env::var_os("CARGO_FEATURE_EMBEDDED_UI").is_none() {
        return;
    }

    let frontend = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../../apps/frontend/dist");
    println!("cargo:rerun-if-changed={}", frontend.display());
    if !frontend.join("index.html").is_file() {
        panic!(
            "the `embedded-ui`/`hobby` feature requires apps/frontend/dist; run `npm ci && npm run build` in apps/frontend before cargo build/install"
        );
    }
}
