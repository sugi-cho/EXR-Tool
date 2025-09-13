use std::env;
use std::path::PathBuf;

#[cfg(feature = "use_ocio")]
fn main() {
    // Build OCIO FFI only when feature is enabled
    let pkg = pkg_config::Config::new()
        .probe("OpenColorIO")
        .expect("Could not find OpenColorIO via pkg-config");

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .file("ffi/ocio_c_api.cpp")
        .flag_if_supported("-std=c++17");
    for p in &pkg.include_paths {
        build.include(p);
    }
    build.compile("ocio_c_api");

    let mut bindings = bindgen::Builder::default()
        .header("ffi/ocio_c_api.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));
    for p in &pkg.include_paths {
        bindings = bindings.clang_arg(format!("-I{}", p.display()));
    }
    let bindings = bindings.generate().expect("Unable to generate bindings");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("ocio_bindings.rs"))
        .expect("Couldn't write bindings");
}

#[cfg(not(feature = "use_ocio"))]
fn main() {
    // No-op when OCIO feature is disabled
    println!("cargo:warning=feature 'use_ocio' disabled; skipping OCIO build script");
}
