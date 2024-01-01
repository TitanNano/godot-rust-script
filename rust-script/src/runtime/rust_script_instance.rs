/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

#[cfg(all(feature = "hot-reload", debug_assertions))]
use std::cell::RefCell;
use std::{collections::HashMap, rc::Rc};

use abi_stable::std_types::{RBox, RString};
use godot::{
    engine::{Script, ScriptInstance},
    obj::UserClass,
    prelude::{
        godot_print,
        meta::{MethodInfo, PropertyInfo},
        GString, Gd, Object, StringName, Variant, VariantType,
    },
};

use crate::{
    apply::Apply,
    script_registry::{RemoteGodotScript_TO, RemoteValueRef},
};

use super::{rust_script::RustScript, rust_script_language::RustScriptLanguage, SCRIPT_REGISTRY};

fn script_method_list(script: &Gd<RustScript>) -> Rc<Vec<MethodInfo>> {
    let rs = script.bind();
    let class_name = rs.str_class_name();

    let methods = SCRIPT_REGISTRY
        .read()
        .expect("script registry is inaccessible")
        .get(&class_name)
        .map(|meta| {
            Rc::new(
                meta.methods()
                    .iter()
                    .map(|method| MethodInfo::from(method.to_owned()))
                    .collect(),
            )
        })
        .unwrap_or_default();

    methods
}

fn script_class_name(script: &Gd<RustScript>) -> GString {
    script.bind().get_class_name()
}

fn script_property_list(script: &Gd<RustScript>) -> Rc<Vec<PropertyInfo>> {
    let rs = script.bind();
    let class_name = rs.str_class_name();

    let props = SCRIPT_REGISTRY
        .read()
        .expect("script registry is inaccessible")
        .get(&class_name)
        .map(|meta| {
            Rc::new(
                meta.properties()
                    .iter()
                    .map(|prop| PropertyInfo::from(prop.to_owned()))
                    .collect(),
            )
        })
        .unwrap_or_default();

    props
}

pub(super) struct RustScriptInstance {
    data: RemoteGodotScript_TO<'static, RBox<()>>,

    script: Gd<RustScript>,
    generic_script: Gd<Script>,
    property_list: Rc<Vec<PropertyInfo>>,
    method_list: Rc<Vec<MethodInfo>>,
}

impl RustScriptInstance {
    pub fn new(
        data: RemoteGodotScript_TO<'static, RBox<()>>,
        _gd_object: Gd<Object>,
        script: Gd<RustScript>,
    ) -> Self {
        Self {
            data,
            generic_script: script.clone().upcast(),
            property_list: script_property_list(&script),
            method_list: script_method_list(&script),
            script,
        }
    }
}

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

impl ScriptInstance for RustScriptInstance {
    fn class_name(&self) -> GString {
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

    fn get_property_list(&self) -> &[PropertyInfo] {
        self.property_list.as_ref()
    }

    fn get_method_list(&self) -> &[MethodInfo] {
        self.method_list.as_ref()
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
            // GDExtensionCallErrorType is not guaranteed to be a u32
            .map_err(|err: u32| err as godot::sys::GDExtensionCallErrorType)
    }

    fn get_script(&self) -> &Gd<Script> {
        &self.generic_script
    }

    fn is_placeholder(&self) -> bool {
        false
    }

    fn has_method(&self, method_name: StringName) -> bool {
        self.method_list
            .iter()
            .any(|method| method.method_name == method_name)
    }

    fn get_property_type(&self, name: StringName) -> godot::sys::VariantType {
        self.get_property_list()
            .iter()
            .find(|prop| prop.property_name == name)
            .map(|prop| prop.variant_type)
            .unwrap_or(godot::sys::VariantType::Nil)
    }

    fn to_string(&self) -> GString {
        self.with_data(|data| data.to_string()).into_string().into()
    }

    fn get_property_state(&self) -> Vec<(StringName, Variant)> {
        self.get_property_list()
            .iter()
            .map(|prop| &prop.property_name)
            .filter_map(|name| {
                self.get(name.to_owned())
                    .map(|value| (name.to_owned(), value))
            })
            .collect()
    }

    fn get_language(&self) -> Gd<godot::engine::ScriptLanguage> {
        RustScriptLanguage::alloc_gd().upcast()
    }

    fn on_refcount_decremented(&self) -> bool {
        true
    }

    fn on_refcount_incremented(&self) {}

    fn property_get_fallback(&self, _name: StringName) -> Option<Variant> {
        None
    }

    fn property_set_fallback(&mut self, _name: StringName, _value: &Variant) -> bool {
        false
    }
}

pub(super) struct RustScriptPlaceholder {
    script: Gd<RustScript>,
    generic_script: Gd<Script>,
    properties: HashMap<StringName, Variant>,
    property_list: Rc<Vec<PropertyInfo>>,
    method_list: Rc<Vec<MethodInfo>>,
}

impl RustScriptPlaceholder {
    pub fn new(script: Gd<RustScript>) -> Self {
        Self {
            generic_script: script.clone().upcast(),
            properties: Default::default(),
            property_list: script_property_list(&script),
            method_list: script_method_list(&script),
            script,
        }
    }
}

impl ScriptInstance for RustScriptPlaceholder {
    fn class_name(&self) -> GString {
        script_class_name(&self.script)
    }

    fn set(&mut self, name: StringName, value: &Variant) -> bool {
        let exists = self
            .get_property_list()
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

    fn get_property_list(&self) -> &[PropertyInfo] {
        self.property_list.as_ref()
    }

    fn get_method_list(&self) -> &[MethodInfo] {
        self.method_list.as_ref()
    }

    fn call(
        &mut self,
        _method: StringName,
        _args: &[&Variant],
    ) -> Result<Variant, godot::sys::GDExtensionCallErrorType> {
        Err(godot::sys::GDEXTENSION_CALL_OK)
    }

    fn get_script(&self) -> &Gd<Script> {
        &self.generic_script
    }

    fn has_method(&self, method_name: StringName) -> bool {
        self.get_method_list()
            .iter()
            .any(|method| method.method_name == method_name)
    }

    fn is_placeholder(&self) -> bool {
        true
    }

    fn get_property_type(&self, name: StringName) -> godot::sys::VariantType {
        self.get_property_list()
            .iter()
            .find(|prop| prop.property_name == name)
            .map(|prop| prop.variant_type)
            .unwrap_or(VariantType::Nil)
    }

    fn to_string(&self) -> GString {
        GString::new()
    }

    fn get_property_state(&self) -> Vec<(StringName, Variant)> {
        self.properties
            .iter()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect()
    }

    fn get_language(&self) -> Gd<godot::engine::ScriptLanguage> {
        RustScriptLanguage::alloc_gd().upcast()
    }

    fn on_refcount_decremented(&self) -> bool {
        true
    }

    fn on_refcount_incremented(&self) {}

    fn property_get_fallback(&self, _name: StringName) -> Option<Variant> {
        None
    }

    fn property_set_fallback(&mut self, _name: StringName, _value: &Variant) -> bool {
        false
    }
}
