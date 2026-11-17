fn main() {
    cc::Build::new()
        .std("c11")
        .opt_level(2)
        .include("vendor/xwax")
        .file("vendor/xwax/timecoder.c")
        .file("vendor/xwax/lut.c")
        .file("src/bridge.c")
        .compile("drdj_xwax");
    println!("cargo:rerun-if-changed=vendor/xwax");
    println!("cargo:rerun-if-changed=src/bridge.c");
}
