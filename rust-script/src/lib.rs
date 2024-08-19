/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

mod apply;

mod interface;
mod runtime;
mod static_script_registry;

pub use godot_rust_script_derive::{godot_script_impl, GodotScript};
pub use interface::*;
pub use runtime::RustScriptExtensionLayer;

#[doc(hidden)]
pub mod private_export {
    pub use crate::static_script_registry::{
        RustScriptMetaData, __godot_rust_plugin_SCRIPT_REGISTRY, assemble_metadata,
        create_default_data_struct, RegistryItem, RustScriptEntry, RustScriptEntryMethods,
        RustScriptMethodDesc, RustScriptPropDesc, RustScriptSignalDesc,
    };
    pub use const_str::{concat, replace, strip_prefix, unwrap};
    pub use godot::sys::{plugin_add, plugin_registry};
}

pub use godot;
