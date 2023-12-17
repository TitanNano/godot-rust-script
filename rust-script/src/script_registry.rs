use std::{collections::HashMap, fmt::Debug, marker::PhantomData};

use abi_stable::{
    sabi_trait::TD_CanDowncast,
    std_types::{RBox, RHashMap, ROption, RResult, RStr, RString, RVec},
    StableAbi,
};
use godot::{
    engine::global::{MethodFlags, PropertyHint, PropertyUsageFlags},
    obj::EngineEnum,
    prelude::{
        godot_print,
        meta::{ClassName, MethodInfo, PropertyInfo},
        Gd, Object, StringName, Variant,
    },
    sys::VariantType,
};

#[abi_stable::sabi_trait]
pub trait RemoteGodotScript {
    fn set(&mut self, name: RString, value: RemoteValueRef) -> bool;
    fn get(&self, name: RString) -> ROption<RemoteValue>;
    fn call(&mut self, method: RString, args: RVec<RemoteValueRef>) -> RResult<RemoteValue, u32>;
    fn to_string(&self) -> RString;
    fn property_state(&self) -> RHashMap<RString, RemoteValue>;
}

impl<T> RemoteGodotScript for T
where
    T: GodotScript,
{
    fn set(&mut self, name: RString, value: RemoteValueRef) -> bool {
        godot_print!(
            "RemoteGodotScript received set_func for \"{}\": {:?}",
            name,
            value
        );

        self.set(StringName::from(name.as_str()), value.as_ref().to())
    }

    fn get(&self, name: RString) -> ROption<RemoteValue> {
        let result = self.get(StringName::from(name.as_str()));

        match result {
            Some(v) => ROption::RSome(RemoteValue::from(v)),
            None => ROption::RNone,
        }
    }

    fn call(&mut self, method: RString, args: RVec<RemoteValueRef>) -> RResult<RemoteValue, u32> where
    {
        let args: Vec<_> = args.into_iter().map(|vref| vref.as_ref()).collect();

        self.call(method.as_str().into(), &args)
            .map(RemoteValue::from)
            // GDExtensionCallErrorType is not guaranteed to be a u32
            .map_err(godot_call_error_type_to_u32)
            .into()
    }

    fn to_string(&self) -> RString {
        self.to_string().into()
    }

    fn property_state(&self) -> RHashMap<RString, RemoteValue> {
        self.property_state()
            .into_iter()
            .map(|(key, value)| (key.to_string().into(), value.into()))
            .collect()
    }
}

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

#[derive(Debug, StableAbi, Clone)]
#[repr(C)]
pub struct RemoteScriptPropertyInfo {
    pub variant_type: RemoteVariantType,
    pub property_name: RString,
    pub class_name: RStr<'static>,
    pub hint: i32,
    pub hint_string: RStr<'static>,
    pub usage: i32,
    pub description: RStr<'static>,
}

impl From<RemoteScriptPropertyInfo> for PropertyInfo {
    fn from(value: RemoteScriptPropertyInfo) -> Self {
        Self {
            variant_type: value.variant_type.into(),
            property_name: value.property_name.into(),
            class_name: ClassName::from_ascii_cstr(value.class_name.as_str().as_bytes()),
            hint: PropertyHint::try_from_ord(value.hint)
                .unwrap_or(PropertyHint::PROPERTY_HINT_NONE),
            hint_string: value.hint_string.into(),
            usage: PropertyUsageFlags::try_from_ord(value.usage)
                .unwrap_or(PropertyUsageFlags::PROPERTY_USAGE_NONE),
        }
    }
}

#[derive(Debug, StableAbi, Clone)]
#[repr(C)]
pub struct RemoteScriptMethodInfo {
    pub id: i32,
    pub method_name: RStr<'static>,
    pub class_name: RStr<'static>,
    pub return_type: RemoteScriptPropertyInfo,
    pub arguments: RVec<RemoteScriptPropertyInfo>,
    pub flags: i32,
    pub description: RStr<'static>,
}

