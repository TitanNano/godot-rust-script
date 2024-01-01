use std::{cell::RefCell, collections::HashSet, ffi::c_void, ops::Deref};

use abi_stable::std_types::RBox;
use godot::{
    builtin::meta::{MethodInfo, PropertyInfo, ToGodot},
    engine::{
        create_script_instance, notify::ObjectNotification, ClassDb, Engine, IScriptExtension,
        Script, ScriptExtension, ScriptLanguage, WeakRef,
    },
    log::{godot_print, godot_warn},
    obj::{InstanceId, UserClass},
    prelude::{
        godot_api, Array, Base, Dictionary, GString, Gd, GodotClass, Object, StringName, Variant,
        VariantArray,
    },
};
use RemoteGodotScript_trait::RemoteGodotScript_TO;

use crate::{apply::Apply, script_registry::RemoteGodotScript_trait};

use super::{
    metadata::{Documented, ToDictionary, ToMethodDoc, ToPropertyDoc},
    rust_script_instance::{RustScriptInstance, RustScriptPlaceholder},
    rust_script_language::RustScriptLanguage,
    SCRIPT_REGISTRY,
};

const NOTIFICATION_EXTENSION_RELOADED: i32 = 2;

#[derive(GodotClass)]
#[class(base = ScriptExtension, tool)]
pub(super) struct RustScript {
    #[var(get = get_class_name, set = set_class_name, usage_flags = [PROPERTY_USAGE_STORAGE])]
    class_name: GString,

    #[var(usage_flags = [PROPERTY_USAGE_STORAGE])]
    source_code: GString,

    #[var( get = owner_ids, set = set_owner_ids, usage_flags = [PROPERTY_USAGE_STORAGE])]
    #[allow(dead_code)]
    owner_ids: Array<i64>,

    owners: RefCell<Vec<Gd<WeakRef>>>,
    base: Base<ScriptExtension>,
}

#[godot_api]
impl RustScript {
    pub fn new(class_name: String) -> Gd<Self> {
        let mut inst: Gd<Self> = ClassDb::singleton()
            .instantiate(<Self as GodotClass>::class_name().to_string_name())
            .to();

        inst.bind_mut().class_name = GString::from(class_name);

        inst
    }

    #[func]
    pub fn get_class_name(&self) -> GString {
        self.class_name.clone()
    }

    #[func]
    fn set_class_name(&mut self, value: GString) {
        self.class_name = value;
    }

    pub fn str_class_name(&self) -> String {
        self.class_name.to_string()
    }

    pub fn create_remote_instance(
        &self,
        base: Gd<Object>,
    ) -> RemoteGodotScript_TO<'static, RBox<()>> {
        let reg = SCRIPT_REGISTRY.read().expect("failed to obtain read lock");

        let meta_data = reg
            .get(&self.str_class_name())
            .expect("we musst know the class name at this point");

        meta_data.create_data(base)
    }

    #[func]
    fn owner_ids(&self) -> Array<i64> {
        let owners = self.owners.borrow();

        godot_print!("extracting script owners: {:?}", *owners);

        let set: HashSet<_> = owners
            .iter()
            .filter_map(|item| {
                let strong_ref = item.get_ref().to::<Option<Gd<Object>>>();

                godot_print!("strong ref: {:?}", strong_ref);

                strong_ref.map(|obj| obj.instance_id().to_i64())
            })
            .collect();

        set.into_iter().collect()
    }

    #[func]
    fn set_owner_ids(&mut self, ids: Array<i64>) {
        if ids.is_empty() {
            // ignore empty owners list from engine
            return;
        }

        if !self.owners.borrow().is_empty() {
            godot_warn!("over writing existing owners of rust script");
        }

        godot_print!("assigning owners list to script: {:?}", ids);

        *self.owners.borrow_mut() = ids
            .iter_shared()
            .map(InstanceId::from_i64)
            .filter_map(|id| {
                godot_print!(
                    "reloading script instance of {}, {}",
                    id,
                    self.get_class_name()
                );

                let result: Option<Gd<Object>> = Gd::try_from_instance_id(id).ok();

                godot_print!("object for {} is {:?}", id, result);
                result
            })
            .map(|gd_ref| godot::engine::utilities::weakref(gd_ref.to_variant()).to())
            .collect();
    }
}

#[godot_api]
impl IScriptExtension for RustScript {
    fn init(base: Base<Self::Base>) -> Self {
        godot_print!("creating RustScript struct for {:?}", base);

        Self {
            class_name: GString::new(),
            source_code: GString::new(),
            base,
            owners: Default::default(),
            owner_ids: Default::default(),
        }
    }

    fn get_global_name(&self) -> StringName {
        self.get_class_name().into()
    }

    fn get_source_code(&self) -> GString {
        self.source_code.clone()
    }
    fn set_source_code(&mut self, code: GString) {
        self.source_code = code;
    }

    fn get_language(&self) -> Option<Gd<ScriptLanguage>> {
        Some(RustScriptLanguage::alloc_gd().upcast())
    }

