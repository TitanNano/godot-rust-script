/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use godot::builtin::{GString, StringName};
use godot::classes::Node;
use godot::obj::{Gd, NewAlloc};
use godot_rust_script::{godot_script_impl, Context, GodotScript};

#[derive(GodotScript, Debug)]
#[script(base = Node)]
struct TestScript {
    pub property_a: GString,
    #[export]
    pub editor_prop: u16,

    #[export(enum_options = ["inactive", "water", "teargas"])]
    pub enum_prop: u8,

    base: Gd<<Self as GodotScript>::Base>,
}

#[godot_script_impl]
impl TestScript {
    pub fn record(&mut self, value: u8) -> bool {
        value > 2
    }

    pub fn action(&mut self, input: GString, mut ctx: Context<Self>) -> bool {
        let result = input.len() > 2;
        let mut base = self.base.clone();

        ctx.reentrant_scope(self, || {
            base.emit_signal(StringName::from("hit"), &[]);
        });

        ctx.reentrant_scope(self, |mut base: Gd<Node>| {
            base.set_owner(Node::new_alloc());
        });

        result
    }
}
