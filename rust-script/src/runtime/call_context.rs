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

/// A call context for a script method call.
///
/// The call context can be used to perform re-entrant calls into engine APIs. Its lifetime is constrained to the duration of the current
/// function.
pub struct Context<'a, Script: GodotScriptImpl + ?Sized> {
    cell: *const GdCell<Box<dyn GodotScriptObject>>,
    data_ptr: *mut Box<dyn GodotScriptObject>,
    base: ScriptBaseMut<'a, RustScriptInstance>,
    base_type: PhantomData<Script>,
}

impl<Script: GodotScriptImpl> Debug for Context<'_, Script> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Context { <Call Context> }")
    }
}

impl<Script: GodotScriptImpl> Context<'_, Script> {
    /// Create a scope in which the current mutable ref to [`Self`] is released.
    ///
    /// A re-entrant scope allows to use engine APIs that call back into the current script.
    pub fn reentrant_scope<T: GodotScriptObject + 'static, Args, Return>(
        &mut self,
        self_ref: &mut T,
        scope: impl ReentrantScope<Script::ImplBase, Args, Return>,
    ) -> Return {
        // SAFETY: the caller guaranteed that the data_ptr is valid for the lifetime of `Self`.
        let known_box_ptr = unsafe { &mut *self.data_ptr };
        let known_ptr = known_box_ptr.as_any_mut().downcast_mut::<T>().unwrap() as *mut T;

        let self_ptr = self_ref as *mut _;

        if known_ptr != self_ptr {
            panic!("unable to create reentrant scope with unrelated self reference!");
        }

        // SAFETY: the caller guaranteed that the data_ptr is valid for the lifetime of `Self`.
        let current_ref = unsafe { &mut *self.data_ptr };
        // SAFETY: the caller guaranteed that the cell is valid for the lifetime of `Self`.
        let cell = unsafe { &*self.cell };
        let guard = cell.make_inaccessible(current_ref).unwrap();

        let result = scope.run(self.base.deref_mut().clone().cast::<Script::ImplBase>());

        drop(guard);

        result
    }
}

/// A generic script call context that is not tied to a specific script type.
pub struct GenericContext<'a> {
    cell: *const GdCell<Box<dyn GodotScriptObject>>,
    data_ptr: *mut Box<dyn GodotScriptObject>,
    base: ScriptBaseMut<'a, RustScriptInstance>,
}

impl<'a> GenericContext<'a> {
    /// Create a new script call context.
    ///
    /// # Safety
    /// - cell must be a valid pointer to a [`GdCell`] & not null.
    /// - data_ptr must be a valid pointer to the [`Box<dyn GodotScriptObject>`] inside the [`GdCell`].
    /// - both `cell` and `data_ptr` must out-live the `base`.
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
