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
use godot::meta::{ClassId, MethodInfo, PropertyHintInfo, PropertyInfo, ToGodot};
use godot::prelude::{Gd, Object};
use godot::sys::VariantType;

use crate::interface::GodotScript;
use crate::runtime::GodotScriptObject;

godot::sys::plugin_registry!(pub SCRIPT_REGISTRY: RegistryItem);

#[macro_export]
macro_rules! register_script_class {
    ($class_name:ty, $base_name:ty, $desc:expr, $props:expr, $signals:expr) => {
        $crate::private_export::plugin_add! {
            $crate::private_export::SCRIPT_REGISTRY;
            $crate::private_export::RegistryItem::Entry($crate::private_export::RustScriptEntry {
                class_name: stringify!($class_name),
                class_name_cstr: ::std::ffi::CStr::from_bytes_with_nul(concat!(stringify!($class_name), "\0").as_bytes()).unwrap(),
                base_type_name: <$base_name as $crate::godot::prelude::GodotClass>::class_id().to_cow_str(),
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
    ($class_name:ty, $method_capacity:literal, $builder:ident => $methods:tt) => {
        $crate::private_export::plugin_add! {
            $crate::private_export::SCRIPT_REGISTRY ;
            $crate::private_export::RegistryItem::Methods(|| {
                let mut $builder = $crate::private_export::RustScriptEntryMethods::builder(stringify!($class_name), $method_capacity);

                $methods

                $builder.build()
            })
        }
    };
}

pub struct RustScriptEntry {
    pub class_name: &'static str,
    pub class_name_cstr: &'static std::ffi::CStr,
    pub base_type_name: Cow<'static, str>,
    pub properties: fn() -> Vec<RustScriptPropDesc>,
    pub signals: fn() -> Vec<RustScriptSignalDesc>,
    pub create_data: fn(Gd<Object>) -> Box<dyn GodotScriptObject>,
    pub description: &'static str,
}

#[derive(Debug)]
pub struct RustScriptEntryMethods {
    class_name: &'static str,
    methods: Box<[RustScriptMethodDesc]>,
}

impl RustScriptEntryMethods {
    pub fn builder(class_name: &'static str, capacity: usize) -> RustScriptEntryMethodsBuilder {
        RustScriptEntryMethodsBuilder {
            class_name,
            methods: Vec::with_capacity(capacity),
        }
    }
}

pub struct RustScriptEntryMethodsBuilder {
    class_name: &'static str,
    methods: Vec<RustScriptMethodDesc>,
}

impl RustScriptEntryMethodsBuilder {
    pub fn add_method(&mut self, method: RustScriptMethodDescBuilder) {
        let index = self.methods.len();

        self.methods
            .push(method.build(index as u32, self.class_name));
    }

    pub fn build(self) -> RustScriptEntryMethods {
        RustScriptEntryMethods {
            class_name: self.class_name,
            methods: self.methods.into(),
        }
    }
}

pub enum RegistryItem {
    Entry(RustScriptEntry),
    Methods(fn() -> RustScriptEntryMethods),
}

#[derive(Debug, Clone)]
pub struct RustScriptPropDesc {
    pub name: &'static str,
    pub ty: VariantType,
    pub class_name: ClassId,
    pub usage: PropertyUsageFlags,
    pub hint: PropertyHint,
    pub hint_string: String,
    pub description: &'static str,
}

#[derive(Debug, Clone)]
pub struct RustScriptMethodDesc {
    pub(crate) id: u32,
    pub(crate) class_name: &'static str,
    pub(crate) name: &'static str,
    pub(crate) return_type: RustScriptPropDesc,
    pub(crate) arguments: Box<[RustScriptPropDesc]>,
    pub(crate) flags: MethodFlags,
    pub(crate) description: &'static str,
}

impl RustScriptMethodDesc {
    pub fn builder(
        name: &'static str,
        arguments: Box<[RustScriptPropDesc]>,
        return_type: RustScriptPropDesc,
    ) -> RustScriptMethodDescBuilder {
        RustScriptMethodDescBuilder {
            name,
            return_type,
            arguments,
            flags: MethodFlags::NORMAL,
            description: Default::default(),
        }
    }
}

pub struct RustScriptMethodDescBuilder {
    name: &'static str,
    return_type: RustScriptPropDesc,
    arguments: Box<[RustScriptPropDesc]>,
    flags: MethodFlags,
    description: &'static str,
}

impl RustScriptMethodDescBuilder {
    pub fn with_flags(mut self, flags: MethodFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn with_description(mut self, description: &'static str) -> Self {
        self.description = description;
        self
    }

    pub fn build(self, id: u32, class_name: &'static str) -> RustScriptMethodDesc {
        RustScriptMethodDesc {
            id,
            class_name,
            name: self.name,
            return_type: self.return_type,
            arguments: self.arguments,
            flags: self.flags,
            description: self.description,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RustScriptSignalDesc {
    pub name: &'static str,
    pub arguments: Box<[RustScriptPropDesc]>,
    pub description: &'static str,
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
            RegistryItem::Methods(methods) => {
                let methods = methods();

                (None, Some((methods.class_name, methods)))
            }
        })
        .unzip();

    let methods: BTreeMap<_, _> = methods.into_iter().flatten().collect();

    entries
        .into_iter()
        .flatten()
        .map(|class| {
            let props = (class.properties)().into();

            let methods = methods
                .get(class.class_name)
                .into_iter()
                .flat_map(|entry| entry.methods.clone())
                .collect();

            let signals = (class.signals)().into();

            let create_data: Box<dyn CreateScriptInstanceData> = Box::new(class.create_data);
            let description = class.description;

            RustScriptMetaData::new(
                class.class_name,
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

impl From<&RustScriptPropDesc> for PropertyInfo {
    fn from(value: &RustScriptPropDesc) -> Self {
        Self {
            variant_type: value.ty,
            property_name: value.name.into(),
            class_id: value.class_name,
            hint_info: PropertyHintInfo {
                hint: value.hint,
                hint_string: value.hint_string.to_godot(),
            },
            usage: value.usage,
        }
    }
}

impl From<&RustScriptSignalDesc> for MethodInfo {
    fn from(value: &RustScriptSignalDesc) -> Self {
        Self {
            id: 0,
            method_name: value.name.into(),
            class_name: ClassId::none(),
            return_type: PropertyInfo {
                variant_type: VariantType::NIL,
                class_id: ClassId::none(),
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

impl From<RustScriptMethodDesc> for MethodInfo {
    fn from(value: RustScriptMethodDesc) -> Self {
        Self {
            id: value
                .id
                .try_into()
                .expect("method index should fit into i32"),
            method_name: value.name.into(),
            class_name: get_class_id(value.class_name),
            return_type: (&value.return_type).into(),
            flags: value.flags,
            arguments: value.arguments.iter().map(|arg| arg.into()).collect(),
            default_arguments: Vec::with_capacity(0),
        }
    }
}

#[derive(Debug)]
pub struct RustScriptMetaData {
    pub(crate) class_name: ClassId,
    pub(crate) base_type_name: StringName,
    pub(crate) properties: Box<[RustScriptPropDesc]>,
    pub(crate) methods: Box<[RustScriptMethodDesc]>,
    pub(crate) signals: Box<[RustScriptSignalDesc]>,
    pub(crate) create_data: Arc<dyn CreateScriptInstanceData>,
    pub(crate) description: &'static str,
}

impl RustScriptMetaData {
    pub fn new(
        class_name: &'static str,
        base_type_name: StringName,
        properties: Box<[RustScriptPropDesc]>,
        methods: Box<[RustScriptMethodDesc]>,
        signals: Box<[RustScriptSignalDesc]>,
        create_data: Box<dyn CreateScriptInstanceData>,
        description: &'static str,
    ) -> Self {
        Self {
            class_name: get_class_id(class_name),

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
    pub fn class_name(&self) -> ClassId {
        self.class_name
    }

    pub fn base_type_name(&self) -> StringName {
        self.base_type_name.clone()
    }

    pub fn create_data(&self, base: Gd<Object>) -> Box<dyn GodotScriptObject> {
        self.create_data.create(base)
    }

    pub fn properties(&self) -> &[RustScriptPropDesc] {
        &self.properties
    }

    pub fn methods(&self) -> &[RustScriptMethodDesc] {
        &self.methods
    }

    pub fn signals(&self) -> &[RustScriptSignalDesc] {
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

fn get_class_id(class_name: &'static str) -> ClassId {
    static SCRIPT_CLASS_NAMES: LazyLock<RwLock<HashMap<&'static str, ClassId>>> =
        LazyLock::new(|| RwLock::new(HashMap::new()));

    if let Some(class_id) = SCRIPT_CLASS_NAMES.read().unwrap().get(class_name) {
        return *class_id;
    }

    *SCRIPT_CLASS_NAMES
        .write()
        .unwrap()
        .entry(class_name)
        .or_insert_with(|| ClassId::__alloc_next_unicode(class_name))
}

#[cfg(test)]
mod tests {
    use godot::global::PropertyHint;
    use godot::global::PropertyUsageFlags;
    use godot::{meta::ClassId, sys::VariantType};

    use crate::{
        private_export::{RustScriptEntryMethods, RustScriptMethodDesc},
        static_script_registry::get_class_id,
    };

    #[test]
    fn new_class_name() {
        let script_name = ClassId::__alloc_next_unicode("TestScript");

        assert_eq!(script_name.to_cow_str(), "TestScript");
    }

    #[cfg(since_api = "4.4")]
    #[test]
    fn new_unicode_class_name() {
        let script_name = ClassId::__alloc_next_unicode("ÜbertragungsScript");

        assert_eq!(script_name.to_cow_str(), "ÜbertragungsScript");
    }

    #[test]
    fn cached_class_id() {
        let script_name_a = get_class_id("CachedTestScript");
        let script_name_b = get_class_id("CachedTestScript");

        assert_eq!(script_name_a, script_name_b);
    }

    #[test]
    fn build_method_list() {
        let mut builder = RustScriptEntryMethods::builder("TestClass", 4);

        builder.add_method(RustScriptMethodDesc::builder(
            "add_member",
            Box::new([]),
            super::RustScriptPropDesc {
                name: "",
                ty: VariantType::BOOL,
                class_name: get_class_id("Node"),
                usage: PropertyUsageFlags::NONE,
                hint: PropertyHint::NONE,
                hint_string: String::from(""),
                description: "",
            },
        ));

        let entry = builder.build();

        assert_eq!(entry.methods[0].class_name, "TestClass");
        assert_eq!(entry.methods[0].name, "add_member");
        assert_eq!(entry.methods[0].return_type.ty, VariantType::BOOL);
    }
}
