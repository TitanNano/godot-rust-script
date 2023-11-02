use std::{collections::HashMap, rc::Rc};

#[cfg(all(feature = "hot-reload", debug_assertions))]
use std::{
    cell::RefCell,
    ops::{Deref, DerefMut},
};

use abi_stable::std_types::{RBox, RString};
use cfg_if::cfg_if;
use godot::{
    builtin::ScriptInstance,
    engine::Script,
    prelude::{
        godot_print,
        meta::{MethodInfo, PropertyInfo},
        Gd, GodotString, Object, StringName, Variant, VariantType,
    },
};

use crate::{
    apply::Apply,
    script_registry::{RemoteGodotScript_TO, RemoteValueRef},
};

#[cfg(all(feature = "hot-reload", debug_assertions))]
use crate::runtime::hot_reloader::HotReloadEntry;

use super::{rust_script::RustScript, rust_script_language::RustScriptLanguage, SCRIPT_REGISTRY};

#[cfg(all(feature = "hot-reload", debug_assertions))]
use super::HOT_RELOAD_BRIDGE;

fn script_method_list(script: &Gd<RustScript>) -> Rc<Vec<MethodInfo>> {
    let rs = script.bind();
    let class_name = rs.str_class_name();

    SCRIPT_REGISTRY.with(|lock| {
        lock.read()
            .expect("script registry is inaccessible")
            .get(class_name)
            .map(|meta| meta.methods())
            .unwrap_or_default()
    })
}

fn script_class_name(script: &Gd<RustScript>) -> GodotString {
    script.bind().class_name()
}

fn script_property_list(script: &Gd<RustScript>) -> Rc<Vec<PropertyInfo>> {
    let rs = script.bind();
    let class_name = rs.str_class_name();

    SCRIPT_REGISTRY.with(|lock| {
        lock.read()
            .expect("script registry is inaccessible")
            .get(class_name)
            .map(|meta| meta.properties())
            .unwrap_or_default()
    })
}

pub(super) struct RustScriptInstance {
    #[cfg(not(all(feature = "hot-reload", debug_assertions)))]
    data: RemoteGodotScript_TO<'static, RBox<()>>,

    #[cfg(all(feature = "hot-reload", debug_assertions))]
    data: RustScriptInstanceId,

    gd_object: Gd<Object>,
    script: Gd<RustScript>,
}

#[cfg(all(feature = "hot-reload", debug_assertions))]
#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub(super) struct RustScriptInstanceId(usize);

#[cfg(all(feature = "hot-reload", debug_assertions))]
impl RustScriptInstanceId {
    pub fn new() -> Self {
        Self(rand::random())
    }
}

impl RustScriptInstance {
    pub fn new(
        data: RemoteGodotScript_TO<'static, RBox<()>>,
        gd_object: Gd<Object>,
        script: Gd<RustScript>,
    ) -> Self {
        cfg_if! {
            if #[cfg(all(feature = "hot-reload", debug_assertions))] {
                let id = RustScriptInstanceId::new();

                HOT_RELOAD_BRIDGE.with(|map| {
                    let entry = HotReloadEntry::new(
                        data,
                        script.clone(),
                        gd_object.clone(),
                    );

                    map.borrow_mut().insert(id, RefCell::new(entry))
                });

                Self {
                    data: id,
                    gd_object,
                    script,
                }
            } else {
                Self {
                    data,
                    gd_object,
                    script,
                }
            }
        }
    }
}

