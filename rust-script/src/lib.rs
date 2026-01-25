/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

mod apply;

mod editor_ui_hacks;
mod interface;
mod runtime;
mod static_script_registry;

pub use godot_rust_script_derive::{
    GodotScript, GodotScriptEnum, ScriptExportGroup, ScriptExportSubgroup, godot_script_impl,
};
pub use interface::*;
pub use runtime::RustScriptExtensionLayer;

#[doc(hidden)]
pub mod private_export {
    pub use crate::static_script_registry::{
        RegistryItem, RustScriptEntry, RustScriptEntryMethods, RustScriptMetaData,
        RustScriptMethodDesc, RustScriptPropDesc, RustScriptSignalDesc, SCRIPT_REGISTRY,
        assemble_metadata, create_default_data_struct,
    };
    pub use const_str::{concat, replace, strip_prefix, unwrap};
    pub use godot::sys::{plugin_add, plugin_registry};
}

pub use godot;
