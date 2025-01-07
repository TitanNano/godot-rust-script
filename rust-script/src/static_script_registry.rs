/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use std::sync::{Arc, LazyLock, RwLock};

use godot::builtin::{GString, StringName};
use godot::global::{MethodFlags, PropertyHint, PropertyUsageFlags};
use godot::meta::{ClassName, MethodInfo, PropertyHintInfo, PropertyInfo, ToGodot};
use godot::obj::{EngineBitfield, EngineEnum};
use godot::prelude::{Gd, Object};
use godot::sys::VariantType;

use crate::interface::GodotScript;
use crate::runtime::GodotScriptObject;

godot::sys::plugin_registry!(pub SCRIPT_REGISTRY: RegistryItem);

#[macro_export]
macro_rules! register_script_class {
    ($class_name:ty, $base_name:ty, $desc:expr, $props:expr, $signals:expr) => {
        $crate::private_export::plugin_add! {
        SCRIPT_REGISTRY in $crate::private_export;
            $crate::private_export::RegistryItem::Entry($crate::private_export::RustScriptEntry {
                class_name: stringify!($class_name),
                class_name_cstr: ::std::ffi::CStr::from_bytes_with_nul(concat!(stringify!($class_name), "\0").as_bytes()).unwrap(),
                base_type_name: <$base_name as $crate::godot::prelude::GodotClass>::class_name().to_cow_str(),
                properties: || {
                    $props
                },
                signals: || {
                    $signals
                },
                create_data: $crate::private_export::create_default_data_struct::<$class_name>,
                description: $desc,
            })
        }
    };
}

#[macro_export]
macro_rules! register_script_methods {
    ($class_name:ty, $methods:expr) => {
        $crate::private_export::plugin_add! {
            SCRIPT_REGISTRY in $crate::private_export;
            $crate::private_export::RegistryItem::Methods($crate::private_export::RustScriptEntryMethods {
                class_name: stringify!($class_name),
                methods: || {
                    $methods
                },
            })
        }
    };
}

pub struct RustScriptEntry {
    pub class_name: &'static str,
    #[cfg(before_api = "4.4")]
    pub class_name_cstr: &'static std::ffi::CStr,
    pub base_type_name: Cow<'static, str>,
    pub properties: fn() -> Vec<RustScriptPropDesc>,
    pub signals: fn() -> Vec<RustScriptSignalDesc>,
    pub create_data: fn(Gd<Object>) -> Box<dyn GodotScriptObject>,
    pub description: &'static str,
}

#[derive(Debug)]
pub struct RustScriptEntryMethods {
    pub class_name: &'static str,
    pub methods: fn() -> Vec<RustScriptMethodDesc>,
}

pub enum RegistryItem {
    Entry(RustScriptEntry),
    Methods(RustScriptEntryMethods),
}

#[derive(Debug)]
pub struct RustScriptPropDesc {
    pub name: &'static str,
    pub ty: VariantType,
    pub class_name: ClassName,
    pub exported: bool,
    pub hint: PropertyHint,
    pub hint_string: String,
    pub description: &'static str,
}

impl RustScriptPropDesc {
    pub fn to_property_info(&self) -> RustScriptPropertyInfo {
        RustScriptPropertyInfo {
            variant_type: self.ty,
            class_name: self.class_name,
            property_name: self.name,
            usage: if self.exported {
                (PropertyUsageFlags::EDITOR | PropertyUsageFlags::STORAGE).ord()
            } else {
                PropertyUsageFlags::NONE.ord()
            },
            hint: self.hint.ord(),
            hint_string: self.hint_string.clone(),
            description: self.description,
        }
    }
}

pub struct RustScriptMethodDesc {
    pub name: &'static str,
    pub return_type: RustScriptPropDesc,
    pub arguments: Box<[RustScriptPropDesc]>,
    pub flags: MethodFlags,
    pub description: &'static str,
}

impl RustScriptMethodDesc {
    pub fn into_method_info(
        self,
        id: i32,
        class_name: &'static str,
        #[cfg(before_api = "4.4")] class_name_cstr: &'static std::ffi::CStr,
    ) -> RustScriptMethodInfo {
        RustScriptMethodInfo {
            id,
            method_name: self.name,
            class_name,
            #[cfg(before_api = "4.4")]
            class_name_cstr,
            return_type: self.return_type.to_property_info(),
            flags: self.flags.ord(),
            arguments: self
                .arguments
                .iter()
                .map(|arg| arg.to_property_info())
                .collect(),
            description: self.description,
        }
    }
}

pub struct RustScriptSignalDesc {
    pub name: &'static str,
    pub arguments: Box<[RustScriptPropDesc]>,
    pub description: &'static str,
}

impl From<RustScriptSignalDesc> for RustScriptSignalInfo {
    fn from(value: RustScriptSignalDesc) -> Self {
        Self {
            name: value.name,
            arguments: value
                .arguments
                .iter()
                .map(|arg| arg.to_property_info())
                .collect(),
            description: value.description,
        }
    }
}

