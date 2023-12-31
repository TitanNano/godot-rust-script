godot_rust_script::setup!(tests_scripts_lib);

#[test]
fn verify_macros() {
    let _ = || {
        godot_rust_script::init!();
    };

    let _ = || {
        godot_rust_script::deinit!();
    };
}
