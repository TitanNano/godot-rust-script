/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

#![doc = include_str!("../../README.md")]

mod apply;

mod editor_ui_hacks;
mod interface;
mod runtime;
mod static_script_registry;

pub use godot_rust_script_derive::{godot_script_impl, GodotScriptEnum};

/// Use this derive macro to create new rust scripts for your projects.
///
/// The macro is desinged to closely align with both godot-rusts [`GodotClass`](https://docs.rs/godot/latest/godot/prelude/derive.GodotClass.html) macro and the GDScript
/// annotations.
///
/// # Top Level Attribute
/// On the struct level the `#[script]` attribute can be used to configure base details of the script.
///
/// ## `#[script(base)]`
/// ```
/// # use godot_rust_script::{GodotScript, godot_script_impl};
/// # use godot::classes::Node3D;
/// #
/// #[derive(GodotScript, Debug)]
/// #[script(base = Node3D)]
/// struct MyScript {}
///
/// # #[godot_script_impl]
/// # impl MyScript {}
/// ```
///
/// Set the `base` field to specify a base class your script should inherit from. By default all scripts inherit from [`RefCounted`](https://docs.rs/godot/latest/godot/classes/struct.RefCounted.html).
///
/// # Field Level Attributes
/// On the field level you can specify customizations for your script properties. Fields that are private will not be exposed to the engine.
/// Public field on the other hand are exposed to the engine and can be annotated with attributes.
///
/// ## `#[prop]`
/// Use the `#[prop]` attribute to set up getter and setter functions for your properties.
///
/// ```
/// # use godot_rust_script::{GodotScript, godot_script_impl};
/// # use godot::builtin::GString;
/// #
/// #[derive(GodotScript, Debug)]
/// struct MyScript {
///     #[prop(set = Self::set_my_prop, get = Self::get_my_prop)]
///     my_prop: GString,
/// }
///
/// #[godot_script_impl]
/// impl MyScript {
///     fn set_my_prop(&mut self, value: GString) {
///         self.my_prop = value;
///     }
///
///     fn get_my_prop(&self) -> GString {
///         self.my_prop.clone()
///     }
/// }
/// ```
///
/// This attribute optionally accepts a `get` and a `set` field. If these fields are defined they have to be set to a function pointer
/// expression. The expression can contain the `Self` keyword.
pub use godot_rust_script_derive::GodotScript;
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
