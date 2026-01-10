use std::collections::HashMap;

use godot::builtin::{StringName, VariantType};
use godot::global::{PropertyHint, PropertyUsageFlags};
use godot::meta::ClassId;

use crate::private_export::RustScriptPropDesc;

/// Build metadata for script property groups.
///
/// The builder allows assembling a group of script properties in multiple steps.
pub struct PropertyGroupBuilder {
    name: &'static str,
    properties: Vec<RustScriptPropDesc>,
}

impl PropertyGroupBuilder {
    pub fn new(name: &'static str, capacity: usize) -> Self {
        Self {
            name,
            properties: Vec::with_capacity(capacity),
        }
    }

    pub fn add_property(mut self, property_desc: RustScriptPropDesc) -> Self {
        self.properties.push(property_desc);
        self
    }

    pub fn build(self, prefix: &str, description: &'static str) -> Box<[RustScriptPropDesc]> {
        [RustScriptPropDesc {
            name: self.name.into(),
            ty: VariantType::NIL,
            class_name: ClassId::none(),
            usage: PropertyUsageFlags::GROUP,
            hint: PropertyHint::NONE,
            hint_string: prefix.into(),
            description,
        }]
        .into_iter()
        .chain(self.properties.into_iter().map(|mut prop| {
            prop.name = format!("{prefix}{}", prop.name).into();
            prop
        }))
        .collect()
    }
}

/// A group of properties that can are exported by a script.
///
// The script will flatten the properties into its own property list when exporting them to Godot, but groups them together.
pub trait ScriptPropertyGroup {
    const NAME: &'static str;

    fn get_property(&self, name: &str) -> godot::builtin::Variant;
    fn set_property(&mut self, name: &str, value: godot::builtin::Variant);
    fn properties() -> PropertyGroupBuilder;
    fn export_property_states(
        &self,
        prefix: &'static str,
        state: &mut HashMap<StringName, godot::builtin::Variant>,
    );
}

/// The non-prefixed name of the property that toggles the `Option<T>` property group.
#[cfg(since_api = "4.5")]
const OPTION_SCRIPT_PROPERTY_GROUP_PROP: &str = "enable";

#[cfg(since_api = "4.5")]
impl<T: ScriptPropertyGroup + Default> ScriptPropertyGroup for Option<T> {
    const NAME: &'static str = T::NAME;

    fn get_property(&self, name: &str) -> godot::builtin::Variant {
        use godot::meta::ToGodot;

        if name == OPTION_SCRIPT_PROPERTY_GROUP_PROP {
            return self.is_some().to_variant();
        }

        match self {
            Some(inner) => inner.get_property(name),
            None => godot::builtin::Variant::nil(),
        }
    }

    fn set_property(&mut self, name: &str, value: godot::builtin::Variant) {
        if name == OPTION_SCRIPT_PROPERTY_GROUP_PROP {
            if value.to::<bool>() {
                *self = Some(Default::default());
            } else {
                *self = None;
            }
            return;
        }

        if let Some(inner) = self {
            inner.set_property(name, value)
        }
    }

    fn properties() -> PropertyGroupBuilder {
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
            self.get_property(OPTION_SCRIPT_PROPERTY_GROUP_PROP),
        );

        if let Some(inner) = self.as_ref() {
            T::export_property_states(inner, prefix, state);
        }
    }
}
