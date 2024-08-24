/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::ops::DerefMut;
use std::{fmt::Debug, marker::PhantomData};

use godot::obj::{script::ScriptBaseMut, Gd};
use godot::prelude::GodotClass;
use godot_cell::blocking::GdCell;

use crate::interface::GodotScriptImpl;

use super::rust_script_instance::{GodotScriptObject, RustScriptInstance};

pub struct Context<'a, Script: GodotScriptImpl + ?Sized> {
    cell: *const GdCell<Box<dyn GodotScriptObject>>,
    data_ptr: *mut Box<dyn GodotScriptObject>,
    base: ScriptBaseMut<'a, RustScriptInstance>,
    base_type: PhantomData<Script>,
}

impl<'a, Script: GodotScriptImpl> Debug for Context<'a, Script> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Context { <Call Context> }")
    }
}

impl<'a, Script: GodotScriptImpl> Context<'a, Script> {
    pub fn reentrant_scope<T: GodotScriptObject + 'static, Args, Return>(
        &mut self,
        self_ref: &mut T,
        scope: impl ReentrantScope<Script::ImplBase, Args, Return>,
    ) -> Return {
        let known_ptr = unsafe {
            let any = (*self.data_ptr).as_any_mut();

            any.downcast_mut::<T>().unwrap() as *mut T
        };

        let self_ptr = self_ref as *mut _;

        if known_ptr != self_ptr {
            panic!("unable to create reentrant scope with unrelated self reference!");
        }

        let current_ref = unsafe { &mut *self.data_ptr };
        let cell = unsafe { &*self.cell };
        let guard = cell.make_inaccessible(current_ref).unwrap();

        let result = scope.run(self.base.deref_mut().clone().cast::<Script::ImplBase>());

        drop(guard);

        result
    }
}

pub struct GenericContext<'a> {
    cell: *const GdCell<Box<dyn GodotScriptObject>>,
    data_ptr: *mut Box<dyn GodotScriptObject>,
    base: ScriptBaseMut<'a, RustScriptInstance>,
}

impl<'a> GenericContext<'a> {
    pub(super) unsafe fn new(
        cell: *const GdCell<Box<dyn GodotScriptObject>>,
        data_ptr: *mut Box<dyn GodotScriptObject>,
        base: ScriptBaseMut<'a, RustScriptInstance>,
    ) -> Self {
        Self {
            cell,
            data_ptr,
            base,
        }
    }
}

impl<'a, Script: GodotScriptImpl> From<GenericContext<'a>> for Context<'a, Script> {
    fn from(value: GenericContext<'a>) -> Self {
        let GenericContext {
            cell,
            data_ptr,
            base,
        } = value;

        Self {
            cell,
            data_ptr,
            base,
            base_type: PhantomData,
        }
    }
}

pub trait ReentrantScope<Base: GodotClass, Args, Return> {
    fn run(self, base: Gd<Base>) -> Return;
}

impl<Base: GodotClass, F: FnOnce() -> R, R> ReentrantScope<Base, (), R> for F {
    fn run(self, _base: Gd<Base>) -> R {
        self()
    }
}

impl<Base: GodotClass, F: FnOnce(Gd<Base>) -> R, R> ReentrantScope<Base, Gd<Base>, R> for F {
    fn run(self, base: Gd<Base>) -> R {
        self(base)
    }
}
