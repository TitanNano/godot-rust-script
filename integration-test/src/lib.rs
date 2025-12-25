/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

mod scripts;

use godot::prelude::{gdextension, ExtensionLibrary, InitLevel};

struct ExtensionLib;

#[gdextension]
unsafe impl ExtensionLibrary for ExtensionLib {
    fn on_level_init(level: InitLevel) {
        match level {
            InitLevel::Scene => godot_rust_script::init!(scripts),
            InitLevel::Editor | InitLevel::Servers | InitLevel::Core => (),
        }
    }

    fn on_level_deinit(level: InitLevel) {
        match level {
            InitLevel::Scene => godot_rust_script::deinit!(),
            InitLevel::Servers | InitLevel::Core | InitLevel::Editor => (),
        }
    }
}