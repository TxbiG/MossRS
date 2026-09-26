use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    println!("cargo:rerun-if-env-changed=MOSS_DIR");
    println!("cargo:rerun-if-env-changed=MOSS_BUILD_DIR");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let repo_root = manifest_dir.join("../..");
    let bridge_dir = repo_root.join("native/moss_bridge");

    let moss_dir = env::var_os("MOSS_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            let sibling = repo_root.join("../Moss");
            if sibling.exists() { Some(sibling) } else { None }
        });

    let mut build = cc::Build::new();
    build.cpp(true)
        .std("c++17")
        .include(bridge_dir.join("include"))
        .include(bridge_dir.join("src"));

    if let Some(ref moss) = moss_dir {
        build.include(moss.join("include"));
        build.include(moss.join("src"));
        println!("cargo:rerun-if-changed={}", moss.display());
    }

    build.file(bridge_dir.join("src/core.cpp"));

    if cfg!(target_os = "windows") {
        build.define("_CRT_SECURE_NO_WARNINGS", None);
    }

    build.compile("moss_bridge");

    // The bridge is intentionally the first buildable layer. As the Moss
    // checkout exposes stable C++ targets, link them here rather than making
    // Rust depend directly on the C++ ABI.
    if let Some(build_dir) = env::var_os("MOSS_BUILD_DIR") {
        println!("cargo:rustc-link-search=native={}", Path::new(&build_dir).display());
    }

    let bindings = bindgen::Builder::default()
        .header(bridge_dir.join("include/moss_bridge/core.h").display().to_string())
        .allowlist_type("Moss.*")
        .allowlist_function("moss_.*")
        .allowlist_var("MOSS_.*")
        .generate_comments(false)
        .layout_tests(false)
        .generate()
        .expect("failed to generate Moss bindings");

    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out.join("bindings.rs"))
        .expect("failed to write bindings");

    // Keep the bridge header available to IDEs/build diagnostics.
    if let Some(moss) = moss_dir {
        println!("cargo:rustc-env=MOSS_DIR={}", moss.display());
    }

    // Helpful warning only; no dependency on the upstream repository being
    // present is required to generate the Rust FFI layer.
    if !bridge_dir.exists() {
        panic!("MossRS bridge directory is missing: {}", bridge_dir.display());
    }

    let _ = Command::new("cmake").arg("--version").output();
}
