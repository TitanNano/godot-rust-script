/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use core::panic;
use std::{collections::HashMap, fmt::Debug, ops::DerefMut, pin::Pin};

use godot::{
    builtin::meta::{MethodInfo, PropertyInfo},
    engine::{Script, ScriptInstance, SiMut},
    prelude::{GString, Gd, Object, StringName, Variant, VariantType},
};
use godot_cell::GdCell;

use crate::script_registry::GodotScriptObject;

use super::{rust_script::RustScript, rust_script_language::RustScriptLanguage, SCRIPT_REGISTRY};

fn script_method_list(script: &Gd<RustScript>) -> Box<[MethodInfo]> {
    let rs = script.bind();
    let class_name = rs.str_class_name();

    let methods = SCRIPT_REGISTRY
        .read()
        .expect("script registry is inaccessible")
        .get(&class_name)
        .map(|meta| meta.methods().iter().map(MethodInfo::from).collect())
        .unwrap_or_else(|| Box::new([]) as Box<[MethodInfo]>);

    methods
}

fn script_class_name(script: &Gd<RustScript>) -> GString {
    script.bind().get_class_name()
}

fn script_property_list(script: &Gd<RustScript>) -> Box<[PropertyInfo]> {
    let rs = script.bind();
    let class_name = rs.str_class_name();

    let props = SCRIPT_REGISTRY
        .read()
        .expect("script registry is inaccessible")
        .get(&class_name)
        .map(|meta| meta.properties().iter().map(PropertyInfo::from).collect())
        .unwrap_or_else(|| Box::new([]) as Box<[PropertyInfo]>);

    props
}

pub struct Context<'a> {
    cell: Pin<&'a GdCell<Box<dyn GodotScriptObject>>>,
    data_ptr: *mut Box<dyn GodotScriptObject>,
}

impl<'a> Debug for Context<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Context { <Call Context> }")
    }
}

impl<'a> Context<'a> {
    unsafe fn new(
        cell: Pin<&'a GdCell<Box<dyn GodotScriptObject>>>,
        data_ptr: *mut Box<dyn GodotScriptObject>,
    ) -> Self {
        Self { cell, data_ptr }
    }

    pub fn reentrant_scope<T: GodotScriptObject + 'static, R>(
        &mut self,
        self_ref: &mut T,
        cb: impl FnOnce() -> R,
    ) -> R {
        let known_ptr = unsafe {
            let any = (*self.data_ptr).as_any_mut();

            any.downcast_mut::<T>().unwrap() as *mut T
        };

        let self_ptr = self_ref as *mut _;

        if known_ptr != self_ptr {
            panic!("unable to create reentrant scope with unrelated self reference!");
        }

        let guard = self
            .cell
            .make_inaccessible(unsafe { &mut *self.data_ptr })
            .unwrap();

        let result = cb();

        drop(guard);

        result
    }
}

pub(super) struct RustScriptInstance {
    data: Pin<Box<GdCell<Box<dyn GodotScriptObject>>>>,

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
        let cell_ref = this.data.as_ref();
        let mut mut_data = cell_ref.borrow_mut().unwrap();

        mut_data.set(name, value.to_owned())
    }

    fn get_property(&self, name: StringName) -> Option<Variant> {
        let guard = self.data.as_ref().borrow().unwrap();

        guard.get(name)
    }

    fn get_property_list(&self) -> Vec<PropertyInfo> {
        self.property_list.to_vec()
    }

    fn get_method_list(&self) -> Vec<MethodInfo> {
        self.method_list.to_vec()
    }

    fn call(
        this: SiMut<Self>,
        method: StringName,
        args: &[&Variant],
    ) -> Result<Variant, godot::sys::GDExtensionCallErrorType> {
        let cell = this.data.as_ref();
        let mut data_guard = cell.borrow_mut().unwrap();
        let data = data_guard.deref_mut();
        let data_ptr = data as *mut _;

        let context = unsafe { Context::new(cell, data_ptr) };

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
            .unwrap_or(godot::sys::VariantType::Nil)
    }

    fn to_string(&self) -> GString {
        // self.data.to_string().into()
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

    fn get_language(&self) -> Gd<godot::engine::ScriptLanguage> {
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
    ) -> Result<Variant, godot::sys::GDExtensionCallErrorType> {
        Err(godot::sys::GDEXTENSION_CALL_ERROR_INVALID_METHOD)
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
}
