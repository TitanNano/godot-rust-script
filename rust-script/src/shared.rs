/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use crate::{script_registry::RustScriptMetaData, RegistryItem};

pub trait RustScriptLibInit: Fn() -> Vec<RustScriptMetaData> {}

impl<F> RustScriptLibInit for F where F: Fn() -> Vec<RustScriptMetaData> {}

godot::sys::plugin_registry!(pub SCRIPT_REGISTRY: RegistryItem);
