/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::collections::BTreeMap;

use godot::global::{MethodFlags, PropertyHint, PropertyUsageFlags};
use godot::{
    obj::{EngineBitfield, EngineEnum},
    prelude::{Gd, Object},
    sys::VariantType,
};

use crate::script_registry::{
    CreateScriptInstanceData, GodotScriptObject, RustScriptPropertyInfo, RustScriptSignalInfo,
};
pub use crate::script_registry::{
    GodotScript, GodotScriptImpl, RustScriptMetaData, RustScriptMethodInfo,
};
pub use signals::{ScriptSignal, Signal, SignalArguments};

mod signals;

#[macro_export]
macro_rules! register_script_class {
    ($class_name:ty, $base_name:ty, $desc:expr, $props:expr, $signals:expr) => {
        $crate::private_export::plugin_add! {
        SCRIPT_REGISTRY in $crate::private_export;
            $crate::RegistryItem::Entry($crate::RustScriptEntry {
                class_name: concat!(stringify!($class_name), "\0"),
                base_type_name: <$base_name as $crate::godot::prelude::GodotClass>::class_name().as_str(),
                properties: || {
                    $props
                },
                signals: || {
                    $signals
                },
                create_data: $crate::create_default_data_struct::<$class_name>,
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
            $crate::RegistryItem::Methods($crate::RustScriptEntryMethods {
                class_name: concat!(stringify!($class_name), "\0"),
                methods: || {
                    $methods
                },
            })
        }
    };
}

#[macro_export]
macro_rules! setup_library {
    () => {
        #[no_mangle]
        pub fn __godot_rust_script_init() -> ::std::vec::Vec<$crate::RustScriptMetaData> {
            use $crate::godot::obj::EngineEnum;
            use $crate::private_export::*;

            let lock = $crate::private_export::__godot_rust_plugin_SCRIPT_REGISTRY
                .lock()
                .expect("unable to aquire mutex lock");

            $crate::assemble_metadata(lock.iter())
        }

        pub const __GODOT_RUST_SCRIPT_SRC_ROOT: &str = $crate::private_export::concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src",
            $crate::private_export::replace!(
                $crate::private_export::unwrap!($crate::private_export::strip_prefix!(
                    module_path!(),
                    $crate::private_export::replace!(env!("CARGO_PKG_NAME"), "-", "_")
                )),
                "::",
                "/"
            ),
        );
    };
}

pub struct RustScriptEntry {
    pub class_name: &'static str,
    pub base_type_name: &'static str,
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
    pub exported: bool,
    pub hint: PropertyHint,
    pub hint_string: &'static str,
    pub description: &'static str,
}

impl RustScriptPropDesc {
    pub fn to_property_info(&self, class_name: &'static str) -> RustScriptPropertyInfo {
        RustScriptPropertyInfo {
            variant_type: self.ty,
            class_name,
            property_name: self.name,
            usage: if self.exported {
                (PropertyUsageFlags::EDITOR | PropertyUsageFlags::STORAGE).ord()
            } else {
                PropertyUsageFlags::NONE.ord()
            },
            hint: self.hint.ord(),
            hint_string: self.hint_string,
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
    pub fn to_method_info(self, id: i32, class_name: &'static str) -> RustScriptMethodInfo {
        RustScriptMethodInfo {
            id,
            method_name: self.name,
            class_name,
            return_type: self.return_type.to_property_info(class_name),
            flags: self.flags.ord(),
            arguments: self
                .arguments
                .iter()
                .map(|arg| arg.to_property_info(class_name))
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
                .map(|arg| arg.to_property_info("\0"))
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
                .map(|prop| prop.to_property_info(class.class_name))
                .collect();

            let methods = methods
                .get(class.class_name)
                .into_iter()
                .flat_map(|entry| (entry.methods)())
                .enumerate()
                .map(|(index, method)| method.to_method_info((index + 1) as i32, class.class_name))
                .collect();

            let signals = (class.signals)().into_iter().map(Into::into).collect();

            let create_data: Box<dyn CreateScriptInstanceData> = Box::new(class.create_data);
            let description = class.description;

            RustScriptMetaData::new(
                class.class_name,
                class.base_type_name.into(),
                props,
                methods,
                signals,
                create_data,
                description,
            )
        })
        .collect()
}
