/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use abi_stable::std_types::RVec;

use crate::{script_registry::RemoteScriptMetaData, RegistryItem};

pub trait RustScriptLibInit: Fn() -> RVec<RemoteScriptMetaData> {}

impl<F> RustScriptLibInit for F where F: Fn() -> RVec<RemoteScriptMetaData> {}

godot::sys::plugin_registry!(pub SCRIPT_REGISTRY: RegistryItem);
