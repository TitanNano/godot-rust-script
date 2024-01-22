/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::ops::Deref;

use abi_stable::std_types::RBox;
use godot::{
    obj::{EngineBitfield, EngineEnum},
    prelude::{
        meta::{ClassName, MethodInfo, PropertyInfo},
        Array, Dictionary, Gd, Object, StringName, ToGodot,
    },
    sys::VariantType,
};

use crate::{
    apply::Apply,
    script_registry::{
        CreateScriptInstanceData_TO, RemoteGodotScript_TO, RemoteScriptMetaData,
        RemoteScriptMethodInfo, RemoteScriptPropertyInfo, RemoteScriptSignalInfo,
    },
};

#[derive(Debug, Clone)]
pub struct ScriptMetaData {
    class_name: ClassName,
    base_type_name: StringName,
    properties: Vec<RemoteScriptPropertyInfo>,
    methods: Vec<RemoteScriptMethodInfo>,
    signals: Vec<RemoteScriptSignalInfo>,
    create_data: CreateScriptInstanceData_TO<'static, RBox<()>>,
    description: &'static str,
}

impl ScriptMetaData {
    pub fn class_name(&self) -> ClassName {
        self.class_name
    }

    pub fn base_type_name(&self) -> StringName {
        self.base_type_name.clone()
    }

    pub fn create_data(&self, base: Gd<Object>) -> RemoteGodotScript_TO<'static, RBox<()>> {
        self.create_data.create(base.to_variant().into())
    }

    pub fn properties(&self) -> &[RemoteScriptPropertyInfo] {
        &self.properties
    }

    pub fn methods(&self) -> &[RemoteScriptMethodInfo] {
        &self.methods
    }

    pub fn signals(&self) -> &[RemoteScriptSignalInfo] {
        &self.signals
    }

    pub fn description(&self) -> &'static str {
        self.description
    }
}

impl From<RemoteScriptMetaData> for ScriptMetaData {
    fn from(value: RemoteScriptMetaData) -> Self {
        Self {
            class_name: ClassName::from_ascii_cstr(value.class_name.as_str().as_bytes()),
            base_type_name: StringName::from(&value.base_type_name.as_str()),
            properties: value.properties.to_vec(),
            methods: value.methods.to_vec(),
            signals: value.signals.to_vec(),
            create_data: value.create_data,
            description: value.description.as_str(),
        }
    }
}

pub(super) trait ToDictionary {
    fn to_dict(&self) -> Dictionary;
}

impl ToDictionary for PropertyInfo {
    fn to_dict(&self) -> Dictionary {
        let mut dict = Dictionary::new();

        dict.set("name", self.property_name.clone());
        dict.set("class_name", self.class_name.to_string_name());
        dict.set("type", self.variant_type as i32);
        dict.set("hint", self.hint.ord());
        dict.set("hint_string", self.hint_string.clone());
        dict.set("usage", self.usage.ord());

        dict
    }
}

impl ToDictionary for MethodInfo {
    fn to_dict(&self) -> Dictionary {
        Dictionary::new().apply(|dict| {
            dict.set("name", self.method_name.clone());
            dict.set("flags", self.flags.ord());

            let args: Array<_> = self.arguments.iter().map(|arg| arg.to_dict()).collect();

            dict.set("args", args);

            dict.set("return", self.return_type.to_dict());
        })
    }
}

