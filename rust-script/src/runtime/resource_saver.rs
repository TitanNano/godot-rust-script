/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::option::Option;

use godot::classes::{
    file_access, resource_saver::SaverFlags, FileAccess, IResourceFormatSaver, Script,
};
use godot::global::{self, godot_warn};
use godot::obj::EngineBitfield;
use godot::prelude::{
    godot_api, godot_print, GString, Gd, GodotClass, PackedStringArray, Resource,
};

use super::rust_script::RustScript;

#[derive(GodotClass)]
#[class(base = ResourceFormatSaver, init, tool)]
pub struct RustScriptResourceSaver;

#[godot_api]
impl IResourceFormatSaver for RustScriptResourceSaver {
    fn save(&mut self, resource: Option<Gd<Resource>>, path: GString, flags: u32) -> global::Error {
        let Some(resource) = resource else {
            godot_warn!("RustScriptResourceSaver: Unable to save a None resource!");
            return global::Error::FAILED;
        };

        let mut script: Gd<Script> = resource.cast();

        godot_print!("saving rust script resource to: {}", path);

        if flags as u64 & SaverFlags::CHANGE_PATH.ord() > 0 {
            script.set_path(&path);
        }

        if !script.has_source_code() {
            return global::Error::OK;
        }

        let handle = FileAccess::open(&path, file_access::ModeFlags::WRITE);

        let mut handle = match handle {
            Some(handle) => handle,
            None => {
                return global::Error::FAILED;
            }
        };

        handle.store_string(&script.get_source_code());
        handle.close();

        global::Error::OK
    }

    fn recognize(&self, resource: Option<Gd<Resource>>) -> bool {
        resource
            .map(|res| res.try_cast::<RustScript>().is_ok())
            .unwrap_or(false)
    }

    fn get_recognized_extensions(&self, _resource: Option<Gd<Resource>>) -> PackedStringArray {
        PackedStringArray::from(&[GString::from("rs")])
    }

    fn recognize_path(&self, _resource: Option<Gd<Resource>>, _path: GString) -> bool {
        true
    }
}
