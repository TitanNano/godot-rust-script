/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use godot::classes::{
    file_access, ClassDb, FileAccess, IResourceFormatLoader, IScriptLanguageExtension, Script,
};
use godot::global::godot_print;
use godot::obj::Base;
use godot::prelude::{
    godot_api, GString, Gd, GodotClass, PackedStringArray, StringName, ToGodot, Variant,
};

use super::{rust_script::RustScript, rust_script_language::RustScriptLanguage};

#[derive(GodotClass)]
#[class(base = ResourceFormatLoader, tool)]
pub(super) struct RustScriptResourceLoader {
    script_lang: Option<Gd<RustScriptLanguage>>,
}

impl RustScriptResourceLoader {
    pub fn new(script_lang: Gd<RustScriptLanguage>) -> Gd<Self> {
        let mut inst: Gd<Self> = ClassDb::singleton()
            .instantiate(StringName::from("RustScriptResourceLoader"))
            .to();

        inst.bind_mut().script_lang = Some(script_lang);

        inst
    }

    fn script_lang(&self) -> &Gd<RustScriptLanguage> {
        match self.script_lang {
            Some(ref lang) => lang,
            None => panic!("script_lang can not be used before setting it!"),
        }
    }
}

#[godot_api]
impl IResourceFormatLoader for RustScriptResourceLoader {
    fn init(_base: Base<Self::Base>) -> Self {
        Self { script_lang: None }
    }

    fn handles_type(&self, type_: StringName) -> bool {
        type_ == StringName::from("Script") || type_ == self.script_lang().bind().get_type().into()
    }

    fn get_resource_type(&self, path: GString) -> GString {
        let script_lang = self.script_lang().bind();
        let ext_match = path
            .to_string()
            .ends_with(&script_lang.get_extension().to_string());

        if !ext_match {
            return GString::new();
        }

        if !script_lang.validate_path(path).is_empty() {
            return GString::new();
        }

        script_lang.get_type()
    }

    fn get_recognized_extensions(&self) -> PackedStringArray {
        PackedStringArray::from(&[self.script_lang().bind().get_extension()])
    }

    fn load(
        &self,
        path: GString,
        original_path: GString,
        _use_sub_threads: bool,
        _cache_mode: i32,
    ) -> Variant {
        godot_print!("loading script with path: {}, {}", path, original_path);

        let class_name = RustScriptLanguage::path_to_class_name(&path);

        let handle = FileAccess::open(path, file_access::ModeFlags::READ).unwrap();
        let rust_script = RustScript::new(class_name);

        let mut script: Gd<Script> = rust_script.upcast();

        script.set_source_code(handle.get_as_text());

        script.to_variant()
    }
}
