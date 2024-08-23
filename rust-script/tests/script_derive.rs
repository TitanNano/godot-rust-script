/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::marker::PhantomData;

use godot::builtin::{Array, GString, StringName, Variant};
use godot::classes::{Node, Node3D};
use godot::engine::Object;
use godot::meta::ToGodot;
use godot::obj::{Gd, NewAlloc};
use godot_rust_script::{
    godot_script_impl, Context, GodotScript, GodotScriptEnum, GodotScriptImpl, RsRef, Signal,
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
#[script(base = Node)]
struct TestScript {
    pub property_a: GString,

    #[export]
    pub editor_prop: u16,

    #[export(enum_options = ["inactive", "water", "teargas"])]
    pub enum_prop: u8,

    #[signal]
    changed: Signal<(u8, u8)>,

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
            base.emit_signal(StringName::from("hit"), &[]);
        });

        ctx.reentrant_scope(self, |mut base: Gd<Node>| {
            base.set_owner(Node::new_alloc());
        });

        result
    }
}

#[diagnostic::on_unimplemented(
    label = "The impl of this trait is missing a #[godot_script_trait] attribute!",
    message = "The implementation of this trait for {Self} has not been marked with #[godot_script_trait]"
)]
trait GodotScriptTraitDispatch<const TRAIT_ID: u64>: GodotScriptImpl {
    fn call(&mut self, method: GString, args: &[Variant], context: Context<'_, Self>);
}

trait TestScriptTrait {
    fn perform_action(&self, action: GString);
}

impl TestScriptTrait for TestScript {
    fn perform_action(&self, action: GString) {
        todo!()
    }
}

const fn byte_array_to_u64<const LENGTH: usize>(array: [u8; LENGTH]) -> u64 {
    let mut result: u64 = 0;
    let mut i = 0;

    while i < 8 {
        result |= (array[i] as u64) << (i * 8);
        i += 1;
    }

    result
}

const TEST_SCRIPT_TRAIT_ID: u64 = {
    let digest = sha2_const::Sha224::new()
        .update(b"TestScriptTrait")
        .finalize();

    byte_array_to_u64(digest)
};

impl GodotScriptTraitDispatch<TEST_SCRIPT_TRAIT_ID> for TestScript {
    fn call(&mut self, method: GString, args: &[Variant], context: Context<'_, Self>) {
        match method.to_string().as_str() {
            "TestScriptTrait__perform_action" => self.perform_action(GString::new()),
            "TestScriptTRait__method_x" => self.perform_action(GString::new()),
            _ => (),
        }
    }
}

fn call_trait_method(method: GString, args: &[Variant], context: Context<'_, TestScript>) {
    let s = TestScript {
        property_a: todo!(),
        editor_prop: todo!(),
        enum_prop: todo!(),
        changed: todo!(),
        node_prop: todo!(),
        node_prop_2: todo!(),
        node_array: todo!(),
        int_range: todo!(),
        custom_enum: todo!(),
        base: todo!(),
    };

    <TestScript as GodotScriptTraitDispatch<TEST_SCRIPT_TRAIT_ID>>::call(
        &mut s, method, args, context,
    )
}

struct RsDyn<T: ?Sized> {
    object: Gd<Object>,
    trait_type: PhantomData<T>,
}

impl TestScriptTrait for RsDyn<dyn TestScriptTrait> {
    fn perform_action(&self, action: GString) {
        self.object.clone().call(
            "TestScriptTrait__perform_action".into(),
            &[action.to_variant()],
        );
    }
}
