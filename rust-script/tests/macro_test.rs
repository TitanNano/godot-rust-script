godot_rust_script::setup!(tests_scripts_lib);

#[test]
fn verify_macros() {
    let _ = godot_rust_script::init!();

    // this won't compile if not in hot_reload mode
    let _ = scripts_lib::subscribe;
}
