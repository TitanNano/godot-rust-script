/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use godot::builtin::{Array, GString};
use godot::classes::{Node, Node3D};
use godot::obj::{Gd, NewAlloc};
use godot_rust_script::{godot_script_impl, Context, GodotScript, GodotScriptEnum, Signal};

#[derive(Debug, Default, GodotScriptEnum)]
#[script_enum(export)]
pub enum ScriptEnum {
    #[default]
    One,
    Two,
    Three,
}

#[derive(GodotScript, Debug)]
#[script(base = Node)]
struct TestScript {
    pub property_a: GString,

    #[export]
    pub editor_prop: u16,

    #[export(enum_options = ["inactive", "water", "teargas"])]
    pub enum_prop: u8,

    #[signal]
    pub changed: Signal<()>,

    #[signal]
    pub ready: Signal<(u32, u32)>,

    pub node_prop: Option<Gd<Node3D>>,

    #[export(ty = "Decal")]
    pub node_prop_2: Option<Gd<Node3D>>,

    #[export]
    pub node_array: Array<Gd<Node3D>>,

    #[export(range(min = 0.0, max = 10.0))]
    pub int_range: u32,

    #[export]
    pub custom_enum: ScriptEnum,

    base: Gd<<Self as GodotScript>::Base>,
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

        ctx.reentrant_scope(self, |mut base: Gd<Node>| {
            base.set_owner(&Node::new_alloc());
        });

        result
    }
}
