/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::collections::HashMap;

use convert_case::{Case, Casing};
use godot::builtin::{StringName, VariantType};
use godot::global::{PropertyHint, PropertyUsageFlags};
use godot::meta::ClassId;

use crate::private_export::RustScriptPropDesc;

enum PropertyGroupItem {
    Prop(RustScriptPropDesc),
    Sub {
        name: &'static str,
        description: &'static str,
        builder: ExportSubgroupBuilder,
    },
}

/// Build metadata for script property groups.
///
/// The builder allows assembling a group of script properties in multiple steps.
pub struct ExportGroupBuilder {
    properties: Vec<PropertyGroupItem>,
}

impl ExportGroupBuilder {
    pub fn new(capacity: usize) -> Self {
        Self {
            properties: Vec::with_capacity(capacity),
        }
    }

    pub fn add_property(mut self, property_desc: RustScriptPropDesc) -> Self {
        self.properties.push(PropertyGroupItem::Prop(property_desc));
        self
    }

    pub fn add_subgroup(
        mut self,
        name: &'static str,
        description: &'static str,
        subgroup: ExportSubgroupBuilder,
    ) -> Self {
        self.properties.push(PropertyGroupItem::Sub {
            name,
            description,
            builder: subgroup,
        });
        self
    }

    pub fn build(
        self,
        prefix: &'static str,
        description: &'static str,
    ) -> Box<[RustScriptPropDesc]> {
        [RustScriptPropDesc {
            name: prefix.to_case(Case::Title).into(),
            ty: VariantType::NIL,
            class_name: ClassId::none(),
            usage: PropertyUsageFlags::GROUP,
            hint: PropertyHint::NONE,
            hint_string: prefix.into(),
            description,
        }]
        .into_iter()
        .chain(self.properties.into_iter().flat_map(|item| match item {
            PropertyGroupItem::Prop(mut prop_desc) => {
                prop_desc.name = format!("{prefix}_{}", prop_desc.name).into();
                vec![prop_desc].into_iter()
            }
            PropertyGroupItem::Sub {
                name,
                description,
                builder,
            } => builder.build(prefix, name, description).into_iter(),
        }))
        .collect()
    }
}

/// Build metadata for script property subgroups.
///
/// The builder allows assembling a subgroup of script properties in multiple steps.
pub struct ExportSubgroupBuilder {
    properties: Vec<RustScriptPropDesc>,
}

impl ExportSubgroupBuilder {
    pub fn new(capacity: usize) -> Self {
        Self {
            properties: Vec::with_capacity(capacity),
        }
    }

    pub fn add_property(mut self, property_desc: RustScriptPropDesc) -> Self {
        self.properties.push(property_desc);
        self
    }

    pub fn build(
        self,
        parent_prefix: &str,
        prefix: &str,
        description: &'static str,
    ) -> Box<[RustScriptPropDesc]> {
        [RustScriptPropDesc {
            name: prefix.to_case(Case::Title).into(),
            ty: VariantType::NIL,
            class_name: ClassId::none(),
            usage: PropertyUsageFlags::SUBGROUP,
            hint: PropertyHint::NONE,
            hint_string: format!("{parent_prefix}_{prefix}"),
            description,
        }]
        .into_iter()
        .chain(self.properties.into_iter().map(|mut prop| {
            prop.name = format!("{parent_prefix}_{prefix}_{}", prop.name).into();
            prop
        }))
        .collect()
    }
}

/// A group of properties that can are exported by a script.
///
// The script will flatten the properties into its own property list when exporting them to Godot, but groups them together.
pub trait ScriptExportGroup {
    const NAME: &'static str;

    fn get_property(&self, name: &str) -> Option<godot::builtin::Variant>;
    fn set_property(&mut self, name: &str, value: godot::builtin::Variant) -> bool;
    fn properties() -> ExportGroupBuilder;
    fn export_property_states(
        &self,
        prefix: &'static str,
        state: &mut HashMap<StringName, godot::builtin::Variant>,
    );
}

/// A subgroup of properties that can are exported by a script.
///
// Script property groups can be nested at most two levels deep. This means subgroups can be flattened into groups, but subgroups can not
// be nested further.
pub trait ScriptExportSubgroup {
    const NAME: &'static str;

    fn get_property(&self, name: &str) -> Option<godot::builtin::Variant>;
    fn set_property(&mut self, name: &str, value: godot::builtin::Variant) -> bool;
    fn properties() -> ExportSubgroupBuilder;
    fn export_property_states(
        &self,
        prefix: &'static str,
        state: &mut HashMap<StringName, godot::builtin::Variant>,
    );
}

/// The non-prefixed name of the property that toggles the `Option<T>` property group.
#[cfg(since_api = "4.5")]
const OPTION_SCRIPT_PROPERTY_GROUP_PROP: &str = "enable";

macro_rules! impl_optional_group {
    ($trait_name:ident, $builder:ty) => {
        #[cfg(since_api = "4.5")]
        impl<T: $trait_name + Default> $trait_name for Option<T> {
            const NAME: &'static str = T::NAME;

            fn get_property(&self, name: &str) -> Option<godot::builtin::Variant> {
                use godot::meta::ToGodot;

                if name == OPTION_SCRIPT_PROPERTY_GROUP_PROP {
                    return Some(self.is_some().to_variant());
                }

                match self {
                    Some(inner) => inner.get_property(name),
                    None => None,
                }
            }

            fn set_property(&mut self, name: &str, value: godot::builtin::Variant) -> bool {
                if name == OPTION_SCRIPT_PROPERTY_GROUP_PROP {
                    // Unset option if group is disabled / set to false
                    if !value.to::<bool>() {
                        *self = None;
                    // Set option to default if group is enabled but None.
                    } else if self.is_none() {
                        *self = Some(Default::default());
                    }
                    return true;
                }

                if let Some(inner) = self {
                    inner.set_property(name, value)
                } else {
                    let mut inner = T::default();
                    let result = inner.set_property(name, value);

                    if result {
                        *self = Some(inner);
                    }

                    result
                }
            }

            fn properties() -> $builder {
                T::properties().add_property(RustScriptPropDesc {
                    name: OPTION_SCRIPT_PROPERTY_GROUP_PROP.into(),
                    ty: VariantType::BOOL,
                    class_name: ClassId::none(),
                    usage: PropertyUsageFlags::SCRIPT_VARIABLE
                        | PropertyUsageFlags::EDITOR
                        | PropertyUsageFlags::STORAGE,
                    hint: PropertyHint::GROUP_ENABLE,
                    hint_string: String::new(),
                    description: "",
                })
            }

            fn export_property_states(
                &self,
                prefix: &'static str,
                state: &mut HashMap<StringName, godot::builtin::Variant>,
            ) {
                state.insert(
                    format!("{}_{}", prefix, OPTION_SCRIPT_PROPERTY_GROUP_PROP)
                        .as_str()
                        .into(),
                    self.get_property(OPTION_SCRIPT_PROPERTY_GROUP_PROP)
                        .unwrap_or(godot::builtin::Variant::from(false)),
                );

                if let Some(inner) = self.as_ref() {
                    T::export_property_states(inner, prefix, state);
                }
            }
        }
    };
}

impl_optional_group!(ScriptExportGroup, ExportGroupBuilder);
impl_optional_group!(ScriptExportSubgroup, ExportSubgroupBuilder);