    fn can_instantiate(&self) -> bool {
        self.is_tool() || !Engine::singleton().is_editor_hint()
    }

    fn get_instance_base_type(&self) -> StringName {
        let reg = SCRIPT_REGISTRY.read().expect("unable to obtain read lock");

        reg.get(&self.str_class_name())
            .map(|class| class.base_type_name())
            .unwrap_or_else(|| StringName::from("RefCounted"))
    }

    fn get_base_script(&self) -> Option<Gd<Script>> {
        None
    }

    fn is_tool(&self) -> bool {
        false
    }

    unsafe fn instance_create(&self, for_object: Gd<Object>) -> *mut c_void {
        self.owners
            .borrow_mut()
            .push(godot::engine::utilities::weakref(for_object.to_variant()).to());

        let data = self.create_remote_instance(for_object.clone());
        let instance = RustScriptInstance::new(data, for_object, self.base.deref().clone().cast());

        create_script_instance(instance)
    }

    unsafe fn placeholder_instance_create(&self, for_object: Gd<Object>) -> *mut c_void {
        godot_print!(
            "creating placeholder instance for script {}",
            self.get_global_name()
        );

        self.owners
            .borrow_mut()
            .push(godot::engine::utilities::weakref(for_object.to_variant()).to());

        let placeholder = RustScriptPlaceholder::new(self.base.deref().clone().cast());

        create_script_instance(placeholder)
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
        let reg = SCRIPT_REGISTRY.read().expect("unable to obtain read lock");

        reg.get(&self.str_class_name())
            .map(|class| {
                class
                    .methods()
                    .iter()
                    .map(|method| MethodInfo::from(method.to_owned()).to_dict())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn get_script_property_list(&self) -> Array<Dictionary> {
        let reg = SCRIPT_REGISTRY.read().expect("unable to obtain read lock");

        reg.get(&self.str_class_name())
            .map(|class| {
                class
                    .properties()
                    .iter()
                    .map(|prop| PropertyInfo::from(prop.to_owned()).to_dict())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn has_method(&self, method_name: StringName) -> bool {
        let reg = SCRIPT_REGISTRY.read().expect("unable to obtain read lock");

        reg.get(&self.str_class_name()).is_some_and(|class| {
            class
                .methods()
                .iter()
                .any(|method| method.method_name == method_name.to_string())
        })
    }

    fn get_constants(&self) -> Dictionary {
        Dictionary::new()
    }

    fn get_method_info(&self, method_name: StringName) -> Dictionary {
        let reg = SCRIPT_REGISTRY.read().expect("unable to obtain read lock");

        reg.get(&self.str_class_name())
            .and_then(|class| {
                class
                    .methods()
                    .iter()
                    .find(|method| method.method_name == method_name.to_string())
                    .map(|method| MethodInfo::from(method.to_owned()).to_dict())
            })
            .unwrap_or_default()
    }

    fn get_documentation(&self) -> Array<Dictionary> {
        let (methods, props, description): (Array<Dictionary>, Array<Dictionary>, &'static str) = {
            let reg = SCRIPT_REGISTRY.read().expect("unable to obtain read lock");

            reg.get(&self.str_class_name())
                .map(|class| {
                    let methods = class
                        .methods()
                        .iter()
                        .map(|method| {
                            Documented::<MethodInfo>::from(method.to_owned()).to_method_doc()
                        })
                        .collect();

                    let props = class
                        .properties()
                        .iter()
                        .map(|prop| {
                            Documented::<PropertyInfo>::from(prop.to_owned()).to_property_doc()
                        })
                        .collect();

                    let description = class.description();

                    (methods, props, description)
                })
                .unwrap_or_default()
        };

        let class_doc = Dictionary::new().apply(|dict| {
            dict.set(GString::from("name"), self.get_class_name());
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

    fn editor_can_reload_from_file(&mut self) -> bool {
        true
    }

    // godot script reload hook
    fn reload(&mut self, _keep_state: bool) -> godot::engine::global::Error {
        godot_print!(
            "new script properties: {:?}",
            self.get_script_property_list()
        );

        self.owners.borrow().iter().for_each(|owner| {
            let mut object: Gd<Object> = match owner.get_ref().try_to() {
                Ok(owner) => owner,
                Err(err) => {
                    godot_warn!("Failed to get script owner: {:?}", err);
                    return;
                }
            };

            let script = object.get_script();

            // clear script to destroy script instance.
            object.set_script(Variant::nil());

            // re-assign script to create new instance.
            // call is defered because this will call back into can_instantiate.
            object.call_deferred(StringName::from("set_script"), &[script]);
        });

        godot::engine::global::Error::OK
    }

    fn on_notification(&mut self, what: ObjectNotification) {
        if let ObjectNotification::Unknown(NOTIFICATION_EXTENSION_RELOADED) = what {
            godot_print!(
                "RustScript({}): received extension reloaded notification!",
                self.str_class_name()
            );

            self.reload(false);
        }
    }
}
