/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::collections::HashMap;

#[cfg(since_api = "4.3")]
use godot::builtin::PackedVector4Array;
use godot::builtin::{
    Aabb, Array, Basis, Callable, Color, GString, NodePath, PackedByteArray, PackedColorArray,
    PackedFloat32Array, PackedFloat64Array, PackedInt32Array, PackedInt64Array, PackedStringArray,
    PackedVector2Array, PackedVector3Array, Plane, Projection, Quaternion, Rect2, Rect2i, Rid,
    StringName, Transform2D, Transform3D, VarDictionary, VariantType, Vector2, Vector2i, Vector3,
    Vector3i, Vector4, Vector4i,
};
use godot::classes::{Node, Resource};
use godot::global::{PropertyHint, PropertyUsageFlags};
use godot::meta::{ArrayElement, ClassId, GodotConvert, GodotType, ToGodot};
use godot::obj::{EngineEnum, Gd};
use godot::prelude::GodotClass;
use godot::register::property::BuiltinExport;
use godot::sys::GodotFfi;

use crate::private_export::RustScriptPropDesc;

use super::{GodotScript, OnEditor, RsRef};

pub trait GodotScriptExport {
    fn hint_string(custom_hint: Option<PropertyHint>, custom_string: Option<String>) -> String;

    fn hint(custom: Option<PropertyHint>) -> PropertyHint;
}

impl<T: GodotClass> GodotScriptExport for Gd<T> {
    fn hint_string(_custom_hint: Option<PropertyHint>, custom_string: Option<String>) -> String {
        if let Some(custom) = custom_string {
            return custom;
        }

        T::class_id().to_string()
    }

    fn hint(custom: Option<PropertyHint>) -> PropertyHint {
        if let Some(custom) = custom {
            return custom;
        }

        if T::inherits::<Node>() {
            PropertyHint::NODE_TYPE
        } else if T::inherits::<Resource>() {
            PropertyHint::RESOURCE_TYPE
        } else {
            PropertyHint::NONE
        }
    }
}

impl<T: GodotScript> GodotScriptExport for RsRef<T> {
    fn hint_string(_custom_hint: Option<PropertyHint>, custom_string: Option<String>) -> String {
        if let Some(custom) = custom_string {
            return custom;
        }

        T::CLASS_NAME.to_string()
    }

    fn hint(custom: Option<PropertyHint>) -> PropertyHint {
        if let Some(custom) = custom {
            return custom;
        }

        if T::Base::inherits::<Node>() {
            PropertyHint::NODE_TYPE
        } else if T::Base::inherits::<Resource>() {
            PropertyHint::RESOURCE_TYPE
        } else {
            PropertyHint::NONE
        }
    }
}

impl<T: GodotScriptExport> GodotScriptExport for Option<T>
where
    Self: GodotConvert + godot::prelude::Var,
{
    fn hint_string(custom_hint: Option<PropertyHint>, custom_string: Option<String>) -> String {
        T::hint_string(custom_hint, custom_string)
    }

    fn hint(custom: Option<PropertyHint>) -> PropertyHint {
        T::hint(custom)
    }
}

impl<T: ArrayElement + GodotScriptExport + GodotType> GodotScriptExport for Array<T> {
    fn hint_string(custom_hint: Option<PropertyHint>, custom_string: Option<String>) -> String {
        let element_type = <<T as GodotType>::Ffi as GodotFfi>::VARIANT_TYPE
            .variant_as_nil()
            .ord();
        let element_hint = <T as GodotScriptExport>::hint(custom_hint).ord();
        let element_hint_string = <T as GodotScriptExport>::hint_string(custom_hint, custom_string);

        format!("{}/{}:{}", element_type, element_hint, element_hint_string)
    }

    fn hint(custom: Option<PropertyHint>) -> PropertyHint {
        if let Some(custom) = custom {
            return custom;
        };

        PropertyHint::ARRAY_TYPE
    }
}

impl<T: GodotScriptExport> GodotScriptExport for OnEditor<T>
where
    Self: GodotConvert + godot::prelude::Var,
{
    fn hint_string(custom_hint: Option<PropertyHint>, custom_string: Option<String>) -> String {
        T::hint_string(custom_hint, custom_string)
    }

    fn hint(custom: Option<PropertyHint>) -> PropertyHint {
        T::hint(custom)
    }
}

impl<T: GodotScript> BuiltinExport for RsRef<T> {}

/// A group of properties that can are exported by a script.
///
// The script will flatten the properties into its own property list when exporting them to Godot, but groups them together.
pub trait ScriptPropertyGroup {
    const NAME: &'static str;

    fn get_property(&self, name: &str) -> godot::builtin::Variant;
    fn set_property(&mut self, name: &str, value: godot::builtin::Variant);
    fn properties() -> PropertyGroupBuilder;
    fn export_property_states(
        &self,
        prefix: &'static str,
        state: &mut HashMap<StringName, godot::builtin::Variant>,
    );
}

