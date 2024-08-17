/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::{any::Any, collections::HashMap, fmt::Debug, sync::Arc};

use godot::global::{MethodFlags, PropertyHint, PropertyUsageFlags};
use godot::meta::{ClassName, MethodInfo, PropertyInfo};
use godot::obj::{EngineBitfield, EngineEnum, Inherits};
use godot::prelude::{GString, Gd, Object, StringName, Variant};
use godot::sys::VariantType;

use crate::runtime::{Context, GenericContext};

pub trait GodotScript: Debug + GodotScriptImpl<ImplBase = Self::Base> {
    type Base: Inherits<Object>;

    const CLASS_NAME: &'static str;

    fn set(&mut self, name: StringName, value: Variant) -> bool;
    fn get(&self, name: StringName) -> Option<Variant>;
    fn call(
        &mut self,
        method: StringName,
        args: &[&Variant],
        context: Context<'_, Self>,
    ) -> Result<Variant, godot::sys::GDExtensionCallErrorType>;

    fn to_string(&self) -> String;
    fn property_state(&self) -> HashMap<StringName, Variant>;

    fn default_with_base(base: godot::prelude::Gd<godot::prelude::Object>) -> Self;
}

pub trait GodotScriptImpl {
    type ImplBase: Inherits<Object>;

    fn call_fn(
        &mut self,
        name: StringName,
        args: &[&Variant],
        context: Context<Self>,
    ) -> Result<Variant, godot::sys::GDExtensionCallErrorType>;
}

pub trait GodotScriptObject {
    fn set(&mut self, name: StringName, value: Variant) -> bool;
    fn get(&self, name: StringName) -> Option<Variant>;
    fn call(
        &mut self,
        method: StringName,
        args: &[&Variant],
        context: GenericContext,
    ) -> Result<Variant, godot::sys::GDExtensionCallErrorType>;
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
    ) -> Result<Variant, godot::sys::GDExtensionCallErrorType> {
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
                variant_type: VariantType::NIL,
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

#[derive(Debug)]
pub struct RsRef<T: GodotScript> {
    owner: Gd<T::Base>,
    script_ty: PhantomData<T>,
}

impl<T: GodotScript> RsRef<T> {
    pub(crate) fn new<B: Inherits<T::Base> + Inherits<Object>>(owner: Gd<B>) -> Self {
        Self {
            owner: owner.upcast(),
            script_ty: PhantomData,
        }
    }

    fn validate_script<O: Inherits<Object>>(owner: &Gd<O>) -> Option<GodotScriptCastError> {
        let script = owner
            .upcast_ref::<Object>()
            .get_script()
            .try_to::<Option<Gd<crate::runtime::RustScript>>>();

        let Ok(script) = script else {
            return Some(GodotScriptCastError::NotRustScript);
        };

        let Some(script) = script else {
            return Some(GodotScriptCastError::NoScriptAttached);
        };

        let class_name = script.bind().str_class_name();

        (class_name != T::CLASS_NAME).then(|| {
            GodotScriptCastError::ClassMismatch(T::CLASS_NAME, script.get_class().to_string())
        })
    }
}

impl<T: GodotScript> Deref for RsRef<T> {
    type Target = Gd<T::Base>;

    fn deref(&self) -> &Self::Target {
        &self.owner
    }
}

impl<T: GodotScript> DerefMut for RsRef<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.owner
    }
}

impl<T: GodotScript> Clone for RsRef<T> {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            script_ty: PhantomData,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum GodotScriptCastError {
    #[error("Object has no script attached!")]
    NoScriptAttached,

    #[error("Script attached to object is not a RustScript!")]
    NotRustScript,

    #[error(
        "Script attached to object does not match expected script class `{0}` but found `{1}`!"
    )]
    ClassMismatch(&'static str, String),
}

pub trait CastToScript<T: GodotScript> {
    fn try_to_script(&self) -> Result<RsRef<T>, GodotScriptCastError>;
    fn try_into_script(self) -> Result<RsRef<T>, GodotScriptCastError>;
    fn to_script(&self) -> RsRef<T>;
    fn into_script(self) -> RsRef<T>;
}

impl<T: GodotScript, B: Inherits<T::Base> + Inherits<Object>> CastToScript<T> for Gd<B> {
    fn try_to_script(&self) -> Result<RsRef<T>, GodotScriptCastError> {
        if let Some(err) = RsRef::<T>::validate_script(self) {
            return Err(err);
        }

        Ok(RsRef::new(self.clone()))
    }

    fn try_into_script(self) -> Result<RsRef<T>, GodotScriptCastError> {
        if let Some(err) = RsRef::<T>::validate_script(&self) {
            return Err(err);
        }

        Ok(RsRef::new(self))
    }

    fn to_script(&self) -> RsRef<T> {
        self.try_to_script().unwrap_or_else(|err| {
            panic!(
                "`{}` was assumed to have rust script `{}`, but this was not the case at runtime!\nError: {}",
                B::class_name(),
                T::CLASS_NAME,
                err,
            );
        })
    }

    fn into_script(self) -> RsRef<T> {
        self.try_into_script().unwrap_or_else(|err| {
            panic!(
                "`{}` was assumed to have rust script `{}`, but this was not the case at runtime!\nError: {}",
                B::class_name(),
                T::CLASS_NAME,
                err
            );
        })
    }
}
