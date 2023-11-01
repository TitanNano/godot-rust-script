use std::collections::BTreeMap;

use abi_stable::{
    sabi_trait::TD_Opaque,
    std_types::{RBox, RStr, RString, RVec},
};
use godot::{
    engine::global::{MethodFlags, PropertyHint, PropertyUsageFlags},
    obj::EngineEnum,
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
    ($class_name:ty, $base_name:ty, $props:expr) => {
        $crate::private_export::plugin_add! {
            __SCRIPT_REGISTRY in crate;
            $crate::RegistryItem::Entry($crate::RustScriptEntry {
                class_name: concat!(stringify!($class_name), "\0"),
                base_type_name: <$base_name as $crate::godot::prelude::GodotClass>::class_name().as_str(),
                properties: || {
                    $props
                },
                create_data: $crate::create_default_data_struct::<$class_name>,
            })
        }
    };
}

#[macro_export]
macro_rules! register_script_methods {
    ($class_name:ty, $methods:expr) => {
        $crate::private_export::plugin_add! {
            __SCRIPT_REGISTRY in crate;
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
        $crate::private_export::plugin_registry!(pub __SCRIPT_REGISTRY: $crate::RegistryItem);

        #[no_mangle]
        pub fn __godot_rust_script_init(binding: Option<$crate::BindingInit>) -> $crate::private_export::RVec<$crate::RemoteScriptMetaData> {
            use $crate::private_export::*;
            use $crate::godot::obj::EngineEnum;

            if let Some(init) = binding {
                let config = $crate::godot::sys::GdextConfig {
                    tool_only_in_editor: false,
                    is_editor: ::std::cell::OnceCell::new(),
                };

                unsafe {
                    $crate::godot::sys::init_with_existing_binding(init);
                }
            }

            let lock = __godot_rust_plugin___SCRIPT_REGISTRY.lock().expect("unable to aquire mutex lock");

            $crate::assemble_metadata(lock.iter())
        }

        pub const __GODOT_RUST_SCRIPT_SRC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    };
}

pub struct RustScriptEntry {
    pub class_name: &'static str,
    pub base_type_name: &'static str,
    pub properties: fn() -> Vec<RustScriptPropDesc>,
    pub create_data: fn(Gd<Object>) -> RemoteGodotScript_TO<'static, RBox<()>>,
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

pub struct RustScriptPropDesc {
    pub name: &'static str,
    pub ty: VariantType,
    pub exported: bool,
    pub hint: PropertyHint,
    pub hint_string: &'static str,
}

impl RustScriptPropDesc {
    pub fn into_property_info(self, class_name: &'static str) -> RemoteScriptPropertyInfo {
        RemoteScriptPropertyInfo {
            variant_type: self.ty.into(),
            class_name: RStr::from_str(class_name),
            property_name: RString::with_capacity(self.name.len()).apply(|s| s.push_str(self.name)),
            usage: if self.exported {
                (PropertyUsageFlags::PROPERTY_USAGE_EDITOR
                    | PropertyUsageFlags::PROPERTY_USAGE_STORAGE)
                    .ord()
            } else {
                PropertyUsageFlags::PROPERTY_USAGE_NONE.ord()
            },
            hint: self.hint.ord(),
            hint_string: self.hint_string.into(),
        }
    }
}

pub struct RustScriptMethodDesc {
    pub name: &'static str,
    pub return_type: RustScriptPropDesc,
    pub arguments: Vec<RustScriptPropDesc>,
    pub flags: MethodFlags,
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

            RemoteScriptMetaData::new(
                class.class_name.into(),
                class.base_type_name.into(),
                props,
                methods,
                create_data,
            )
        })
        .collect()
}
