use std::{ffi::c_void, ops::Deref};

use abi_stable::std_types::RBox;
use godot::{
    builtin::create_script_instance,
    engine::{Engine, IScriptExtension, Script, ScriptExtension, ScriptLanguage},
    obj::UserClass,
    prelude::{
        godot_api, Array, Base, Dictionary, GString, Gd, GodotClass, Object, StringName,
        VariantArray,
    },
};
use RemoteGodotScript_trait::RemoteGodotScript_TO;

use crate::{apply::Apply, script_registry::RemoteGodotScript_trait};

use super::{
    metadata::{ToDictionary, ToMethodDoc, ToPropertyDoc},
    rust_script_instance::{RustScriptInstance, RustScriptPlaceholder},
    rust_script_language::RustScriptLanguage,
    SCRIPT_REGISTRY,
};

#[derive(GodotClass)]
#[class(base = ScriptExtension, tool)]
pub(super) struct RustScript {
    class_name: String,
    source_code: String,
    base: Base<ScriptExtension>,
}

impl RustScript {
    pub fn new(class_name: String) -> Gd<Self> {
        Gd::from_init_fn(|base| Self {
            class_name,
            source_code: String::new(),
            base,
        })
    }

    pub fn class_name(&self) -> GString {
        self.class_name.clone().into()
    }

    pub fn str_class_name(&self) -> &str {
        &self.class_name
    }
    pub fn create_remote_instance(
        &self,
        base: Gd<Object>,
    ) -> RemoteGodotScript_TO<'static, RBox<()>> {
        SCRIPT_REGISTRY.with(|lock| {
            let reg = lock.read().expect("failed to obtain read lock");

            let meta_data = reg
                .get(&self.class_name)
                .expect("we musst know the class name at this point");

            meta_data.create_data(base)
        })
    }
}

#[godot_api]
impl IScriptExtension for RustScript {
    fn init(base: Base<Self::Base>) -> Self {
        Self {
            class_name: String::new(),
            source_code: String::new(),
            base,
        }
    }

    fn get_source_code(&self) -> GString {
        self.source_code.clone().into()
    }
    fn set_source_code(&mut self, code: GString) {
        self.source_code = code.to_string();
    }

    fn get_language(&self) -> Option<Gd<ScriptLanguage>> {
        Some(RustScriptLanguage::alloc_gd().upcast())
    }

    fn can_instantiate(&self) -> bool {
        self.is_tool() || !Engine::singleton().is_editor_hint()
    }

    fn get_instance_base_type(&self) -> StringName {
        SCRIPT_REGISTRY
            .with(|lock| {
                let reg = lock.read().expect("unable to obtain read lock");

                reg.get(&self.class_name)
                    .map(|class| class.base_type_name())
            })
            .unwrap_or_else(|| StringName::from("RefCounted"))
    }

    fn get_base_script(&self) -> Option<Gd<Script>> {
        None
    }

    fn is_tool(&self) -> bool {
        false
    }

    unsafe fn instance_create(&self, for_object: Gd<Object>) -> *mut c_void {
        let data = self.create_remote_instance(for_object.clone());
        let instance = RustScriptInstance::new(data, for_object, self.base.deref().clone().cast());

        create_script_instance(instance) as *mut c_void
    }

    unsafe fn placeholder_instance_create(&self, _for_object: Gd<Object>) -> *mut c_void {
        let placeholder = RustScriptPlaceholder::new(self.base.deref().clone().cast());

        create_script_instance(placeholder) as *mut c_void
    }

    fn is_valid(&self) -> bool {
        true
    }

    fn has_property_default_value(&self, _property: StringName) -> bool {
        false
    }

    fn get_script_signal_list(&self) -> Array<Dictionary> {
        Array::new()
    }

    fn update_exports(&mut self) {}

    fn get_script_method_list(&self) -> Array<Dictionary> {
        SCRIPT_REGISTRY.with(|lock| {
            let reg = lock.read().expect("unable to obtain read lock");

            reg.get(&self.class_name)
                .map(|class| {
                    class
                        .methods()
                        .iter()
                        .map(|method| method.to_dict())
                        .collect()
                })
                .unwrap_or_default()
        })
    }

    fn get_script_property_list(&self) -> Array<Dictionary> {
        SCRIPT_REGISTRY.with(|lock| {
            let reg = lock.read().expect("unable to obtain read lock");

            reg.get(&self.class_name)
                .map(|class| {
                    class
                        .properties()
                        .iter()
                        .map(|prop| prop.to_dict())
                        .collect()
                })
                .unwrap_or_default()
        })
    }

    fn has_method(&self, method_name: StringName) -> bool {
        SCRIPT_REGISTRY.with(|lock| {
            let reg = lock.read().expect("unable to obtain read lock");

            reg.get(&self.class_name).is_some_and(|class| {
                class
                    .methods()
                    .iter()
                    .any(|method| method.method_name == method_name)
            })
        })
    }

    fn get_constants(&self) -> Dictionary {
        Dictionary::new()
    }

    fn get_method_info(&self, method_name: StringName) -> Dictionary {
        SCRIPT_REGISTRY.with(|lock| {
            let reg = lock.read().expect("unable to obtain read lock");

            reg.get(&self.class_name)
                .and_then(|class| {
                    class
                        .methods()
                        .iter()
                        .find(|method| method.method_name == method_name)
                        .map(|method| method.to_dict())
                })
                .unwrap_or_default()
        })
    }

    fn get_documentation(&self) -> Array<Dictionary> {
        let (methods, props, description): (Array<Dictionary>, Array<Dictionary>, &'static str) =
            SCRIPT_REGISTRY
                .with(|lock| {
                    let reg = lock.read().expect("unable to obtain read lock");

                    reg.get(&self.class_name).map(|class| {
                        let methods = class
                            .methods_documented()
                            .iter()
                            .map(|method| method.to_method_doc())
                            .collect();

                        let props = class
                            .properties_documented()
                            .iter()
                            .map(|prop| prop.to_property_doc())
                            .collect();

                        let description = class.description();

                        (methods, props, description)
                    })
                })
                .unwrap_or_default();

        let class_doc = Dictionary::new().apply(|dict| {
            dict.set(GString::from("name"), self.class_name());
            dict.set(GString::from("inherits"), self.get_instance_base_type());
            dict.set(GString::from("brief_description"), GString::new());
            dict.set(GString::from("description"), description);
            dict.set(GString::from("tutorials"), VariantArray::new());
            dict.set(GString::from("constructors"), VariantArray::new());
            dict.set(GString::from("methods"), methods);
            dict.set(GString::from("operators"), VariantArray::new());
            dict.set(GString::from("signals"), VariantArray::new());
            dict.set(GString::from("constants"), VariantArray::new());
            dict.set(GString::from("enums"), VariantArray::new());
            dict.set(GString::from("properties"), props);
            dict.set(GString::from("theme_properties"), VariantArray::new());
            dict.set(GString::from("annotations"), VariantArray::new());
            dict.set(GString::from("is_deprecated"), false);
            dict.set(GString::from("is_experimental"), false);
            dict.set(GString::from("is_script_doc"), true);
            dict.set(GString::from("script_path"), self.base.get_path());
        });

        Array::from(&[class_doc])
    }
}
