use std::ffi::OsStr;

use godot::{
    engine::{FileAccess, ProjectSettings, Script, ScriptLanguageExtensionVirtual},
    prelude::{
        godot_api, Array, Base, Dictionary, Gd, GodotClass, GodotString, Object, PackedStringArray,
        VariantArray,
    },
};
use itertools::Itertools;

use crate::apply::Apply;

use super::rust_script::RustScript;

#[derive(GodotClass)]
#[class(base = ScriptLanguageExtension, tool)]
pub(super) struct RustScriptLanguage {
    scripts_src_dir: Option<&'static str>,
}

#[godot_api]
impl RustScriptLanguage {
    pub fn new(scripts_src_dir: Option<&'static str>) -> Gd<Self> {
        Gd::new(Self { scripts_src_dir })
    }

    pub fn path_to_class_name(path: &GodotString) -> String {
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
}

#[godot_api]
impl ScriptLanguageExtensionVirtual for RustScriptLanguage {
    fn get_name(&self) -> GodotString {
        GodotString::from("Rust")
    }

    fn get_type(&self) -> GodotString {
        GodotString::from("RustScript")
    }

    fn get_extension(&self) -> GodotString {
        GodotString::from("rs")
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
    fn frame(&mut self) {}

    fn handles_global_class_type(&self, type_: GodotString) -> bool {
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
    fn validate_path(&self, path: GodotString) -> GodotString {
        let Some(rs_root) = self
            .scripts_src_dir
            .map(|path| ProjectSettings::singleton().localize_path(path.into()))
        else {
            return GodotString::from("Unable to validate script location! RustScript source location is known in the current execution context.");
        };

        if !path.to_string().starts_with(&rs_root.to_string()) {
            return GodotString::from("rust file is not part of the scripts crate!");
        }

        if !FileAccess::file_exists(path) {
            return GodotString::from("RustScripts can not be created via the Godot editor!");
        }

        GodotString::new()
    }

    fn make_template(
        &self,
        _template: GodotString,
        _class_name: GodotString,
        _base_class_name: GodotString,
    ) -> Option<Gd<Script>> {
        None
    }

    fn create_script(&self) -> Option<Gd<Object>> {
        Some(RustScript::new(String::new()).upcast())
    }

    fn get_reserved_words(&self) -> PackedStringArray {
        PackedStringArray::new()
    }

    fn get_global_class_name(&self, path: GodotString) -> Dictionary {
        let class_name = Self::path_to_class_name(&path);

        Dictionary::new().apply(|dict| dict.set("name", class_name))
    }

    fn overrides_external_editor(&mut self) -> bool {
        false
    }

    fn get_string_delimiters(&self) -> PackedStringArray {
        PackedStringArray::from(&[GodotString::from("\"")])
    }

    fn get_comment_delimiters(&self) -> PackedStringArray {
        PackedStringArray::from(&[GodotString::from("//")])
    }

    fn validate(
        &self,
        _script: GodotString,
        _path: GodotString,
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
}
