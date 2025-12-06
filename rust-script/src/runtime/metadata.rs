/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::borrow::Cow;
use std::ops::Deref;

use godot::meta::{ClassId, MethodInfo, PropertyInfo};
use godot::obj::{EngineBitfield, EngineEnum};
use godot::prelude::{Array, VarDictionary};
use godot::sys::VariantType;

use crate::apply::Apply;

pub(super) trait ToDictionary {
    fn to_dict(&self) -> VarDictionary;
}

impl ToDictionary for PropertyInfo {
    fn to_dict(&self) -> VarDictionary {
        let mut dict = VarDictionary::new();

        dict.set("name", self.property_name.clone());
        dict.set("class_name", self.class_id.to_string_name());
        dict.set("type", self.variant_type.ord());
        dict.set("hint", self.hint_info.hint.ord());
        dict.set("hint_string", self.hint_info.hint_string.clone());
        dict.set("usage", self.usage.ord());

        dict
    }
}

impl ToDictionary for MethodInfo {
    fn to_dict(&self) -> VarDictionary {
        VarDictionary::new().apply(|dict| {
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
        V::NIL => "void",
        V::BOOL => "Bool",
        V::INT => "Int",
        V::FLOAT => "Float",
        V::STRING => "String",
        V::VECTOR2 => "Vector2",
        V::VECTOR2I => "Vector2i",
        V::RECT2 => "Rect2",
        V::RECT2I => "Rect2i",
        V::VECTOR3 => "Vector3",
        V::VECTOR3I => "Vector3i",
        V::TRANSFORM2D => "Transform2D",
        V::VECTOR4 => "Vector4",
        V::VECTOR4I => "Vector4i",
        V::PLANE => "Plane",
        V::QUATERNION => "Quaternion",
        V::AABB => "Aabb",
        V::BASIS => "Basis",
        V::TRANSFORM3D => "Transform3D",
        V::PROJECTION => "Projection",
        V::COLOR => "Color",
        V::STRING_NAME => "StringName",
        V::NODE_PATH => "NodePath",
        V::RID => "Rid",
        V::OBJECT => "Object",
        V::CALLABLE => "Callable",
        V::SIGNAL => "Signal",
        V::DICTIONARY => "Dictionary",
        V::ARRAY => "Array",
        V::PACKED_BYTE_ARRAY => "PackedByteArray",
        V::PACKED_INT32_ARRAY => "PackedInt32Array",
        V::PACKED_INT64_ARRAY => "PackedInt64Array",
        V::PACKED_COLOR_ARRAY => "PackedColorArray",
        V::PACKED_STRING_ARRAY => "PackedStringArray",
        V::PACKED_VECTOR3_ARRAY => "PackedVector3Array",
        V::PACKED_VECTOR2_ARRAY => "PackedVector2Array",
        V::PACKED_FLOAT64_ARRAY => "PackedFloat64Array",
        V::PACKED_FLOAT32_ARRAY => "PackedFloat32Array",
        _ => "UNKNOWN",
    }
}

fn prop_doc_type(prop_type: VariantType, class_name: ClassId) -> Cow<'static, str> {
    match prop_type {
        VariantType::OBJECT => class_name.to_cow_str(),
        _ => variant_type_to_str(prop_type).into(),
    }
}

pub trait ToMethodDoc {
    fn to_method_doc(&self) -> VarDictionary;
}

impl ToMethodDoc for MethodInfo {
    fn to_method_doc(&self) -> VarDictionary {
        let args: Array<VarDictionary> = self
            .arguments
            .iter()
            .map(|arg| arg.to_argument_doc())
            .collect();

        VarDictionary::new().apply(|dict| {
            dict.set("name", self.method_name.clone());
            dict.set(
                "return_type",
                prop_doc_type(self.return_type.variant_type, self.return_type.class_id).as_ref(),
            );
            dict.set("is_deprecated", false);
            dict.set("is_experimental", false);
            dict.set("arguments", args);
        })
    }
}

impl<T: ToMethodDoc> ToMethodDoc for Documented<T> {
    fn to_method_doc(&self) -> VarDictionary {
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

impl From<crate::static_script_registry::RustScriptPropertyInfo> for Documented<PropertyInfo> {
    fn from(value: crate::static_script_registry::RustScriptPropertyInfo) -> Self {
        Self {
            description: value.description,
            inner: (&value).into(),
        }
    }
}

impl From<crate::static_script_registry::RustScriptMethodInfo> for Documented<MethodInfo> {
    fn from(value: crate::static_script_registry::RustScriptMethodInfo) -> Self {
        Self {
            description: value.description,
            inner: (&value).into(),
        }
    }
}

impl From<crate::static_script_registry::RustScriptSignalInfo> for Documented<MethodInfo> {
    fn from(value: crate::static_script_registry::RustScriptSignalInfo) -> Self {
        Self {
            description: value.description,
            inner: (&value).into(),
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
    fn to_argument_doc(&self) -> VarDictionary;
}

impl ToArgumentDoc for PropertyInfo {
    fn to_argument_doc(&self) -> VarDictionary {
        VarDictionary::new().apply(|dict| {
            dict.set("name", self.property_name.clone());
            dict.set(
                "type",
                prop_doc_type(self.variant_type, self.class_id).as_ref(),
            );
        })
    }
}

impl<T: ToArgumentDoc> ToArgumentDoc for Documented<T> {
    fn to_argument_doc(&self) -> VarDictionary {
        self.inner.to_argument_doc().apply(|dict| {
            dict.set("description", self.description);
        })
    }
}

pub trait ToPropertyDoc {
    fn to_property_doc(&self) -> VarDictionary;
}

impl ToPropertyDoc for PropertyInfo {
    fn to_property_doc(&self) -> VarDictionary {
        VarDictionary::new().apply(|dict| {
            dict.set("name", self.property_name.clone());
            dict.set(
                "type",
                prop_doc_type(self.variant_type, self.class_id).as_ref(),
            );
            dict.set("is_deprecated", false);
            dict.set("is_experimental", false);
        })
    }
}

impl<T: ToPropertyDoc> ToPropertyDoc for Documented<T> {
    fn to_property_doc(&self) -> VarDictionary {
        self.inner
            .to_property_doc()
            .apply(|dict| dict.set("description", self.description))
    }
}