pub fn create_default_data_struct<T: GodotScript + GodotScriptObject + 'static>(
    base: Gd<Object>,
) -> Box<dyn GodotScriptObject> {
    Box::new(T::default_with_base(base))
}

pub fn assemble_metadata<'a>(
    items: impl Iterator<Item = &'a RegistryItem> + 'a,
) -> Vec<RustScriptMetaData> {
    let (entries, methods): (Vec<_>, Vec<_>) = items
        .map(|item| match item {
            RegistryItem::Entry(entry) => (Some(entry), None),
            RegistryItem::Methods(methods) => (None, Some((methods.class_name, methods))),
        })
        .unzip();

    let methods: BTreeMap<_, _> = methods.into_iter().flatten().collect();

    entries
        .into_iter()
        .flatten()
        .map(|class| {
            let props = (class.properties)()
                .into_iter()
                .map(|prop| prop.to_property_info())
                .collect();

            let methods = methods
                .get(class.class_name)
                .into_iter()
                .flat_map(|entry| (entry.methods)())
                .enumerate()
                .map(|(index, method)| {
                    method.into_method_info(
                        (index + 1) as i32,
                        class.class_name,
                        #[cfg(before_api = "4.4")]
                        class.class_name_cstr,
                    )
                })
                .collect();

            let signals = (class.signals)().into_iter().map(Into::into).collect();

            let create_data: Box<dyn CreateScriptInstanceData> = Box::new(class.create_data);
            let description = class.description;

            RustScriptMetaData::new(
                class.class_name,
                #[cfg(before_api = "4.4")]
                class.class_name_cstr,
                class.base_type_name.as_ref().into(),
                props,
                methods,
                signals,
                create_data,
                description,
            )
        })
        .collect()
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct RustScriptPropertyInfo {
    pub variant_type: VariantType,
    pub property_name: &'static str,
    pub class_name: ClassName,
    pub hint: i32,
    pub hint_string: String,
    pub usage: u64,
    pub description: &'static str,
}

impl From<&RustScriptPropertyInfo> for PropertyInfo {
    fn from(value: &RustScriptPropertyInfo) -> Self {
        Self {
            variant_type: value.variant_type,
            property_name: value.property_name.into(),
            class_name: value.class_name,
            hint_info: PropertyHintInfo {
                hint: PropertyHint::try_from_ord(value.hint).unwrap_or(PropertyHint::NONE),
                hint_string: value.hint_string.to_godot(),
            },
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
    #[cfg(before_api = "4.4")]
    pub class_name_cstr: &'static std::ffi::CStr,
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
            class_name: ClassName::new_script(
                value.class_name,
                #[cfg(before_api = "4.4")]
                value.class_name_cstr,
            ),
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
                hint_info: PropertyHintInfo {
                    hint: PropertyHint::NONE,
                    hint_string: GString::default(),
                },
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
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        class_name: &'static str,
        #[cfg(before_api = "4.4")] class_name_cstr: &'static std::ffi::CStr,
        base_type_name: StringName,
        properties: Box<[RustScriptPropertyInfo]>,
        methods: Box<[RustScriptMethodInfo]>,
        signals: Box<[RustScriptSignalInfo]>,
        create_data: Box<dyn CreateScriptInstanceData>,
        description: &'static str,
    ) -> Self {
        Self {
            #[cfg(before_api = "4.4")]
            class_name: ClassName::new_script(class_name, class_name_cstr),

            #[cfg(since_api = "4.4")]
            class_name: ClassName::new_script(class_name),
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

static DYNAMIC_INDEX_BY_CLASS_NAME: LazyLock<RwLock<HashMap<&'static str, ClassName>>> =
    LazyLock::new(RwLock::default);

trait ClassNameExtension {
    #[cfg(before_api = "4.4")]
    fn new_script(str: &'static str, cstr: &'static std::ffi::CStr) -> Self;

    #[cfg(since_api = "4.4")]
    fn new_script(str: &'static str) -> Self;
}

impl ClassNameExtension for ClassName {
    #[cfg(before_api = "4.4")]
    fn new_script(str: &'static str, cstr: &'static std::ffi::CStr) -> Self {
        // Check if class name exists.
        if let Some(name) = DYNAMIC_INDEX_BY_CLASS_NAME.read().unwrap().get(str) {
            return *name;
        }

        let mut map = DYNAMIC_INDEX_BY_CLASS_NAME.write().unwrap();

        let class_name = map
            .entry(str)
            .or_insert_with(|| ClassName::alloc_next_ascii(cstr));

        *class_name
    }

    #[cfg(since_api = "4.4")]
    fn new_script(str: &'static str) -> Self {
        // Check if class name exists.

        if let Some(name) = DYNAMIC_INDEX_BY_CLASS_NAME.read().unwrap().get(str) {
            return *name;
        }

        let mut map = DYNAMIC_INDEX_BY_CLASS_NAME.write().unwrap();

        let class_name = *map
            .entry(str)
            .or_insert_with(|| ClassName::alloc_next_unicode(str));

        class_name
    }
}