cfg_if! {
    if #[cfg(all(feature = "hot-reload", debug_assertions))] {
        impl RustScriptInstance {
            fn with_data<T>(&self, cb: impl FnOnce(&RemoteGodotScript_TO<'static, RBox<()>>) -> T) -> T {
                HOT_RELOAD_BRIDGE.with(|map| {
                    let map_ref = map.borrow();

                    let inst = map_ref
                        .get(&self.data)
                        .expect("not having the remote script instance is fatal");

                    let result = cb(&inst.borrow().deref().instance);

                    result
                })
            }

            fn with_data_mut<T>(
                &self,
                cb: impl FnOnce(&mut RemoteGodotScript_TO<'static, RBox<()>>) -> T,
            ) -> T {
                HOT_RELOAD_BRIDGE.with(|map| {
                    let map_ref = map.borrow();

                    let inst = map_ref
                        .get(&self.data)
                        .expect("not having the remote script instance is fatal");

                    let result = cb(&mut inst.borrow_mut().deref_mut().instance);

                    result
                })
            }
        }
    } else {
        impl RustScriptInstance {
            fn with_data<T>(&self, cb: impl FnOnce(&RemoteGodotScript_TO<'static, RBox<()>>) -> T) -> T {
                cb(&self.data)
            }

            fn with_data_mut<T>(
                &mut self,
                cb: impl FnOnce(&mut RemoteGodotScript_TO<'static, RBox<()>>) -> T,
            ) -> T {
                cb(&mut self.data)
            }
        }
    }
}

impl ScriptInstance for RustScriptInstance {
    fn class_name(&self) -> GodotString {
        script_class_name(&self.script)
    }

    fn set(&mut self, name: StringName, value: &Variant) -> bool {
        let name = RString::with_capacity(name.len()).apply(|s| s.push_str(&name.to_string()));
        let value = RemoteValueRef::new(value);

        self.with_data_mut(|data| data.set(name, value))
    }

    fn get(&self, name: StringName) -> Option<Variant> {
        let name =
            RString::with_capacity(name.to_string().len()).apply(|s| s.push_str(&name.to_string()));

        self.with_data(move |data| data.get(name))
            .map(Into::into)
            .into()
    }

    fn property_list(&self) -> Rc<Vec<PropertyInfo>> {
        script_property_list(&self.script)
    }

    fn method_list(&self) -> Rc<Vec<MethodInfo>> {
        script_method_list(&self.script)
    }

    fn call(
        &mut self,
        method: StringName,
        args: &[&Variant],
    ) -> Result<Variant, godot::sys::GDExtensionCallErrorType> {
        godot_print!("calling {}::{}", self.class_name(), method);
        let method =
            RString::with_capacity(method.len()).apply(|s| s.push_str(&method.to_string()));
        let rargs = args.iter().map(|v| RemoteValueRef::new(v)).collect();

        self.with_data_mut(move |data| data.call(method, rargs))
            .map(Into::into)
            .into_result()
    }

    fn get_script(&self) -> Gd<Script> {
        self.script.clone().upcast()
    }

    fn is_placeholder(&self) -> bool {
        false
    }

    fn has_method(&self, method: StringName) -> bool {
        let rs = self.script.bind();
        let class_name = rs.str_class_name();

        SCRIPT_REGISTRY.with(|lock| {
            lock.read()
                .expect("script registry is not accessible")
                .get(class_name)
                .and_then(|meta| {
                    meta.methods()
                        .iter()
                        .find(|m| m.method_name == method)
                        .map(|_| ())
                })
                .is_some()
        })
    }

    fn get_property_type(&self, name: StringName) -> godot::sys::VariantType {
        self.property_list()
            .iter()
            .find(|prop| prop.property_name == name)
            .map(|prop| prop.variant_type)
            .unwrap_or(godot::sys::VariantType::Nil)
    }

    fn to_string(&self) -> GodotString {
        self.with_data(|data| data.to_string()).into_string().into()
    }

    fn owner(&self) -> Gd<godot::prelude::Object> {
        self.gd_object.clone().upcast()
    }

    fn property_state(&self) -> Vec<(StringName, Variant)> {
        self.property_list()
            .as_slice()
            .iter()
            .map(|prop| &prop.property_name)
            .filter_map(|name| {
                self.get(name.to_owned())
                    .map(|value| (name.to_owned(), value))
            })
            .collect()
    }

    fn language(&self) -> Gd<godot::engine::ScriptLanguage> {
        Gd::<RustScriptLanguage>::new_default().upcast()
    }

    fn refcount_decremented(&self) -> bool {
        true
    }

    fn refcount_incremented(&self) {}
}

pub(super) struct RustScriptPlaceholder {
    script: Gd<RustScript>,
    properties: HashMap<StringName, Variant>,
}

impl RustScriptPlaceholder {
    pub fn new(script: Gd<RustScript>) -> Self {
        Self {
            script,
            properties: Default::default(),
        }
    }
}

impl ScriptInstance for RustScriptPlaceholder {
    fn class_name(&self) -> GodotString {
        script_class_name(&self.script)
    }

    fn set(&mut self, name: StringName, value: &Variant) -> bool {
        let exists = self
            .property_list()
            .iter()
            .any(|prop| prop.property_name == name);

        if !exists {
            return false;
        }

        self.properties.insert(name, value.to_owned());
        true
    }

    fn get(&self, name: StringName) -> Option<Variant> {
        self.properties.get(&name).cloned()
    }

    fn property_list(&self) -> Rc<Vec<PropertyInfo>> {
        script_property_list(&self.script)
    }

    fn method_list(&self) -> Rc<Vec<MethodInfo>> {
        script_method_list(&self.script)
    }

    fn call(
        &mut self,
        _method: StringName,
        _args: &[&Variant],
    ) -> Result<Variant, godot::sys::GDExtensionCallErrorType> {
        Err(godot::sys::GDEXTENSION_CALL_OK)
    }

    fn get_script(&self) -> Gd<Script> {
        self.script.clone().upcast()
    }

    fn has_method(&self, method_name: StringName) -> bool {
        self.method_list()
            .iter()
            .any(|method| method.method_name == method_name)
    }

    fn is_placeholder(&self) -> bool {
        true
    }

    fn get_property_type(&self, name: StringName) -> godot::sys::VariantType {
        self.property_list()
            .iter()
            .find(|prop| prop.property_name == name)
            .map(|prop| prop.variant_type)
            .unwrap_or(VariantType::Nil)
    }

    fn to_string(&self) -> GodotString {
        GodotString::new()
    }

    fn owner(&self) -> Gd<godot::prelude::Object> {
        self.script.clone().upcast()
    }

    fn property_state(&self) -> Vec<(StringName, Variant)> {
        self.properties
            .iter()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect()
    }

    fn language(&self) -> Gd<godot::engine::ScriptLanguage> {
        Gd::<RustScriptLanguage>::new_default().upcast()
    }

    fn refcount_decremented(&self) -> bool {
        true
    }

    fn refcount_incremented(&self) {}
}
