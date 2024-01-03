/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

godot_rust_script::setup!(tests_scripts_lib);

#[test]
fn verify_macros() {
    let _ = || {
        godot_rust_script::init!(tests_scripts_lib);
    };

    let _ = || {
        godot_rust_script::deinit!();
    };
}
