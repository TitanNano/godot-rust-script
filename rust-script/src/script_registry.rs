/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::{collections::HashMap, fmt::Debug, sync::Arc};

use godot::{
    builtin::{
        meta::{ClassName, MethodInfo, PropertyInfo},
        GString,
    },
    engine::global::{MethodFlags, PropertyHint, PropertyUsageFlags},
    obj::{EngineBitfield, EngineEnum},
    prelude::{Gd, Object, StringName, Variant},
    sys::VariantType,
};

pub trait GodotScript: Debug + GodotScriptImpl {
    fn set(&mut self, name: StringName, value: Variant) -> bool;
    fn get(&self, name: StringName) -> Option<Variant>;
    fn call(
        &mut self,
        method: StringName,
        args: &[&Variant],
    ) -> Result<Variant, godot::sys::GDExtensionCallErrorType>;

    fn to_string(&self) -> String;
    fn property_state(&self) -> HashMap<StringName, Variant>;

    fn default_with_base(base: godot::prelude::Gd<godot::prelude::Object>) -> Self;
}

pub trait GodotScriptImpl {
    fn call_fn(
        &mut self,
        name: StringName,
        args: &[&Variant],
    ) -> Result<Variant, godot::sys::GDExtensionCallErrorType>;
}

pub trait GodotScriptObject {
    fn set(&mut self, name: StringName, value: Variant) -> bool;
    fn get(&self, name: StringName) -> Option<Variant>;
    fn call(
        &mut self,
        method: StringName,
        args: &[&Variant],
    ) -> Result<Variant, godot::sys::GDExtensionCallErrorType>;
    fn to_string(&self) -> String;
    fn property_state(&self) -> HashMap<StringName, Variant>;
}

impl<T: GodotScript> GodotScriptObject for T {
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
    ) -> Result<Variant, godot::sys::GDExtensionCallErrorType> {
        GodotScript::call(self, method, args)
    }

    fn to_string(&self) -> String {
        GodotScript::to_string(self)
    }

    fn property_state(&self) -> HashMap<StringName, Variant> {
        GodotScript::property_state(self)
    }
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct RustScriptPropertyInfo {
    pub variant_type: VariantType,
    pub property_name: &'static str,
    pub class_name: &'static str,
    pub hint: i32,
    pub hint_string: &'static str,
    pub usage: u64,
    pub description: &'static str,
}

impl From<&RustScriptPropertyInfo> for PropertyInfo {
    fn from(value: &RustScriptPropertyInfo) -> Self {
        Self {
            variant_type: value.variant_type,
            property_name: value.property_name.into(),
            class_name: ClassName::from_ascii_cstr(value.class_name.as_bytes()),
            hint: PropertyHint::try_from_ord(value.hint).unwrap_or(PropertyHint::NONE),
            hint_string: value.hint_string.into(),
            usage: PropertyUsageFlags::try_from_ord(value.usage)
                .unwrap_or(PropertyUsageFlags::NONE),
        }
    }
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct RustScriptMethodInfo {
    pub id: i32,
    pub method_name: &'static str,
    pub class_name: &'static str,
    pub return_type: RustScriptPropertyInfo,
    pub arguments: Box<[RustScriptPropertyInfo]>,
    pub flags: u64,
    pub description: &'static str,
}

impl From<&RustScriptMethodInfo> for MethodInfo {
    fn from(value: &RustScriptMethodInfo) -> Self {
        Self {
            id: value.id,
            method_name: value.method_name.into(),
            class_name: ClassName::from_ascii_cstr(value.class_name.as_bytes()),
            return_type: (&value.return_type).into(),
            arguments: value.arguments.iter().map(|arg| arg.into()).collect(),
            default_arguments: vec![],
            flags: MethodFlags::try_from_ord(value.flags).unwrap_or(MethodFlags::DEFAULT),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RustScriptSignalInfo {
    pub name: &'static str,
    pub arguments: Box<[RustScriptPropertyInfo]>,
    pub description: &'static str,
}

impl From<&RustScriptSignalInfo> for MethodInfo {
    fn from(value: &RustScriptSignalInfo) -> Self {
        Self {
            id: 0,
            method_name: value.name.into(),
            class_name: ClassName::none(),
            return_type: PropertyInfo {
                variant_type: VariantType::Nil,
                class_name: ClassName::none(),
                property_name: StringName::default(),
                hint: PropertyHint::NONE,
                hint_string: GString::default(),
                usage: PropertyUsageFlags::NONE,
            },
            arguments: value.arguments.iter().map(|arg| arg.into()).collect(),
            default_arguments: vec![],
            flags: MethodFlags::NORMAL,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RustScriptMetaData {
    pub(crate) class_name: ClassName,
    pub(crate) base_type_name: StringName,
    pub(crate) properties: Box<[RustScriptPropertyInfo]>,
    pub(crate) methods: Box<[RustScriptMethodInfo]>,
    pub(crate) signals: Box<[RustScriptSignalInfo]>,
    pub(crate) create_data: Arc<dyn CreateScriptInstanceData>,
    pub(crate) description: &'static str,
}

impl RustScriptMetaData {
    pub fn new(
        class_name: &'static str,
        base_type_name: StringName,
        properties: Box<[RustScriptPropertyInfo]>,
        methods: Box<[RustScriptMethodInfo]>,
        signals: Box<[RustScriptSignalInfo]>,
        create_data: Box<dyn CreateScriptInstanceData>,
        description: &'static str,
    ) -> Self {
        Self {
            class_name: ClassName::from_ascii_cstr(class_name.as_bytes()),
            base_type_name,
            properties,
            methods,
            signals,
            create_data: Arc::from(create_data),
            description,
        }
    }
}

impl RustScriptMetaData {
    pub fn class_name(&self) -> ClassName {
        self.class_name
    }

    pub fn base_type_name(&self) -> StringName {
        self.base_type_name.clone()
    }

    pub fn create_data(&self, base: Gd<Object>) -> Box<dyn GodotScriptObject> {
        self.create_data.create(base)
    }

    pub fn properties(&self) -> &[RustScriptPropertyInfo] {
        &self.properties
    }

    pub fn methods(&self) -> &[RustScriptMethodInfo] {
        &self.methods
    }

    pub fn signals(&self) -> &[RustScriptSignalInfo] {
        &self.signals
    }

    pub fn description(&self) -> &'static str {
        self.description
    }
}

pub trait CreateScriptInstanceData: Sync + Send + Debug {
    fn create(&self, base: Gd<Object>) -> Box<dyn GodotScriptObject>;
}

impl<F> CreateScriptInstanceData for F
where
    F: (Fn(Gd<Object>) -> Box<dyn GodotScriptObject>) + Send + Sync + Debug,
{
    fn create(&self, base: Gd<Object>) -> Box<dyn GodotScriptObject> {
        self(base)
    }
}
