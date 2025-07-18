/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

#[cfg(since_api = "4.3")]
use godot::builtin::PackedVector4Array;
use godot::builtin::{
    Aabb, Array, Basis, Callable, Color, Dictionary, GString, NodePath, PackedByteArray,
    PackedColorArray, PackedFloat32Array, PackedFloat64Array, PackedInt32Array, PackedInt64Array,
    PackedStringArray, PackedVector2Array, PackedVector3Array, Plane, Projection, Quaternion,
    Rect2, Rect2i, Rid, StringName, Transform2D, Transform3D, Vector2, Vector2i, Vector3, Vector3i,
    Vector4, Vector4i,
};
use godot::classes::{Node, Resource};
use godot::global::PropertyHint;
use godot::meta::{ArrayElement, FromGodot, GodotConvert, GodotType, ToGodot};
use godot::obj::{EngineEnum, Gd};
use godot::prelude::GodotClass;
use godot::sys::GodotFfi;

use super::{GodotScript, RsRef};

pub trait GodotScriptExport: GodotConvert + FromGodot + ToGodot {
    fn hint_string(custom_hint: Option<PropertyHint>, custom_string: Option<String>) -> String;

    fn hint(custom: Option<PropertyHint>) -> PropertyHint;
}

impl<T: GodotClass> GodotScriptExport for Gd<T> {
    fn hint_string(_custom_hint: Option<PropertyHint>, custom_string: Option<String>) -> String {
        if let Some(custom) = custom_string {
            return custom;
        }

        T::class_name().to_string()
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
    for<'v> T: 'v,
    for<'v> <<T as ToGodot>::ToVia<'v> as GodotType>::Ffi: godot::sys::GodotNullableFfi,
    for<'f> <<T as GodotConvert>::Via as GodotType>::ToFfi<'f>: godot::sys::GodotNullableFfi,
    <<T as GodotConvert>::Via as GodotType>::Ffi: godot::sys::GodotNullableFfi,
    for<'v, 'f> <<T as ToGodot>::ToVia<'v> as GodotType>::ToFfi<'f>: godot::sys::GodotNullableFfi,
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
default_export!(u64);

default_export!(Callable);
default_export!(godot::builtin::Signal);
default_export!(Dictionary);

default_export!(Rid);
