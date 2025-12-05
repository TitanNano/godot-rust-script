/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::ffi::{c_void, OsStr};

use godot::classes::class_macros::private::virtuals::Os::VarDictionary;
use godot::classes::native::ScriptLanguageExtensionProfilingInfo;
#[cfg(since_api = "4.3")]
use godot::classes::script_language::ScriptNameCasing;
use godot::classes::{Engine, FileAccess, IScriptLanguageExtension, ProjectSettings, Script};
use godot::global::{self, godot_error};
use godot::obj::{Base, Singleton as _};
use godot::prelude::{
    godot_api, Array, GString, Gd, GodotClass, Object, PackedStringArray, StringName, VarArray,
    Variant,
};
use itertools::Itertools;

use crate::apply::Apply;
use crate::static_script_registry::RustScriptMetaData;

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
            .get_singleton(&RustScriptLanguage::class_id().to_string_name())
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

    fn get_public_functions(&self) -> Array<VarDictionary> {
        Array::new()
    }

    fn get_public_constants(&self) -> VarDictionary {
        VarDictionary::new()
    }

    fn get_public_annotations(&self) -> Array<VarDictionary> {
        Array::new()
    }

    /// frame hook will be called for each reandered frame
    fn frame(&mut self) {}

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
            .map(|path| ProjectSettings::singleton().localize_path(path))
        else {
            return GString::from("Unable to validate script location! RustScript source location is known in the current execution context.");
        };

        if !path.to_string().starts_with(&rs_root.to_string()) {
            return GString::from("rust file is not part of the scripts crate!");
        }

        if !FileAccess::file_exists(&path) {
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

    fn get_global_class_name(&self, path: GString) -> VarDictionary {
        let class_name = Self::path_to_class_name(&path);

        let Some(script) = Self::script_meta_data(&class_name) else {
            return VarDictionary::new();
        };

        VarDictionary::new().apply(|dict| {
            dict.set("name", class_name);
            dict.set("base_type", script.base_type_name());
        })
    }

    fn overrides_external_editor(&mut self) -> bool {
        true
    }

    fn open_in_external_editor(
        &mut self,
        _script: Option<Gd<Script>>,
        _line: i32,
        _col: i32,
    ) -> global::Error {
        // TODO: From Godot 4.4 we can show an editor toast here. Just waiting for a new gdext release.
        godot_error!("Editing rust scripts from inside Godot is currently not supported.");

        global::Error::OK
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
    ) -> VarDictionary {
        let mut validation = VarDictionary::new();

        validation.set("valid", "true");
        validation.set("errors", VarArray::new());
        validation.set("functions", VarArray::new());
        validation.set("warnings", VarArray::new());

        validation
    }

    // godot hook to trigger script reload
    fn reload_all_scripts(&mut self) {}

    fn init_ext(&mut self) {}

    fn finish(&mut self) {}

    fn is_control_flow_keyword(&self, #[expect(unused)] keyword: GString) -> bool {
        false
    }
    fn get_built_in_templates(&self, #[expect(unused)] object: StringName) -> Array<VarDictionary> {
        Array::new()
    }

    fn find_function(
        &self,
        #[expect(unused)] function: GString,
        #[expect(unused)] code: GString,
    ) -> i32 {
        0
    }

    #[expect(unused_variables)]
    fn make_function(
        &self,
        class_name: GString,
        function_name: GString,
        function_args: PackedStringArray,
    ) -> GString {
        GString::new()
    }

    #[cfg(since_api = "4.3")]
    fn can_make_function(&self) -> bool {
        false
    }

    #[cfg(since_api = "4.3")]
    fn preferred_file_name_casing(&self) -> ScriptNameCasing {
        ScriptNameCasing::SNAKE_CASE
    }

    #[expect(unused_variables)]
    fn complete_code(
        &self,
        code: GString,
        path: GString,
        owner: Option<Gd<Object>>,
    ) -> VarDictionary {
        VarDictionary::new()
    }

    #[expect(unused_variables)]
    fn lookup_code(
        &self,
        code: GString,
        symbol: GString,
        path: GString,
        owner: Option<Gd<Object>>,
    ) -> VarDictionary {
        VarDictionary::new()
    }

    fn auto_indent_code(
        &self,
        code: GString,
        #[expect(unused)] from_line: i32,
        #[expect(unused)] to_line: i32,
    ) -> GString {
        code
    }

    #[expect(unused_variables)]
    fn add_global_constant(&mut self, name: StringName, value: Variant) {}

    #[expect(unused_variables)]
    fn add_named_global_constant(&mut self, name: StringName, value: Variant) {}

    #[expect(unused_variables)]
    fn remove_named_global_constant(&mut self, name: StringName) {}

    fn debug_get_error(&self) -> GString {
        GString::new()
    }

    fn debug_get_stack_level_count(&self) -> i32 {
        0
    }

    #[expect(unused_variables)]
    fn debug_get_stack_level_line(&self, level: i32) -> i32 {
        0
    }

    #[expect(unused_variables)]
    fn debug_get_stack_level_function(&self, level: i32) -> GString {
        GString::new()
    }

    #[cfg(since_api = "4.3")]
    #[expect(unused_variables)]
    fn debug_get_stack_level_source(&self, level: i32) -> GString {
        GString::new()
    }

    #[expect(unused_variables)]
    fn debug_get_stack_level_locals(
        &mut self,
        level: i32,
        max_subitems: i32,
        max_depth: i32,
    ) -> VarDictionary {
        VarDictionary::new()
    }

    #[expect(unused_variables)]
    fn debug_get_stack_level_members(
        &mut self,
        level: i32,
        max_subitems: i32,
        max_depth: i32,
    ) -> VarDictionary {
        VarDictionary::new()
    }

    #[expect(unused_variables)]
    unsafe fn debug_get_stack_level_instance_rawptr(&mut self, level: i32) -> *mut c_void {
        unimplemented!("debugging is not implemented!");
    }

    #[expect(unused_variables)]
    fn debug_get_globals(&mut self, max_subitems: i32, max_depth: i32) -> VarDictionary {
        VarDictionary::new()
    }

    #[expect(unused_variables)]
    fn debug_parse_stack_level_expression(
        &mut self,
        level: i32,
        expression: GString,
        max_subitems: i32,
        max_depth: i32,
    ) -> GString {
        GString::new()
    }

    fn debug_get_current_stack_info(&mut self) -> Array<VarDictionary> {
        Array::default()
    }

    #[expect(unused_variables)]
    fn reload_tool_script(&mut self, script: Option<Gd<Script>>, soft_reload: bool) {}
    fn profiling_start(&mut self) {}
    fn profiling_stop(&mut self) {}

    #[cfg(since_api = "4.3")]
    #[expect(unused_variables)]
    fn profiling_set_save_native_calls(&mut self, enable: bool) {}

    #[expect(unused_variables)]
    unsafe fn profiling_get_accumulated_data_rawptr(
        &mut self,
        info_array: *mut ScriptLanguageExtensionProfilingInfo,
        info_max: i32,
    ) -> i32 {
        0
    }

    #[expect(unused_variables)]
    unsafe fn profiling_get_frame_data_rawptr(
        &mut self,
        info_array: *mut ScriptLanguageExtensionProfilingInfo,
        info_max: i32,
    ) -> i32 {
        0
    }

    #[cfg(since_api = "4.4")]
    #[expect(unused_variables)]
    fn reload_scripts(&mut self, scripts: Array<Variant>, soft: bool) {
        use godot::global::godot_warn;

        godot_warn!("Reloading Rust Scripts is currently a no-op!");
    }
}
