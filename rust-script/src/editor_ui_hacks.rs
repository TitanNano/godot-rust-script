/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use godot::engine::{EditorInterface, Engine};
use godot::log::godot_warn;
use godot::meta::ToGodot;
use godot::prelude::GodotConvert;

#[derive(Clone, Copy)]
pub enum EditorToaserSeverity {
    Warning,
}

impl From<EditorToaserSeverity> for u8 {
    fn from(value: EditorToaserSeverity) -> Self {
        use EditorToaserSeverity::*;

        match value {
            Warning => 1,
        }
    }
}

impl GodotConvert for EditorToaserSeverity {
    type Via = u8;
}

impl ToGodot for EditorToaserSeverity {
    fn to_godot(&self) -> Self::Via {
        (*self).into()
    }
}

pub fn show_editor_toast(message: &str, severity: EditorToaserSeverity) {
    if !Engine::singleton().is_editor_hint() {
        return;
    }

    let Some(base_control) = EditorInterface::singleton().get_base_control() else {
        godot_warn!("[godot-rust-script] unable to access editor UI!");
        return;
    };

    let editor_toaser = base_control
        .find_children_ex("*".into())
        .type_("EditorToaster".into())
        .recursive(true)
        .owned(false)
        .done()
        .get(0);

    let Some(mut editor_toaser) = editor_toaser else {
        godot_warn!("[godot-rust-script] unable to access editor toast notifications!");
        return;
    };

    if !editor_toaser.has_method("_popup_str".into()) {
        godot_warn!("[godot-rust-script] Internal toast notifications API no longer exists!");
        return;
    }

    editor_toaser.call(
        "_popup_str".into(),
        &[message.to_variant(), severity.to_variant(), "".to_variant()],
    );
}
