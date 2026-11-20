// Builds realm-core (vendor/realm-core, tag v20.1.2 = the core inside
// osu!lazer's Realm .NET 20.1.0) as static libs and links its C API.
fn main() {
    let dst = cmake::Config::new("vendor/realm-core")
        .profile("Release")
        .define("REALM_BUILD_LIB_ONLY", "ON")
        .define("REALM_NO_TESTS", "ON")
        // lazer's client.realm isn't encrypted; skipping this drops OpenSSL.
        .define("REALM_ENABLE_ENCRYPTION", "OFF")
        .define("REALM_ENABLE_GEOSPATIAL", "OFF")
        // Match Rust's /MD CRT (realm-core defaults to static /MT on Windows).
        .define("CMAKE_MSVC_RUNTIME_LIBRARY", "MultiThreadedDLL")
        .build_target("RealmFFIStatic")
        .build();

    // build_target skips `install`, so pick the libs out of the build tree.
    let build = dst.join("build");
    for dir in ["src/realm", "src/realm/object-store", "src/realm/object-store/c_api", "src/realm/parser"] {
        println!("cargo:rustc-link-search=native={}", build.join(dir).display());
        println!("cargo:rustc-link-search=native={}", build.join(dir).join("Release").display());
    }
    for lib in ["realm-ffi-static", "realm-object-store", "realm-parser", "realm"] {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "linux" {
        println!("cargo:rustc-link-lib=stdc++");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
