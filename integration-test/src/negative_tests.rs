/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

/// Subgroups inside subgroups are not allowed:
/// ```compile_fail
/// use godot_rust_script::{ScriptExportGroup, ScriptExportSubgroup};
///
/// #[derive(ScriptExportGroup, Default, Debug)]
/// struct PropertyGroup {
///   #[export(flatten)]
///   subgroup: PropertySubgroup,
/// }
///
/// #[derive(ScriptExportSubgroup, Default, Debug)]
/// struct PropertySubgroup {
///   #[export(flatten)]
///   deeper: PropertySubgroup2,
/// }
///
/// #[derive(ScriptExportSubgroup, Default, Debug)]
/// struct PropertySubgroup2;
/// ````
#[allow(dead_code)]
pub struct InternalDocTests;