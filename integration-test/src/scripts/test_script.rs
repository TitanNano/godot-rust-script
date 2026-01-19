/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::collections::HashMap;

use godot::builtin::{Array, GString, StringName, Variant};
use godot::classes::{Node, Node3D};
use godot::global::PropertyHint;
use godot::meta::{FromGodot, ToGodot};
use godot::obj::{Gd, NewAlloc};
use godot_rust_script::{
    CastToScript, Context, GodotScript, GodotScriptEnum, OnEditor, RsRef, ScriptPropertyGroup,
    ScriptPropertySubgroup, ScriptSignal, godot_script_impl,
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

    #[cfg(since_api = "4.5")]
    #[export(flatten)]
    pub property_group: Option<PropertyGroup>,

    #[cfg(before_api = "4.5")]
    #[export(flatten)]
    pub property_group: PropertyGroup,

    base: Gd<<Self as GodotScript>::Base>,
}

#[derive(Debug, Default, ScriptPropertyGroup)]
struct PropertyGroup {
    item1: u32,
    #[export(flatten)]
    item2: PropertySubgroup,
    item3: OnEditor<Gd<Node3D>>,
}

#[derive(ScriptPropertySubgroup, Default, Debug)]
struct PropertySubgroup {
    subitem: f32,
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
