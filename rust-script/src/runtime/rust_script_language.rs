/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::ffi::OsStr;

use godot::{
    engine::{Engine, FileAccess, IScriptLanguageExtension, ProjectSettings, Script},
    log::godot_print,
    obj::Base,
    prelude::{
        godot_api, Array, Dictionary, GString, Gd, GodotClass, Object, PackedStringArray,
        VariantArray,
    },
};
use itertools::Itertools;

use crate::{apply::Apply, RustScriptMetaData};

use super::{rust_script::RustScript, SCRIPT_REGISTRY};

#[derive(GodotClass)]
#[class(base = ScriptLanguageExtension, tool)]
pub(super) struct RustScriptLanguage {
    scripts_src_dir: Option<&'static str>,
}

#[godot_api]
impl RustScriptLanguage {
    pub fn new(scripts_src_dir: Option<&'static str>) -> Gd<Self> {
        Gd::from_object(Self { scripts_src_dir })
    }

    pub fn path_to_class_name(path: &GString) -> String {
        std::path::Path::new(&path.to_string())
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap()
            .rsplit_once('.')
            .unwrap()
            .0
            .split('_')
            .map(|part| {
                let mut chars = part.chars();
                let first = chars.next().unwrap();

                let part: String = first.to_uppercase().chain(chars).collect();

                part
            })
            .join("")
    }

    pub fn singleton() -> Option<Gd<Self>> {
        Engine::singleton()
            .get_singleton(RustScriptLanguage::class_name().to_string_name())
            .map(|gd| gd.cast())
    }

    pub fn script_meta_data(class_name: &str) -> Option<RustScriptMetaData> {
        let reg = SCRIPT_REGISTRY
            .read()
            .expect("unable to obtain read access");

        reg.get(class_name).map(ToOwned::to_owned)
    }
}

#[godot_api]
impl IScriptLanguageExtension for RustScriptLanguage {
    fn get_name(&self) -> GString {
        GString::from("RustScript")
    }

    fn get_type(&self) -> GString {
        GString::from("RustScript")
    }

    fn get_extension(&self) -> GString {
        GString::from("rs")
    }

    fn supports_documentation(&self) -> bool {
        true
    }

    /// thread enter hook will be called before entering a thread
    fn thread_enter(&mut self) {}

    /// thread exit hook will be called before leaving a thread
    fn thread_exit(&mut self) {}

    fn get_public_functions(&self) -> Array<Dictionary> {
        Array::new()
    }

    fn get_public_constants(&self) -> Dictionary {
        Dictionary::new()
    }

    fn get_public_annotations(&self) -> Array<Dictionary> {
        Array::new()
    }

    /// frame hook will be called for each reandered frame
    /// fn frame(&mut self) {}

    fn handles_global_class_type(&self, type_: GString) -> bool {
        type_ == self.get_type()
    }

    fn get_recognized_extensions(&self) -> PackedStringArray {
        PackedStringArray::from(&[self.get_extension()])
    }

    fn has_named_classes(&self) -> bool {
        true
    }

    fn supports_builtin_mode(&self) -> bool {
        false
    }

    fn can_inherit_from_file(&self) -> bool {
        false
    }

    fn is_using_templates(&mut self) -> bool {
        false
    }

    fn init(_base: Base<Self::Base>) -> Self {
        Self {
            scripts_src_dir: None,
        }
    }

    /// validate that the path of a new rust script is valid. Constraints for script locations can be enforced here.
    fn validate_path(&self, path: GString) -> GString {
        let Some(rs_root) = self
            .scripts_src_dir
            .map(|path| ProjectSettings::singleton().localize_path(path.into()))
        else {
            return GString::from("Unable to validate script location! RustScript source location is known in the current execution context.");
        };

        if !path.to_string().starts_with(&rs_root.to_string()) {
            return GString::from("rust file is not part of the scripts crate!");
        }

        if !FileAccess::file_exists(path.clone()) {
            return GString::from("RustScripts can not be created via the Godot editor!");
        }

        if !self.get_global_class_name(path).contains_key("name") {
            return GString::from("Rust script has not been complied into shared library yet!");
        }

        GString::new()
    }

    fn make_template(
        &self,
        _template: GString,
        _class_name: GString,
        _base_class_name: GString,
    ) -> Option<Gd<Script>> {
        None
    }

    fn create_script(&self) -> Option<Gd<Object>> {
        Some(RustScript::new(String::new()).upcast())
    }

    fn get_reserved_words(&self) -> PackedStringArray {
        PackedStringArray::new()
    }

    fn get_global_class_name(&self, path: GString) -> Dictionary {
        let class_name = Self::path_to_class_name(&path);

        let Some(script) = Self::script_meta_data(&class_name) else {
            return Dictionary::new();
        };

        Dictionary::new().apply(|dict| {
            dict.set("name", class_name);
            dict.set("base_type", script.base_type_name());
        })
    }

    fn overrides_external_editor(&mut self) -> bool {
        false
    }

    fn get_string_delimiters(&self) -> PackedStringArray {
        PackedStringArray::from(&[GString::from("\"")])
    }

    fn get_comment_delimiters(&self) -> PackedStringArray {
        PackedStringArray::from(&[GString::from("//")])
    }

    fn validate(
        &self,
        _script: GString,
        _path: GString,
        _validate_functions: bool,
        _validate_errors: bool,
        _validate_warnings: bool,
        _validate_safe_lines: bool,
    ) -> Dictionary {
        let mut validation = Dictionary::new();

        validation.insert("valid", "true");
        validation.insert("errors", VariantArray::new());
        validation.insert("functions", VariantArray::new());
        validation.insert("warnings", VariantArray::new());

        validation
    }

    fn debug_get_current_stack_info(&mut self) -> Array<Dictionary> {
        Array::default()
    }

    // godot hook to trigger script reload
    fn reload_all_scripts(&mut self) {}

    fn frame(&mut self) {
        godot_print!("script language frame called!");
    }
}
