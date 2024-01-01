/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

mod apply;
mod script_registry;
mod shared;

#[cfg(feature = "scripts")]
mod library;
#[cfg(feature = "runtime")]
mod runtime;

#[cfg(feature = "scripts")]
pub use library::*;
#[cfg(feature = "runtime")]
pub use runtime::*;

#[cfg(feature = "scripts")]
pub use godot_rust_script_derive::{godot_script_impl, GodotScript};

pub mod private_export {
    pub use super::script_registry::RemoteVariantType;
    pub use abi_stable::std_types::{RStr, RString, RVec};
    pub use godot::sys::{plugin_add, plugin_registry};
}

pub use godot;
