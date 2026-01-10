/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::any::Any;
use std::{collections::HashMap, ops::DerefMut};

use godot::classes::Script;
use godot::meta::error::CallErrorType;
use godot::meta::{MethodInfo, PropertyInfo};
use godot::obj::script::{ScriptInstance, SiMut};
use godot::prelude::{GString, Gd, Object, StringName, Variant, VariantType};
use godot_cell::blocking::GdCell;

use super::Context;
use super::call_context::GenericContext;
use super::{SCRIPT_REGISTRY, rust_script::RustScript, rust_script_language::RustScriptLanguage};
use crate::GodotScript;

fn script_method_list(script: &Gd<RustScript>) -> Box<[MethodInfo]> {
    let rs = script.bind();
    let class_name = rs.str_class_name();

    SCRIPT_REGISTRY
        .read()
        .expect("script registry is inaccessible")
        .get(&class_name)
        .map(|meta| {
            meta.methods()
                .iter()
                .cloned()
                .map(MethodInfo::from)
                .collect()
        })
        .unwrap_or_else(|| Box::new([]) as Box<[MethodInfo]>)
}

fn script_class_name(script: &Gd<RustScript>) -> GString {
    script.bind().get_class_name()
}

fn script_property_list(script: &Gd<RustScript>) -> Box<[PropertyInfo]> {
    let rs = script.bind();
    let class_name = rs.str_class_name();

    SCRIPT_REGISTRY
        .read()
        .expect("script registry is inaccessible")
        .get(&class_name)
        .map(|meta| meta.properties().iter().map(PropertyInfo::from).collect())
        .unwrap_or_else(|| Box::new([]) as Box<[PropertyInfo]>)
}

pub trait GodotScriptObject {
    fn set(&mut self, name: StringName, value: Variant) -> bool;
    fn get(&self, name: StringName) -> Option<Variant>;
    fn call(
        &mut self,
        method: StringName,
        args: &[&Variant],
        context: GenericContext,
    ) -> Result<Variant, CallErrorType>;
    fn to_string(&self) -> String;
    fn property_state(&self) -> HashMap<StringName, Variant>;

    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: GodotScript + 'static> GodotScriptObject for T {
    fn set(&mut self, name: StringName, value: Variant) -> bool {
        GodotScript::set(self, name, value)
    }

    fn get(&self, name: StringName) -> Option<Variant> {
        GodotScript::get(self, name)
    }

    fn call(
        &mut self,
        method: StringName,
        args: &[&Variant],
        context: GenericContext,
    ) -> Result<Variant, CallErrorType> {
        GodotScript::call(self, method, args, Context::from(context))
    }

    fn to_string(&self) -> String {
        GodotScript::to_string(self)
    }

