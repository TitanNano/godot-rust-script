/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use godot::{
    engine::{
        file_access, global, resource_saver::SaverFlags, FileAccess, IResourceFormatSaver, Script,
    },
    obj::EngineBitfield,
    prelude::{godot_api, godot_print, GString, Gd, GodotClass, PackedStringArray, Resource},
};

use super::rust_script::RustScript;

#[derive(GodotClass)]
#[class(base = ResourceFormatSaver, init, tool)]
pub struct RustScriptResourceSaver;

#[godot_api]
impl IResourceFormatSaver for RustScriptResourceSaver {
    fn save(&mut self, resource: Gd<Resource>, path: GString, flags: u32) -> global::Error {
        let mut script: Gd<Script> = resource.cast();

        godot_print!("saving rust script resource to: {}", path);

        if flags as u64 & SaverFlags::FLAG_CHANGE_PATH.ord() > 0 {
            script.set_path(path.clone());
        }

        let handle = FileAccess::open(path, file_access::ModeFlags::WRITE);

        let mut handle = match handle {
            Some(handle) => handle,
            None => {
                return global::Error::FAILED;
            }
        };

        handle.store_string(script.get_source_code());
        handle.close();

        global::Error::OK
    }
    fn recognize(&self, resource: Gd<Resource>) -> bool {
        resource.try_cast::<RustScript>().is_ok()
    }
    fn get_recognized_extensions(&self, _resource: Gd<Resource>) -> PackedStringArray {
        PackedStringArray::from(&[GString::from("rs")])
    }
    fn recognize_path(&self, _resource: Gd<Resource>, _path: GString) -> bool {
        true
    }
}
