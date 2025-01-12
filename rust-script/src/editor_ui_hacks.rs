/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use godot::classes::{EditorInterface, Engine};
use godot::global::godot_warn;
use godot::meta::ToGodot;
use godot::prelude::GodotConvert;

#[derive(Clone, Copy)]
pub enum EditorToasterSeverity {
    Warning,
}

impl From<EditorToasterSeverity> for u8 {
    fn from(value: EditorToasterSeverity) -> Self {
        use EditorToasterSeverity::*;

        match value {
            Warning => 1,
        }
    }
}

impl GodotConvert for EditorToasterSeverity {
    type Via = u8;
}

impl ToGodot for EditorToasterSeverity {
    type ToVia<'v> = Self::Via;

    fn to_godot(&self) -> Self::ToVia<'static> {
        (*self).into()
    }
}

pub fn show_editor_toast(message: &str, severity: EditorToasterSeverity) {
    if !Engine::singleton().is_editor_hint() {
        return;
    }

    #[cfg(before_api = "4.2")]
    let Some(base_control) = Engine::singleton()
        .get_singleton("EditorInterface")
        .and_then(|obj| obj.cast::<EditorInterface>().get_base_control())
    else {
        godot_warn!("[godot-rust-script] unable to access editor UI!");
        return;
    };

    #[cfg(since_api = "4.2")]
    let Some(base_control) = EditorInterface::singleton().get_base_control() else {
        godot_warn!("[godot-rust-script] unable to access editor UI!");
        return;
    };

    let editor_toaser = base_control
        .find_children_ex("*")
        .type_("EditorToaster")
        .recursive(true)
        .owned(false)
        .done()
        .get(0);

    let Some(mut editor_toaser) = editor_toaser else {
        godot_warn!("[godot-rust-script] unable to access editor toast notifications!");
        return;
    };

    if !editor_toaser.has_method("_popup_str") {
        godot_warn!("[godot-rust-script] Internal toast notifications API no longer exists!");
        return;
    }

    editor_toaser.call(
        "_popup_str",
        &[message.to_variant(), severity.to_variant(), "".to_variant()],
    );
}
