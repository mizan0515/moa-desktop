fn main() {
    #[cfg(windows)]
    embed_test_common_controls_manifest();

    tauri_build::build()
}

#[cfg(windows)]
fn embed_test_common_controls_manifest() {
    let out_dir = std::path::PathBuf::from(
        std::env::var_os("OUT_DIR").expect("OUT_DIR must be set by Cargo"),
    );
    let manifest_path = out_dir.join("moa-tests-common-controls-v6.manifest");
    std::fs::write(
        &manifest_path,
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*" />
    </dependentAssembly>
  </dependency>
</assembly>
"#,
    )
    .expect("write Windows common-controls manifest for test binaries");

    // T20: AppHandle-bearing integration tests link Tauri/Wry symbols that
    // import comctl32 v6 entrypoints such as SetWindowSubclass. Cargo test
    // binaries do not inherit the normal Tauri app manifest, so Windows can
    // resolve comctl32 without those exports and terminate before `--list`
    // with 0xc0000139 STATUS_ENTRYPOINT_NOT_FOUND.
    println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-tests=/MANIFESTINPUT:{}",
        manifest_path.display()
    );
}
