fn main() {
    println!("cargo:rerun-if-changed=data/ward.gresource.xml");
    println!("cargo:rerun-if-changed=data/io.github.ward.Ward.svg");
    if std::env::var_os("CARGO_FEATURE_GUI").is_none() {
        return;
    }
    let output =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo output directory"));
    let status = std::process::Command::new("glib-compile-resources")
        .args(["--sourcedir=data", "--target"])
        .arg(output.join("ward.gresource"))
        .arg("data/ward.gresource.xml")
        .status()
        .expect("Install GLib development tools (glib-compile-resources)");
    assert!(status.success(), "Unable to compile Ward resources");
}
