/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::{cell::RefCell, collections::HashSet, ffi::c_void};

use godot::classes::{
    notify::ObjectNotification, object::ConnectFlags, ClassDb, Engine, IScriptExtension, Object,
    Script, ScriptExtension, ScriptLanguage, WeakRef,
};
use godot::global::{godot_error, godot_print, godot_warn};
use godot::meta::{MethodInfo, PropertyInfo, ToGodot};
use godot::obj::script::create_script_instance;
use godot::obj::{EngineEnum, InstanceId, WithBaseField};
use godot::prelude::{
    godot_api, Array, Base, Callable, Dictionary, GString, Gd, GodotClass, StringName, Variant,
    VariantArray,
};

use crate::apply::Apply;

use super::rust_script_instance::GodotScriptObject;
use super::{
    downgrade_self::DowngradeSelf,
    metadata::{Documented, ToDictionary, ToMethodDoc, ToPropertyDoc},
    rust_script_instance::{RustScriptInstance, RustScriptPlaceholder},
    rust_script_language::RustScriptLanguage,
    SCRIPT_REGISTRY,
};

const NOTIFICATION_EXTENSION_RELOADED: i32 = 2;

#[derive(GodotClass)]
#[class(base = ScriptExtension, tool)]
pub(crate) struct RustScript {
    #[var(get = get_class_name, set = set_class_name, usage_flags = [STORAGE])]
    class_name: GString,

    #[var(usage_flags = [STORAGE])]
    source_code: GString,

    #[var( get = owner_ids, set = set_owner_ids, usage_flags = [STORAGE])]
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

    pub fn create_remote_instance(&self, base: Gd<Object>) -> Box<dyn GodotScriptObject> {
        let reg = SCRIPT_REGISTRY.read().expect("failed to obtain read lock");

        let meta_data = reg
            .get(&self.str_class_name())
            .expect("we musst know the class name at this point");

        meta_data.create_data(base)
    }

    #[func]
    fn owner_ids(&self) -> Array<i64> {
        let owners = self.owners.borrow();

        let set: HashSet<_> = owners
            .iter()
            .filter_map(|item| item.get_ref().to::<Option<Gd<Object>>>())
            .map(|obj| obj.instance_id().to_i64())
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

        *self.owners.borrow_mut() = ids
            .iter_shared()
            .map(InstanceId::from_i64)
            .filter_map(|id| {
                let result: Option<Gd<Object>> = Gd::try_from_instance_id(id).ok();
                result
            })
            .map(|gd_ref| godot::global::weakref(gd_ref.to_variant()).to())
            .collect();
    }

    #[func]
    fn init_script_instance(base: Variant) {
        let mut base: Gd<Object> = match base.try_to() {
            Ok(base) => base,
            Err(err) => panic!(
                "init_rust_script_instance was called without base object bind!\n{}",
                err
            ),
        };

        if let Err(err) = base.get_script().try_to::<Gd<RustScript>>() {
            godot_warn!("expected new script to be previously assigned RustScript, but it wasn't!");
            godot_warn!("{}", err);

            return;
        }

        if !base.has_method("_init".into()) {
            return;
        }

        base.call(StringName::from("_init"), &[]);
    }
}