fn variant_type_to_str(var_type: VariantType) -> &'static str {
    use VariantType as V;

    match var_type {
        V::Nil => "void",
        V::Bool => "Bool",
        V::Int => "Int",
        V::Float => "Float",
        V::String => "String",
        V::Vector2 => "Vector2",
        V::Vector2i => "Vector2i",
        V::Rect2 => "Rect2",
        V::Rect2i => "Rect2i",
        V::Vector3 => "Vector3",
        V::Vector3i => "Vector3i",
        V::Transform2D => "Transform2D",
        V::Vector4 => "Vector4",
        V::Vector4i => "Vector4i",
        V::Plane => "Plane",
        V::Quaternion => "Quaternion",
        V::Aabb => "Aabb",
        V::Basis => "Basis",
        V::Transform3D => "Transform3D",
        V::Projection => "Projection",
        V::Color => "Color",
        V::StringName => "StringName",
        V::NodePath => "NodePath",
        V::Rid => "Rid",
        V::Object => "Object",
        V::Callable => "Callable",
        V::Signal => "Signal",
        V::Dictionary => "Dictionary",
        V::Array => "Array",
        V::PackedByteArray => "PackedByteArray",
        V::PackedInt32Array => "PackedInt32Array",
        V::PackedInt64Array => "PackedInt64Array",
        V::PackedColorArray => "PackedColorArray",
        V::PackedStringArray => "PackedStringArray",
        V::PackedVector3Array => "PackedVector3Array",
        V::PackedVector2Array => "PackedVector2Array",
        V::PackedFloat64Array => "PackedFloat64Array",
        V::PackedFloat32Array => "PackedFloat32Array",
    }
}

pub trait ToMethodDoc {
    fn to_method_doc(&self) -> Dictionary;
}

impl ToMethodDoc for MethodInfo {
    fn to_method_doc(&self) -> Dictionary {
        let args: Array<Dictionary> = self
            .arguments
            .iter()
            .map(|arg| arg.to_argument_doc())
            .collect();

        Dictionary::new().apply(|dict| {
            dict.set("name", self.method_name.clone());
            dict.set(
                "return_type",
                variant_type_to_str(self.return_type.variant_type),
            );
            dict.set("is_deprecated", false);
            dict.set("is_experimental", false);
            dict.set("arguments", args);
        })
    }
}

impl<T: ToMethodDoc> ToMethodDoc for Documented<T> {
    fn to_method_doc(&self) -> Dictionary {
        self.inner
            .to_method_doc()
            .apply(|dict| dict.set("description", self.description))
    }
}

#[derive(Debug)]
pub struct Documented<T> {
    inner: T,
    description: &'static str,
}

impl From<crate::script_registry::RemoteScriptPropertyInfo> for Documented<PropertyInfo> {
    fn from(value: crate::script_registry::RemoteScriptPropertyInfo) -> Self {
        Self {
            description: value.description.as_str(),
            inner: value.into(),
        }
    }
}

impl From<crate::script_registry::RemoteScriptMethodInfo> for Documented<MethodInfo> {
    fn from(value: crate::script_registry::RemoteScriptMethodInfo) -> Self {
        Self {
            description: value.description.as_str(),
            inner: value.into(),
        }
    }
}

impl From<crate::script_registry::RemoteScriptSignalInfo> for Documented<MethodInfo> {
    fn from(value: crate::script_registry::RemoteScriptSignalInfo) -> Self {
        Self {
            description: value.description.as_str(),
            inner: value.into(),
        }
    }
}

impl<T> Deref for Documented<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: Clone> Clone for Documented<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            description: self.description,
        }
    }
}

pub trait ToArgumentDoc {
    fn to_argument_doc(&self) -> Dictionary;
}

impl ToArgumentDoc for PropertyInfo {
    fn to_argument_doc(&self) -> Dictionary {
        Dictionary::new().apply(|dict| {
            dict.set("name", self.property_name.clone());
            dict.set("type", variant_type_to_str(self.variant_type));
        })
    }
}

impl<T: ToArgumentDoc> ToArgumentDoc for Documented<T> {
    fn to_argument_doc(&self) -> Dictionary {
        self.inner.to_argument_doc().apply(|dict| {
            dict.set("description", self.description);
        })
    }
}

pub trait ToPropertyDoc {
    fn to_property_doc(&self) -> Dictionary;
}

impl ToPropertyDoc for PropertyInfo {
    fn to_property_doc(&self) -> Dictionary {
        Dictionary::new().apply(|dict| {
            dict.set("name", self.property_name.clone());
            dict.set("type", variant_type_to_str(self.variant_type));
            dict.set("is_deprecated", false);
            dict.set("is_experimental", false);
        })
    }
}

impl<T: ToPropertyDoc> ToPropertyDoc for Documented<T> {
    fn to_property_doc(&self) -> Dictionary {
        self.inner
            .to_property_doc()
            .apply(|dict| dict.set("description", self.description))
    }
}
