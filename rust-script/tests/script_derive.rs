/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use godot::builtin::GString;
use godot_rust_script::godot_script_impl;
use godot_rust_script::GodotScript;

#[derive(GodotScript, Debug)]
struct TestScript {
    pub property_a: GString,
    #[export]
    pub editor_prop: u16,
}

#[godot_script_impl]
impl TestScript {
    pub fn record(&mut self, value: u8) -> bool {
        value > 2
    }
}
