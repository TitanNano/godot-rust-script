/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::collections::BTreeMap;

use abi_stable::{
    sabi_trait::TD_Opaque,
    std_types::{RBox, RStr, RString, RVec},
};
use godot::{
    engine::global::{MethodFlags, PropertyHint, PropertyUsageFlags},
    obj::{EngineBitfield, EngineEnum},
    prelude::{Gd, Object},
    sys::VariantType,
};

pub use crate::script_registry::{
    GodotScript, GodotScriptImpl, RemoteScriptMetaData, RemoteScriptMethodInfo,
};
use crate::{
    apply::Apply,
    script_registry::{RemoteGodotScript_TO, RemoteScriptPropertyInfo},
};

#[macro_export]
macro_rules! register_script_class {
    ($class_name:ty, $base_name:ty, $desc:expr, $props:expr) => {
        $crate::private_export::plugin_add! {
        SCRIPT_REGISTRY in $crate::private_export;
            $crate::RegistryItem::Entry($crate::RustScriptEntry {
                class_name: concat!(stringify!($class_name), "\0"),
                base_type_name: <$base_name as $crate::godot::prelude::GodotClass>::class_name().as_str(),
                properties: || {
                    $props
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
        pub fn __godot_rust_script_init(
        ) -> $crate::private_export::RVec<$crate::RemoteScriptMetaData> {
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
    pub create_data: fn(Gd<Object>) -> RemoteGodotScript_TO<'static, RBox<()>>,
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
    pub fn into_property_info(self, class_name: &'static str) -> RemoteScriptPropertyInfo {
        RemoteScriptPropertyInfo {
            variant_type: self.ty.into(),
            class_name: RStr::from_str(class_name),
            property_name: RString::with_capacity(self.name.len()).apply(|s| s.push_str(self.name)),
            usage: if self.exported {
                (PropertyUsageFlags::EDITOR | PropertyUsageFlags::STORAGE).ord()
            } else {
                PropertyUsageFlags::NONE.ord()
            },
            hint: self.hint.ord(),
            hint_string: self.hint_string.into(),
            description: RStr::from_str(self.description),
        }
    }
}

pub struct RustScriptMethodDesc {
    pub name: &'static str,
    pub return_type: RustScriptPropDesc,
    pub arguments: Vec<RustScriptPropDesc>,
    pub flags: MethodFlags,
    pub description: &'static str,
}

impl RustScriptMethodDesc {
    pub fn into_method_info(self, id: i32, class_name: &'static str) -> RemoteScriptMethodInfo {
        RemoteScriptMethodInfo {
            id,
            method_name: self.name.into(),
            class_name: class_name.into(),
            return_type: self.return_type.into_property_info(class_name),
            flags: self.flags.ord(),
            arguments: self
                .arguments
                .into_iter()
                .map(|arg| arg.into_property_info(class_name))
                .collect(),
            description: RStr::from_str(self.description),
        }
    }
}

pub fn create_default_data_struct<T: GodotScript + 'static>(
    base: Gd<Object>,
) -> RemoteGodotScript_TO<'static, RBox<()>> {
    RemoteGodotScript_TO::from_value(T::default_with_base(base), TD_Opaque)
}

pub fn assemble_metadata<'a>(
    items: impl Iterator<Item = &'a RegistryItem> + 'a,
) -> RVec<RemoteScriptMetaData> {
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
                .map(|prop| prop.into_property_info(class.class_name))
                .collect();

            let methods = methods
                .get(class.class_name)
                .into_iter()
                .flat_map(|entry| (entry.methods)())
                .enumerate()
                .map(|(index, method)| {
                    method.into_method_info((index + 1) as i32, class.class_name)
                })
                .collect();

            let create_data = class.create_data;
            let description = class.description;

            RemoteScriptMetaData::new(
                class.class_name.into(),
                class.base_type_name.into(),
                props,
                methods,
                create_data,
                description.into(),
            )
        })
        .collect()
}