    fn property_state(&self) -> HashMap<StringName, Variant> {
        GodotScript::property_state(self)
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub(crate) struct RustScriptInstance {
    data: GdCell<Box<dyn GodotScriptObject>>,

    script: Gd<RustScript>,
    generic_script: Gd<Script>,
    property_list: Box<[PropertyInfo]>,
    method_list: Box<[MethodInfo]>,
}

impl RustScriptInstance {
    pub fn new(
        data: Box<dyn GodotScriptObject>,
        _gd_object: Gd<Object>,
        script: Gd<RustScript>,
    ) -> Self {
        Self {
            data: GdCell::new(data),
            generic_script: script.clone().upcast(),
            property_list: script_property_list(&script),
            method_list: script_method_list(&script),
            script,
        }
    }
}

impl ScriptInstance for RustScriptInstance {
    type Base = Object;

    fn class_name(&self) -> GString {
        script_class_name(&self.script)
    }

    fn set_property(this: SiMut<Self>, name: StringName, value: &Variant) -> bool {
        let cell_ref = &this.data;
        let mut mut_data = match cell_ref.borrow_mut() {
            Ok(guard) => guard,
            Err(err) => {
                panic!(
                    "Error while writing to script property {}::{name}: {err}",
                    this.script.bind().get_class_name()
                );
            }
        };

        mut_data.set(name, value.to_owned())
    }

    fn get_property(&self, name: StringName) -> Option<Variant> {
        let guard = match self.data.borrow() {
            Ok(guard) => guard,
            Err(err) => {
                panic!(
                    "Error while reading script property {}::{name}: {err}",
                    self.script.bind().get_class_name()
                );
            }
        };

        guard.get(name)
    }

    fn get_property_list(&self) -> Vec<PropertyInfo> {
        self.property_list.to_vec()
    }

    fn get_method_list(&self) -> Vec<MethodInfo> {
        self.method_list.to_vec()
    }

    fn call(
        mut this: SiMut<Self>,
        method: StringName,
        args: &[&Variant],
    ) -> Result<Variant, CallErrorType> {
        let cell: *const _ = &this.data;

        let base = this.base_mut();

        // SAFETY: cell pointer was just created and is valid. It will not outlive the current function.
        let mut data_guard = match unsafe { &*cell }.borrow_mut() {
            Ok(guard) => guard,
            Err(err) => {
                drop(base);

                panic!(
                    "Error while calling script function {}::{}: {}",
                    this.script.bind().get_class_name(),
                    method,
                    err
                );
            }
        };
        let data = data_guard.deref_mut();
        let data_ptr = data as *mut _;

        // SAFETY: cell & data_ptr are valid for the duration of the call. The context can not outlive the current function as it's tied
        // to the lifetime of the base ref.
        let context = unsafe { GenericContext::new(cell, data_ptr, base) };

        data.call(method, args, context)
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
            .unwrap_or(godot::sys::VariantType::NIL)
    }

    fn to_string(&self) -> GString {
        GString::new()
    }

    fn get_property_state(&self) -> Vec<(StringName, Variant)> {
        self.get_property_list()
            .iter()
            .map(|prop| &prop.property_name)
            .filter_map(|name| {
                self.get_property(name.to_owned())
                    .map(|value| (name.to_owned(), value))
            })
            .collect()
    }

    fn get_language(&self) -> Gd<godot::classes::ScriptLanguage> {
        RustScriptLanguage::singleton()
            .map(Gd::upcast)
            .expect("RustScriptLanguage singleton is not initialized")
    }

    fn on_refcount_decremented(&self) -> bool {
        true
    }

    fn on_refcount_incremented(&self) {}

    fn property_get_fallback(&self, _name: StringName) -> Option<Variant> {
        None
    }

    fn property_set_fallback(_this: SiMut<Self>, _name: StringName, _value: &Variant) -> bool {
        false
    }

    #[cfg(since_api = "4.3")]
    fn get_method_argument_count(&self, method: StringName) -> Option<u32> {
        self.method_list
            .iter()
            .find(|m| m.method_name == method)
            .map(|method| method.arguments.len() as u32)
    }
}

pub(super) struct RustScriptPlaceholder {
    script: Gd<RustScript>,
    generic_script: Gd<Script>,
    properties: HashMap<StringName, Variant>,
    property_list: Box<[PropertyInfo]>,
    method_list: Box<[MethodInfo]>,
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
    type Base = Object;

    fn class_name(&self) -> GString {
        script_class_name(&self.script)
    }

    fn set_property(mut this: SiMut<Self>, name: StringName, value: &Variant) -> bool {
        let exists = this
            .get_property_list()
            .iter()
            .any(|prop| prop.property_name == name);

        if !exists {
            return false;
        }

        this.properties.insert(name, value.to_owned());
        true
    }

    fn get_property(&self, name: StringName) -> Option<Variant> {
        self.properties.get(&name).cloned()
    }

    fn get_property_list(&self) -> Vec<PropertyInfo> {
        self.property_list.to_vec()
    }

    fn get_method_list(&self) -> Vec<MethodInfo> {
        self.method_list.to_vec()
    }

    fn call(
        _this: SiMut<Self>,
        _method: StringName,
        _args: &[&Variant],
    ) -> Result<Variant, CallErrorType> {
        Err(CallErrorType::InvalidMethod)
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
            .unwrap_or(VariantType::NIL)
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

    fn get_language(&self) -> Gd<godot::classes::ScriptLanguage> {
        RustScriptLanguage::singleton()
            .map(Gd::upcast)
            .expect("RustScriptLanguage singleton is not initialized")
    }

    fn on_refcount_decremented(&self) -> bool {
        true
    }

    fn on_refcount_incremented(&self) {}

    fn property_get_fallback(&self, _name: StringName) -> Option<Variant> {
        None
    }

    fn property_set_fallback(_this: SiMut<Self>, _name: StringName, _value: &Variant) -> bool {
        false
    }

    #[cfg(since_api = "4.3")]
    fn get_method_argument_count(&self, method: StringName) -> Option<u32> {
        self.method_list
            .iter()
            .find(|m| m.method_name == method)
            .map(|method| method.arguments.len() as u32)
    }
}
