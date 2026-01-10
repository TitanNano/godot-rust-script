/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::{cell::RefCell, collections::HashSet, ffi::c_void};

use godot::classes::{
    ClassDb, Engine, IScriptExtension, Object, Script, ScriptExtension, ScriptLanguage,
    notify::ObjectNotification, object::ConnectFlags,
};
use godot::global::{PropertyUsageFlags, godot_error, godot_print, godot_warn};
use godot::meta::{MethodInfo, PropertyInfo, ToGodot};
use godot::obj::script::create_script_instance;
use godot::obj::{EngineBitfield, InstanceId, Singleton as _, WithBaseField};
use godot::prelude::{
    Array, Base, Callable, GString, Gd, GodotClass, StringName, VarArray, VarDictionary, Variant,
    godot_api,
};

use crate::apply::Apply;
use crate::private_export::RustScriptPropDesc;

use super::rust_script_instance::GodotScriptObject;
use super::{
    SCRIPT_REGISTRY,
    downgrade_self::DowngradeSelf,
    metadata::{Documented, ToDictionary, ToMethodDoc, ToPropertyDoc},
    rust_script_instance::{RustScriptInstance, RustScriptPlaceholder},
    rust_script_language::RustScriptLanguage,
};

const NOTIFICATION_EXTENSION_RELOADED: i32 = 2;

#[derive(GodotClass)]
#[class(base = ScriptExtension, tool)]
pub(crate) struct RustScript {
    #[var(get = get_class_name, set = set_class_name, usage_flags = [STORAGE])]
    class_name: GString,

    /// Dummy property that stores the owner ids when the extension gets reloaded by the engine.
    #[var( get = owner_ids, set = set_owner_ids, usage_flags = [STORAGE])]
    #[allow(dead_code)]
    owner_ids: Array<i64>,

    owners: RefCell<HashSet<InstanceId>>,
    base: Base<ScriptExtension>,
}

#[godot_api]
impl RustScript {
    pub fn new(class_name: String) -> Gd<Self> {
        let mut inst: Gd<Self> = ClassDb::singleton()
            .instantiate(&<Self as GodotClass>::class_id().to_string_name())
            .to();

        inst.bind_mut().class_name = GString::from(&class_name);

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

        owners.iter().map(|id| id.to_i64()).collect()
    }

    #[func]
    fn set_owner_ids(&mut self, ids: Array<i64>) {
        if ids.is_empty() {
            // Ignore empty owners list from engine
            return;
        }

        if !self.owners.borrow().is_empty() {
            godot_warn!("overwriting existing owners of rust script");
        }

        *self.owners.borrow_mut() = ids.iter_shared().map(InstanceId::from_i64).collect();
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

        if let Err(err) = base
            .get_script()
            .map(|script| script.try_cast::<RustScript>())
            .transpose()
        {
            godot_warn!("expected new script to be previously assigned RustScript, but it wasn't!");
            godot_warn!("{}", err);

            return;
        }

        if !base.has_method("_init") {
            return;
        }

        base.call("_init", &[]);
    }

    fn map_property_info_list<R>(&self, f: impl Fn(&RustScriptPropDesc) -> R) -> Vec<R> {
        let reg = SCRIPT_REGISTRY.read().expect("unable to obtain read lock");

        reg.get(&self.str_class_name())
            .map(|class| class.properties().iter().map(f).collect())
            .unwrap_or_default()
    }
}

#[godot_api]
impl IScriptExtension for RustScript {
    fn init(base: Base<Self::Base>) -> Self {
        Self {
            class_name: GString::new(),
            base,
            owners: Default::default(),
            owner_ids: Default::default(),
        }
    }

    fn get_global_name(&self) -> StringName {
        (&self.get_class_name()).into()
    }

    fn get_source_code(&self) -> GString {
        GString::default()
    }

