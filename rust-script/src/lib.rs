mod apply;
mod library;
mod runtime;
mod script_registry;

pub use library::*;
pub use runtime::*;

pub use godot_rust_script_derive::{godot_script_impl, GodotScript};

pub mod private_export {
    pub use super::script_registry::RemoteVariantType;
    pub use abi_stable::std_types::{RStr, RString, RVec};
    pub use godot::sys::{plugin_add, plugin_registry};
    pub use hot_lib_reloader::{self, hot_module};
}

pub use godot;
