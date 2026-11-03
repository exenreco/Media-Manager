fn main() {
    println!("cargo:rerun-if-changed=macos/DesktopPro.swift");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let target = if arch == "aarch64" {
        "arm64-apple-macosx13.0"
    } else {
        "x86_64-apple-macosx13.0"
    };
    let status = std::process::Command::new("xcrun")
        .args([
            "swiftc",
            "-parse-as-library",
            "-emit-library",
            "-static",
            "-O",
            "-module-name",
            "MediaManagerPro",
            "-module-cache-path",
        ])
        .arg(out.join("swift-module-cache"))
        .args(["-target", target, "macos/DesktopPro.swift", "-o"])
        .arg(out.join("libMediaManagerPro.a"))
        .status()
        .expect("Xcode Swift compiler is required");
    assert!(status.success(), "Desktop StoreKit bridge did not compile");
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=MediaManagerPro");
    for framework in ["Foundation", "StoreKit", "AppKit"] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
    let swift = std::process::Command::new("xcrun")
        .args(["--find", "swiftc"])
        .output()
        .unwrap();
    let compiler = std::path::PathBuf::from(String::from_utf8(swift.stdout).unwrap().trim());
    println!(
        "cargo:rustc-link-search=native={}",
        compiler
            .parent()
            .unwrap()
            .join("../lib/swift/macosx")
            .display()
    );
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
}