    fn set_source_code(&mut self, _code: GString) {}

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
        RustScriptLanguage::with_script_metadata(&self.str_class_name(), |metadata| {
            let Some(script) = metadata else {
                godot_error!(
                    "RustScript class {} does not exist in compiled dynamic library!",
                    self.str_class_name()
                );
                return false;
            };

            script.is_tool
        })
    }

    unsafe fn instance_create_rawptr(&self, mut for_object: Gd<Object>) -> *mut c_void {
        self.owners.borrow_mut().insert(for_object.instance_id());

        let data = self.create_remote_instance(for_object.clone());
        let instance = RustScriptInstance::new(data, for_object.clone(), self.to_gd());

        let callbale_args = VarArray::from(&[for_object.to_variant()]);

        for_object.connect_flags(
            "script_changed",
            &Callable::from_object_method(&self.to_gd(), "init_script_instance")
                .bindv(&callbale_args),
            ConnectFlags::ONE_SHOT,
        );

        // SAFETY: we are not freeing the owner object before we pass the script instance pointer back to the engine.
        unsafe { create_script_instance(instance, for_object) }
    }

    unsafe fn placeholder_instance_create_rawptr(&self, for_object: Gd<Object>) -> *mut c_void {
        self.owners.borrow_mut().insert(for_object.instance_id());

        let placeholder = RustScriptPlaceholder::new(self.to_gd());

        // SAFETY: we are not freeing the owner object before we pass the script instance pointer back to the engine.
        unsafe { create_script_instance(placeholder, for_object) }
    }

    fn is_valid(&self) -> bool {
        true
    }

    fn has_property_default_value(&self, _property: StringName) -> bool {
        // Default values are currently not exposed
        false
    }

    fn get_property_default_value(&self, #[expect(unused)] property: StringName) -> Variant {
        // Default values are currently not exposed
        Variant::nil()
    }

    fn get_script_signal_list(&self) -> Array<VarDictionary> {
        RustScriptLanguage::with_script_metadata(&self.str_class_name(), |script_data| {
            let Some(script) = script_data else {
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
        })
    }

    fn has_script_signal(&self, name: StringName) -> bool {
        RustScriptLanguage::with_script_metadata(&self.str_class_name(), |script_data| {
            let Some(script) = script_data else {
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
        })
    }

    fn update_exports(&mut self) {}

    fn get_script_method_list(&self) -> Array<VarDictionary> {
        let reg = SCRIPT_REGISTRY.read().expect("unable to obtain read lock");

        reg.get(&self.str_class_name())
            .map(|class| {
                class
                    .methods()
                    .iter()
                    .map(|method| MethodInfo::from(method.clone()).to_dict())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn get_script_property_list(&self) -> Array<VarDictionary> {
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
                .any(|method| method.name == method_name.to_string())
        })
    }

    fn get_constants(&self) -> VarDictionary {
        VarDictionary::new()
    }
    fn get_method_info(&self, method_name: StringName) -> VarDictionary {
        let reg = SCRIPT_REGISTRY.read().expect("unable to obtain read lock");

        reg.get(&self.str_class_name())
            .and_then(|class| {
                class
                    .methods()
                    .iter()
                    .find(|method| method.name == method_name.to_string())
                    .map(|method| MethodInfo::from(method.clone()).to_dict())
            })
            .unwrap_or_default()
    }

    fn get_documentation(&self) -> Array<VarDictionary> {
        let (methods, props, signals, description): (
            Array<VarDictionary>,
            Array<VarDictionary>,
            Array<VarDictionary>,
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

        let class_doc = VarDictionary::new().apply(|dict| {
            dict.set(GString::from("name"), self.get_class_name());
            dict.set(GString::from("inherits"), self.get_instance_base_type());
            dict.set(GString::from("brief_description"), GString::new());
            dict.set(GString::from("description"), description);
            dict.set(GString::from("tutorials"), VarArray::new());
            dict.set(GString::from("constructors"), VarArray::new());
            dict.set(GString::from("methods"), methods);
            dict.set(GString::from("operators"), VarArray::new());
            dict.set(GString::from("signals"), signals);
            dict.set(GString::from("constants"), VarArray::new());
            dict.set(GString::from("enums"), VarArray::new());
            dict.set(GString::from("properties"), props);
            dict.set(GString::from("theme_properties"), VarArray::new());
            dict.set(GString::from("annotations"), VarArray::new());
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

    // Godot script reload hook
    fn reload(
        &mut self,
        // Before 4.4 the engine does not correctly pass the keep_state flag
        #[cfg_attr(before_api = "4.4", expect(unused_variables))] keep_state: bool,
    ) -> godot::global::Error {
        #[cfg(before_api = "4.4")]
        let keep_state = true;

        let owners = self.owners.borrow().clone();
        let exported_properties_list = if keep_state {
            self.map_property_info_list(|prop| {
                (prop.usage.ord() & PropertyUsageFlags::EDITOR.ord()
                    != PropertyUsageFlags::NONE.ord())
                .then_some(prop.name)
            })
        } else {
            Vec::with_capacity(0)
        };

        owners.iter().for_each(|owner_id| {
            let mut object: Gd<Object> = match Gd::try_from_instance_id(*owner_id) {
                Ok(owner) => owner,
                Err(err) => {
                    godot_warn!("Failed to get script owner: {:?}", err);
                    return;
                }
            };

            let property_backup: Vec<_> = if keep_state {
                exported_properties_list
                    .iter()
                    .flatten()
                    .map(|key| {
                        let value = object.get(*key);

                        (*key, value)
                    })
                    .collect()
            } else {
                Vec::with_capacity(0)
            };

            // Clear script to destroy script instance.
            object.set_script(Option::<&Gd<Script>>::None);

            self.downgrade_gd(|self_gd| {
                // Reassign script to create new instance.
                object.set_script(Some(&self_gd));

                if keep_state {
                    property_backup.into_iter().for_each(|(key, value)| {
                        object.set(key, &value);
                    });
                }
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

            self.reload(true);
        }
    }

    fn has_source_code(&self) -> bool {
        false
    }

    fn inherits_script(&self, #[expect(unused)] script: Gd<Script>) -> bool {
        false
    }

    fn instance_has(&self, object: Gd<Object>) -> bool {
        #[expect(unused)]
        let Some(script): Option<Gd<RustScript>> = object
            .get_script()
            .map(|script| script.try_cast::<RustScript>())
            .transpose()
            .ok()
            .flatten()
        else {
            return false;
        };

        true
    }

    #[cfg(since_api = "4.2")]
    fn has_static_method(&self, #[expect(unused)] method: StringName) -> bool {
        // Static methods are currently not supported
        false
    }

    fn get_member_line(&self, #[expect(unused)] member: StringName) -> i32 {
        0
    }

    fn get_members(&self) -> Array<StringName> {
        let reg = SCRIPT_REGISTRY.read().expect("unable to obtain read lock");

        reg.get(&self.str_class_name())
            .map(|class| {
                class
                    .properties()
                    .iter()
                    .map(|prop| StringName::from(prop.name))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn is_placeholder_fallback_enabled(&self) -> bool {
        false
    }

    fn get_rpc_config(&self) -> Variant {
        godot_warn!("godot-rust-script: rpc config is unsupported!");
        Variant::nil()
    }

    #[cfg(since_api = "4.4")]
    fn get_doc_class_name(&self) -> StringName {
        StringName::from(&self.class_name)
    }
}