const OPTION_SCRIPT_PROPERTY_GROUP_PROP: &str = "enable";

impl<T: ScriptPropertyGroup + Default> ScriptPropertyGroup for Option<T> {
    const NAME: &'static str = T::NAME;

    fn get_property(&self, name: &str) -> godot::builtin::Variant {
        if name == OPTION_SCRIPT_PROPERTY_GROUP_PROP {
            return self.is_some().to_variant();
        }

        match self {
            Some(inner) => inner.get_property(name),
            None => godot::builtin::Variant::nil(),
        }
    }

    fn set_property(&mut self, name: &str, value: godot::builtin::Variant) {
        if name == OPTION_SCRIPT_PROPERTY_GROUP_PROP {
            if value.to::<bool>() {
                *self = Some(Default::default());
            } else {
                *self = None;
            }
            return;
        }

        if let Some(inner) = self {
            inner.set_property(name, value)
        }
    }

    fn properties() -> PropertyGroupBuilder {
        T::properties().add_property(RustScriptPropDesc {
            name: OPTION_SCRIPT_PROPERTY_GROUP_PROP.into(),
            ty: VariantType::BOOL,
            class_name: ClassId::none(),
            usage: PropertyUsageFlags::SCRIPT_VARIABLE
                | PropertyUsageFlags::EDITOR
                | PropertyUsageFlags::STORAGE,
            hint: PropertyHint::GROUP_ENABLE,
            hint_string: String::new(),
            description: "",
        })
    }

    fn export_property_states(
        &self,
        prefix: &'static str,
        state: &mut HashMap<StringName, godot::builtin::Variant>,
    ) {
        state.insert(
            format!("{}_{}", prefix, OPTION_SCRIPT_PROPERTY_GROUP_PROP)
                .as_str()
                .into(),
            self.get_property(OPTION_SCRIPT_PROPERTY_GROUP_PROP),
        );

        if let Some(inner) = self.as_ref() {
            T::export_property_states(inner, prefix, state);
        }
    }
}

pub struct PropertyGroupBuilder {
    name: &'static str,
    properties: Vec<RustScriptPropDesc>,
}

impl PropertyGroupBuilder {
    pub fn new(name: &'static str, capacity: usize) -> Self {
        Self {
            name,
            properties: Vec::with_capacity(capacity),
        }
    }

    pub fn add_property(mut self, property_desc: RustScriptPropDesc) -> Self {
        self.properties.push(property_desc);
        self
    }

    pub fn build(self, prefix: &str, description: &'static str) -> Box<[RustScriptPropDesc]> {
        [RustScriptPropDesc {
            name: self.name.into(),
            ty: VariantType::NIL,
            class_name: ClassId::none(),
            usage: PropertyUsageFlags::GROUP,
            hint: PropertyHint::NONE,
            hint_string: prefix.into(),
            description,
        }]
        .into_iter()
        .chain(self.properties.into_iter().map(|mut prop| {
            prop.name = format!("{prefix}{}", prop.name).into();
            prop
        }))
        .collect()
    }
}

macro_rules! default_export {
    ($ty:ty) => {
        impl GodotScriptExport for $ty {
            fn hint_string(
                _custom_hint: Option<PropertyHint>,
                custom_string: Option<String>,
            ) -> String {
                if let Some(custom) = custom_string {
                    return custom;
                }

                String::new()
            }

            fn hint(custom: Option<PropertyHint>) -> PropertyHint {
                if let Some(custom) = custom {
                    return custom;
                }

                PropertyHint::NONE
            }
        }
    };
}

// Bounding Boxes
default_export!(Aabb);
default_export!(Rect2);
default_export!(Rect2i);

// Matrices
default_export!(Basis);
default_export!(Transform2D);
default_export!(Transform3D);
default_export!(Projection);

// Vectors
default_export!(Vector2);
default_export!(Vector2i);
default_export!(Vector3);
default_export!(Vector3i);
default_export!(Vector4);
default_export!(Vector4i);

// Misc Math
default_export!(Quaternion);
default_export!(Plane);

// Stringy Types
default_export!(GString);
default_export!(StringName);
default_export!(NodePath);

default_export!(Color);

// Arrays
default_export!(PackedByteArray);
default_export!(PackedInt32Array);
default_export!(PackedInt64Array);
default_export!(PackedFloat32Array);
default_export!(PackedFloat64Array);
default_export!(PackedStringArray);
default_export!(PackedVector2Array);
default_export!(PackedVector3Array);
#[cfg(since_api = "4.3")]
default_export!(PackedVector4Array);
default_export!(PackedColorArray);

// Primitives
default_export!(f64);
default_export!(i64);
default_export!(bool);
default_export!(f32);

default_export!(i32);
default_export!(i16);
default_export!(i8);
default_export!(u32);
default_export!(u16);
default_export!(u8);

default_export!(Callable);
default_export!(godot::builtin::Signal);
default_export!(VarDictionary);

default_export!(Rid);
