/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

pub mod negative_tests;
mod scripts;

use godot::{
    init::InitStage,
    prelude::{ExtensionLibrary, gdextension},
};

struct ExtensionLib;

#[gdextension]
unsafe impl ExtensionLibrary for ExtensionLib {
    fn on_stage_init(level: InitStage) {
        if level == InitStage::Scene {
            godot_rust_script::init!(scripts)
        }
    }

    fn on_stage_deinit(level: InitStage) {
        if level == InitStage::Scene {
            godot_rust_script::deinit!()
        }
    }
}