impl From<RemoteScriptMethodInfo> for MethodInfo {
    fn from(value: RemoteScriptMethodInfo) -> Self {
        Self {
            id: value.id,
            method_name: value.method_name.into(),
            class_name: ClassName::from_ascii_cstr(value.class_name.as_str().as_bytes()),
            return_type: value.return_type.into(),
            arguments: value.arguments.into_iter().map(|arg| arg.into()).collect(),
            default_arguments: vec![],
            flags: MethodFlags::try_from_ord(value.flags)
                .unwrap_or(MethodFlags::METHOD_FLAGS_DEFAULT),
        }
    }
}

#[derive(Debug, StableAbi)]
#[repr(C)]
pub struct RemoteScriptMetaData {
    pub(crate) class_name: RStr<'static>,
    pub(crate) base_type_name: RStr<'static>,
    pub(crate) properties: RVec<RemoteScriptPropertyInfo>,
    pub(crate) methods: RVec<RemoteScriptMethodInfo>,
    pub(crate) create_data: CreateScriptInstanceData_TO<'static, RBox<()>>,
    pub(crate) description: RStr<'static>,
}

impl RemoteScriptMetaData {
    pub fn new<CD>(
        class_name: RStr<'static>,
        base_type_name: RStr<'static>,
        properties: RVec<RemoteScriptPropertyInfo>,
        methods: RVec<RemoteScriptMethodInfo>,
        create_data: CD,
        description: RStr<'static>,
    ) -> Self
    where
        CD: CreateScriptInstanceData + 'static,
    {
        Self {
            class_name,
            base_type_name,
            properties,
            methods,
            create_data: CreateScriptInstanceData_TO::from_value(create_data, TD_CanDowncast),
            description,
        }
    }
}

#[abi_stable::sabi_trait]
pub trait CreateScriptInstanceData: Debug {
    fn create(&self, base: RemoteValue) -> RemoteGodotScript_TO<'static, RBox<()>>;
}

impl<F> CreateScriptInstanceData for F
where
    F: Fn(Gd<Object>) -> RemoteGodotScript_TO<'static, RBox<()>> + Debug,
{
    fn create(&self, base: RemoteValue) -> RemoteGodotScript_TO<'static, RBox<()>> {
        let variant: Variant = base.into();

        self(variant.to())
    }
}

#[derive(Debug, StableAbi, Clone, Copy)]
#[repr(usize)]
pub enum RemoteVariantType {
    Nil,
    Bool,
    Int,
    Float,
    String,
    Vector2,
    Vector2i,
    Rect2,
    Rect2i,
    Vector3,
    Vector3i,
    Transform2D,
    Vector4,
    Vector4i,
    Plane,
    Quaternion,
    Aabb,
    Basis,
    Transform3D,
    Projection,
    Color,
    StringName,
    NodePath,
    Rid,
    Object,
    Callable,
    Signal,
    Dictionary,
    Array,
    PackedByteArray,
    PackedInt32Array,
    PackedInt64Array,
    PackedFloat32Array,
    PackedFloat64Array,
    PackedStringArray,
    PackedVector2Array,
    PackedVector3Array,
    PackedColorArray,
}

impl From<RemoteVariantType> for VariantType {
    fn from(value: RemoteVariantType) -> Self {
        use RemoteVariantType as V;

        match value {
            V::Nil => Self::Nil,
            V::Bool => Self::Bool,
            V::Int => Self::Int,
            V::Float => Self::Float,
            V::String => Self::String,
            V::Vector2 => Self::Vector2,
            V::Vector2i => Self::Vector2i,
            V::Rect2 => Self::Rect2,
            V::Rect2i => Self::Vector2i,
            V::Vector3 => Self::Vector3,
            V::Vector3i => Self::Vector3i,
            V::Transform2D => Self::Transform2D,
            V::Vector4 => Self::Vector4,
            V::Vector4i => Self::Vector4i,
            V::Plane => Self::Plane,
            V::Quaternion => Self::Quaternion,
            V::Aabb => Self::Aabb,
            V::Basis => Self::Basis,
            V::Transform3D => Self::Transform3D,
            V::Projection => Self::Projection,
            V::Color => Self::Color,
            V::StringName => Self::StringName,
            V::NodePath => Self::NodePath,
            V::Rid => Self::Rid,
            V::Object => Self::Object,
            V::Callable => Self::Callable,
            V::Signal => Self::Signal,
            V::Dictionary => Self::Dictionary,
            V::Array => Self::Array,
            V::PackedByteArray => Self::PackedByteArray,
            V::PackedInt32Array => Self::PackedInt32Array,
            V::PackedInt64Array => Self::PackedInt64Array,
            V::PackedFloat32Array => Self::PackedFloat32Array,
            V::PackedFloat64Array => Self::PackedInt64Array,
            V::PackedStringArray => Self::PackedStringArray,
            V::PackedVector2Array => Self::PackedVector2Array,
            V::PackedVector3Array => Self::PackedVector3Array,
            V::PackedColorArray => Self::PackedColorArray,
        }
    }
}

