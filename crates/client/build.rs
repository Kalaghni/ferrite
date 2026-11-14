//! Gives ferrite.exe its icon on Windows. Nothing to do elsewhere.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../packaging/ferrite.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../packaging/ferrite.ico");
        res.set("ProductName", "FERRITE");
        res.set("FileDescription", "FERRITE");
        // An icon is a nicety: without a resource compiler the build goes on.
        if let Err(e) = res.compile() {
            println!("cargo:warning=no icon embedded in ferrite.exe: {e}");
        }
    }
}
