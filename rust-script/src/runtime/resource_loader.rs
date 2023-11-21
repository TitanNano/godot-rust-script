use godot::{
    engine::{file_access, FileAccess, IResourceFormatLoader, IScriptLanguageExtension, Script},
    prelude::{
        godot_api, GString, Gd, GodotClass, PackedStringArray, StringName, ToGodot, Variant,
    },
};

use super::{rust_script::RustScript, rust_script_language::RustScriptLanguage};

#[derive(GodotClass)]
#[class(base = ResourceFormatLoader, tool)]
pub(super) struct RustScriptResourceLoader {
    script_lang: Gd<RustScriptLanguage>,
}

impl RustScriptResourceLoader {
    pub fn new(script_lang: Gd<RustScriptLanguage>) -> Gd<Self> {
        Gd::from_object(Self { script_lang })
    }
}

#[godot_api]
impl IResourceFormatLoader for RustScriptResourceLoader {
    fn handles_type(&self, type_: StringName) -> bool {
        type_ == StringName::from("Script") || type_ == self.script_lang.bind().get_type().into()
    }

    fn get_resource_type(&self, path: GString) -> GString {
        let script_lang = self.script_lang.bind();
        let ext_match = path
            .to_string()
            .ends_with(&script_lang.get_extension().to_string());

        if !ext_match {
            return GString::new();
        }

        script_lang.get_type()
    }

    fn get_recognized_extensions(&self) -> PackedStringArray {
        PackedStringArray::from(&[self.script_lang.bind().get_extension()])
    }

    fn load(
        &self,
        path: GString,
        original_path: GString,
        _use_sub_threads: bool,
        _cache_mode: i32,
    ) -> Variant {
        let class_name = RustScriptLanguage::path_to_class_name(&path);

        let handle = FileAccess::open(path, file_access::ModeFlags::READ).unwrap();

        let mut script: Gd<Script> = RustScript::new(class_name).upcast();

        script.set_path(original_path);
        script.set_source_code(handle.get_as_text());

        script.to_variant()
    }
}