#[godot_api]
impl IScriptExtension for RustScript {
    fn init(base: Base<Self::Base>) -> Self {
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
        RustScriptLanguage::singleton().map(Gd::upcast)
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

    unsafe fn instance_create(&self, mut for_object: Gd<Object>) -> *mut c_void {
        self.owners
            .borrow_mut()
            .push(godot::global::weakref(for_object.to_variant()).to());

        let data = self.create_remote_instance(for_object.clone());
        let instance = RustScriptInstance::new(data, for_object.clone(), self.to_gd());

        let callbale_args = VariantArray::from(&[for_object.to_variant()]);

        for_object
            .connect_ex(
                StringName::from("script_changed"),
                Callable::from_object_method(&self.to_gd(), "init_script_instance")
                    .bindv(callbale_args),
            )
            .flags(ConnectFlags::ONE_SHOT.ord() as u32)
            .done();

        create_script_instance(instance, for_object)
    }

    unsafe fn placeholder_instance_create(&self, for_object: Gd<Object>) -> *mut c_void {
        self.owners
            .borrow_mut()
            .push(godot::global::weakref(for_object.to_variant()).to());

        let placeholder = RustScriptPlaceholder::new(self.to_gd());

        create_script_instance(placeholder, for_object)
    }

    fn is_valid(&self) -> bool {
        true
    }

    fn has_property_default_value(&self, _property: StringName) -> bool {
        false
    }

    fn get_script_signal_list(&self) -> Array<Dictionary> {
        let Some(script) = RustScriptLanguage::script_meta_data(&self.str_class_name()) else {
            godot_error!(
                "RustScript class {} does not exist in compiled dynamic library!",
                self.str_class_name()
            );
            return Array::new();
        };

        script
            .signals()
            .iter()
            .map(|signal| MethodInfo::from(signal).to_dict())
            .collect()
    }

    fn has_script_signal(&self, name: StringName) -> bool {
        let Some(script) = RustScriptLanguage::script_meta_data(&self.str_class_name()) else {
            godot_error!(
                "RustScript class {} does not exist in compiled dynamic library!",
                self.str_class_name()
            );
            return false;
        };

        script
            .signals()
            .iter()
            .any(|signal| signal.name == name.to_string())
    }

    fn update_exports(&mut self) {}

    fn get_script_method_list(&self) -> Array<Dictionary> {
        let reg = SCRIPT_REGISTRY.read().expect("unable to obtain read lock");

        reg.get(&self.str_class_name())
            .map(|class| {
                class
                    .methods()
                    .iter()
                    .map(|method| MethodInfo::from(method).to_dict())
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
                    .map(|prop| PropertyInfo::from(prop).to_dict())
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
                    .map(|method| MethodInfo::from(method).to_dict())
            })
            .unwrap_or_default()
    }

    fn get_documentation(&self) -> Array<Dictionary> {
        let (methods, props, signals, description): (
            Array<Dictionary>,
            Array<Dictionary>,
            Array<Dictionary>,
            &'static str,
        ) = {
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

                    let signals = class
                        .signals()
                        .iter()
                        .map(|signal| {
                            Documented::<MethodInfo>::from(signal.to_owned()).to_method_doc()
                        })
                        .collect();

                    let description = class.description();

                    (methods, props, signals, description)
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
            dict.set(GString::from("signals"), signals);
            dict.set(GString::from("constants"), VariantArray::new());
            dict.set(GString::from("enums"), VariantArray::new());
            dict.set(GString::from("properties"), props);
            dict.set(GString::from("theme_properties"), VariantArray::new());
            dict.set(GString::from("annotations"), VariantArray::new());
            dict.set(GString::from("is_deprecated"), false);
            dict.set(GString::from("is_experimental"), false);
            dict.set(GString::from("is_script_doc"), true);
            dict.set(GString::from("script_path"), self.base().get_path());
        });

        Array::from(&[class_doc])
    }

    fn editor_can_reload_from_file(&mut self) -> bool {
        true
    }

    // godot script reload hook
    fn reload(&mut self, _keep_state: bool) -> godot::global::Error {
        let owners = self.owners.borrow().clone();

        owners.iter().for_each(|owner| {
            let mut object: Gd<Object> = match owner.get_ref().try_to() {
                Ok(owner) => owner,
                Err(err) => {
                    godot_warn!("Failed to get script owner: {:?}", err);
                    return;
                }
            };

            // clear script to destroy script instance.
            object.set_script(Variant::nil());

            self.downgrade_gd(|self_gd| {
                // re-assign script to create new instance.
                object.set_script(self_gd.to_variant());
            })
        });

        godot::global::Error::OK
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
