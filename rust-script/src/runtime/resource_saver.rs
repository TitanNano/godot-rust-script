use godot::{
    engine::{
        file_access, global, resource_saver::SaverFlags, FileAccess, ResourceFormatSaverVirtual,
        Script,
    },
    obj::EngineEnum,
    prelude::{godot_api, godot_print, Gd, GodotClass, GodotString, PackedStringArray, Resource},
};

use super::rust_script::RustScript;

#[derive(GodotClass)]
#[class(base = ResourceFormatSaver, init, tool)]
pub struct RustScriptResourceSaver;

#[godot_api]
impl ResourceFormatSaverVirtual for RustScriptResourceSaver {
    fn save(&mut self, resource: Gd<Resource>, path: GodotString, flags: u32) -> global::Error {
        let mut script: Gd<Script> = resource.cast();

        godot_print!("saving rust script resource to: {}", path);

        if flags as i32 & SaverFlags::FLAG_CHANGE_PATH.ord() > 0 {
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
        resource.try_cast::<RustScript>().is_some()
    }
    fn get_recognized_extensions(&self, _resource: Gd<Resource>) -> PackedStringArray {
        PackedStringArray::from(&[GodotString::from("rs")])
    }
    fn recognize_path(&self, _resource: Gd<Resource>, _path: GodotString) -> bool {
        true
    }
}
