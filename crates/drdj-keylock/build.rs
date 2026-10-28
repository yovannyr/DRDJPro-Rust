fn main() {
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .flag_if_supported("/Zc:__cplusplus")
        .include("vendor")
        .include("vendor/signalsmith-stretch")
        .file("vendor/wrapper.cpp")
        .file("src/bridge.cpp")
        .compile("drdj_stretch");
    println!("cargo:rerun-if-changed=vendor");
    println!("cargo:rerun-if-changed=src/bridge.cpp");
}