impl From<VariantType> for RemoteVariantType {
    fn from(value: VariantType) -> Self {
        use VariantType as V;

        match value {
            V::Nil => Self::Nil,
            V::Bool => Self::Bool,
            V::Int => Self::Int,
            V::Float => Self::Float,
            V::String => Self::String,
            V::Vector2 => Self::Vector2,
            V::Vector2i => Self::Vector2i,
            V::Rect2 => Self::Rect2,
            V::Rect2i => Self::Vector2i,
            V::Vector3 => Self::Vector3,
            V::Vector3i => Self::Vector3i,
            V::Transform2D => Self::Transform2D,
            V::Vector4 => Self::Vector4,
            V::Vector4i => Self::Vector4i,
            V::Plane => Self::Plane,
            V::Quaternion => Self::Quaternion,
            V::Aabb => Self::Aabb,
            V::Basis => Self::Basis,
            V::Transform3D => Self::Transform3D,
            V::Projection => Self::Projection,
            V::Color => Self::Color,
            V::StringName => Self::StringName,
            V::NodePath => Self::NodePath,
            V::Rid => Self::Rid,
            V::Object => Self::Object,
            V::Callable => Self::Callable,
            V::Signal => Self::Signal,
            V::Dictionary => Self::Dictionary,
            V::Array => Self::Array,
            V::PackedByteArray => Self::PackedByteArray,
            V::PackedInt32Array => Self::PackedInt32Array,
            V::PackedInt64Array => Self::PackedInt64Array,
            V::PackedFloat32Array => Self::PackedFloat32Array,
            V::PackedFloat64Array => Self::PackedInt64Array,
            V::PackedStringArray => Self::PackedStringArray,
            V::PackedVector2Array => Self::PackedVector2Array,
            V::PackedVector3Array => Self::PackedVector3Array,
            V::PackedColorArray => Self::PackedColorArray,
        }
    }
}

#[derive(StableAbi, Debug, Clone)]
#[repr(C)]
#[sabi(unsafe_opaque_fields)]
pub struct RemoteValue(godot::sys::GDExtensionVariantPtr);

impl RemoteValue {
    pub fn new(ptr: godot::sys::GDExtensionVariantPtr) -> Self {
        Self(ptr)
    }
}

impl From<Variant> for RemoteValue {
    fn from(value: Variant) -> Self {
        let ptr = Box::into_raw(Box::new(value)) as godot::sys::GDExtensionVariantPtr;

        Self(ptr)
    }
}

impl From<RemoteValue> for Variant {
    fn from(value: RemoteValue) -> Self {
        unsafe { Self::from_var_sys(value.0) }
    }
}

#[derive(StableAbi, Debug, Clone)]
#[repr(C)]
#[sabi(unsafe_opaque_fields)]
pub struct RemoteValueRef<'a> {
    ptr: godot::sys::GDExtensionVariantPtr,
    lt: PhantomData<&'a ()>,
}

impl<'a> RemoteValueRef<'a> {
    pub fn new(value: &'a Variant) -> Self {
        Self {
            ptr: value.var_sys(),
            lt: PhantomData,
        }
    }

    fn as_ref(&self) -> &'a Variant {
        unsafe { &*(self.ptr as *const Variant) }
    }
}

#[cfg(not(target_os = "windows"))]
fn godot_call_error_type_to_u32(err: godot::sys::GDExtensionCallErrorType) -> u32 {
    err
}

#[cfg(target_os = "windows")]
fn godot_call_error_type_to_u32(err: godot::sys::GDExtensionCallErrorType) -> u32 {
    err as u32
}
