/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::{
    any::type_name,
    ops::{Deref, DerefMut},
};

use godot::{
    classes::class_macros::sys::GodotNullableFfi,
    meta::{FromGodot, GodotConvert, GodotType, ToGodot},
};

#[derive(Debug)]
enum ValueState<T> {
    Invalid,
    Valid(T),
}

#[derive(Debug)]
pub struct OnEditor<T> {
    value: ValueState<T>,
}

impl<T> Default for OnEditor<T> {
    fn default() -> Self {
        Self {
            value: ValueState::Invalid,
        }
    }
}

impl<T: GodotConvert> GodotConvert for OnEditor<T>
where
    for<'a> <T::Via as GodotType>::ToFfi<'a>: GodotNullableFfi,
    <T::Via as GodotType>::Ffi: GodotNullableFfi,
{
    type Via = Option<T::Via>;
}

impl<T> godot::prelude::Var for OnEditor<T>
where
    for<'v> T: ToGodot<ToVia<'v> = <T as GodotConvert>::Via> + FromGodot + 'v,
    Self: GodotConvert<Via = Option<T::Via>>,
{
    fn get_property(&self) -> Self::Via {
        match self.value {
            ValueState::Invalid => None,
            ValueState::Valid(ref value) => Some(value.to_godot()),
        }
    }

    fn set_property(&mut self, value: Self::Via) {
        match value {
            Some(value) => self.value = ValueState::Valid(T::from_godot(value)),
            None => self.value = ValueState::Invalid,
        }
    }
}

impl<T> Deref for OnEditor<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self.value {
            ValueState::Invalid => panic!(
                "OnEditor property of type {} is uninitialized!",
                type_name::<T>()
            ),

            ValueState::Valid(ref value) => value,
        }
    }
}

impl<T> DerefMut for OnEditor<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self.value {
            ValueState::Invalid => panic!(
                "OnEditor property of type {} is uninitialized!",
                type_name::<T>()
            ),

            ValueState::Valid(ref mut value) => value,
        }
    }
}
