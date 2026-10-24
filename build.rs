fn main() {
    // Headless accessibility tests must also work without CI environment variables.
    let config = slint_build::CompilerConfiguration::new().with_debug_info(true);
    slint_build::compile_with_config("ui/app.slint", config).expect("Failed to compile Slint UI");
}
