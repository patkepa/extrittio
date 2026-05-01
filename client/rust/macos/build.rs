fn main() {
    let plist_path = std::path::Path::new("Info.plist")
        .canonicalize()
        .expect("Info.plist not found in client/macos/");

    println!("cargo::rustc-link-arg=-sectcreate");
    println!("cargo::rustc-link-arg=__TEXT");
    println!("cargo::rustc-link-arg=__info_plist");
    println!("cargo::rustc-link-arg={}", plist_path.display());
    println!("cargo::rerun-if-changed=Info.plist");
}
