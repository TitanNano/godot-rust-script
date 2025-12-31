/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::collections::HashMap;

use godot::builtin::{Array, GString, StringName, Variant};
use godot::classes::{Node, Node3D};
use godot::global::{PropertyHint, PropertyUsageFlags};
use godot::meta::{FromGodot, GodotType, ToGodot};
use godot::obj::{Gd, NewAlloc};
use godot::sys::GodotFfi;
use godot_rust_script::private_export::RustScriptPropDesc;
use godot_rust_script::{
    CastToScript, Context, GodotScript, GodotScriptEnum, OnEditor, PropertyGroupBuilder, RsRef,
    ScriptPropertyGroup, ScriptSignal, SetScriptProperty, godot_script_impl,
};

#[derive(Debug, Default, GodotScriptEnum)]
#[script_enum(export)]
pub enum ScriptEnum {
    #[default]
    One,
    Two,
    Three,
}

#[derive(GodotScript, Debug)]
#[script(base = Node, tool)]
struct TestScript {
    pub property_a: GString,

    #[export]
    pub editor_prop: u16,

    #[export(enum_options = ["inactive", "water", "teargas"])]
    pub enum_prop: u8,

    #[signal]
    pub changed: ScriptSignal<()>,

    #[signal("Expected", "Actual")]
    pub ready: ScriptSignal<(u32, u32)>,

    #[signal("Base_Node")]
    pub ready_base: ScriptSignal<Gd<Node>>,

    #[signal]
    pub ready_self: ScriptSignal<RsRef<TestScript>>,

    pub node_prop: Option<Gd<Node3D>>,

    #[export(ty = "Decal")]
    pub node_prop_2: Option<Gd<Node3D>>,

    #[export(ty = "Decal")]
    pub node_prop_3: OnEditor<Gd<Node3D>>,

    #[export]
    pub node_array: Array<Gd<Node3D>>,

    #[export_group(name = "prop_group")]
    #[export(range(min = 0.0, max = 10.0))]
    pub int_range: u32,

    #[export(storage)]
    pub custom_enum: ScriptEnum,

    #[export]
    pub script_ref_opt: Option<RsRef<TestScript>>,

    #[export_subgroup(name = "Refs")]
    #[export(custom(hint = PropertyHint::NODE_TYPE, hint_string = ""))]
    pub script_ref: OnEditor<RsRef<TestScript>>,

    #[export(flatten)]
    pub property_group: Option<PropertyGroup>,

    base: Gd<<Self as GodotScript>::Base>,
}

#[derive(Debug, Default)]
struct PropertyGroup {
    item1: u32,
    item2: GString,
    item3: OnEditor<Gd<Node3D>>,
}

impl ScriptPropertyGroup for PropertyGroup {
    const NAME: &'static str = "Property Group";

    fn get_property(&self, name: &str) -> godot::builtin::Variant {
        match name {
            "item1" => self.item1.to_variant(),
            "item2" => self.item2.to_variant(),
            "item3" => self.item3.to_variant(),
            _ => Variant::nil(),
        }
    }

    fn set_property(&mut self, name: &str, value: godot::builtin::Variant) {
        match name {
            "item1" => self.item1 = FromGodot::try_from_variant(&value).unwrap(),
            "item2" => self.item2 = FromGodot::try_from_variant(&value).unwrap(),
            "item3" => self
                .item3
                .set_property(FromGodot::try_from_variant(&value).unwrap()),
            _ => (),
        }
    }

    fn properties() -> PropertyGroupBuilder {
        PropertyGroupBuilder::new(Self::NAME, 2)
            .add_property(RustScriptPropDesc {
                class_name: <u32 as GodotType>::class_id(),
                name: "item1".into(),
                ty: <<u32 as GodotType>::Ffi as GodotFfi>::VARIANT_TYPE.variant_as_nil(),
                hint: PropertyHint::NONE,
                usage: PropertyUsageFlags::SCRIPT_VARIABLE
                    | PropertyUsageFlags::EDITOR
                    | PropertyUsageFlags::STORAGE,
                hint_string: String::new(),
                description: "",
            })
            .add_property(RustScriptPropDesc {
                class_name: <GString as GodotType>::class_id(),
                name: "item2".into(),
                ty: <<GString as GodotType>::Ffi as GodotFfi>::VARIANT_TYPE.variant_as_nil(),
                hint: PropertyHint::NONE,
                usage: PropertyUsageFlags::SCRIPT_VARIABLE
                    | PropertyUsageFlags::EDITOR
                    | PropertyUsageFlags::STORAGE,
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
            format!("{}_item1", prefix).as_str().into(),
            self.item1.to_variant(),
        );
        state.insert(
            format!("{}_item2", prefix).as_str().into(),
            self.item2.to_variant(),
        );
    }
}

#[godot_script_impl]
impl TestScript {
    pub fn _init(&self) {}

    pub fn record(&mut self, value: u8) -> bool {
        value > 2
    }

    pub fn action(&mut self, input: GString, mut ctx: Context<Self>) -> bool {
        let result = input.len() > 2;
        let mut base = self.base.clone();

        ctx.reentrant_scope(self, || {
            base.emit_signal("hit", &[]);
        });

        self.ready.emit((1, 2));
        self.ready_base.emit(self.base.clone());
        self.ready_self.emit(self.base.to_script());

        ctx.reentrant_scope(self, |mut base: Gd<Node>| {
            base.set_owner(&Node::new_alloc());
        });

        result
    }
}
