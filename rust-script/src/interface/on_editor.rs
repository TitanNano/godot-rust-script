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

    fn godot_shape() -> godot::meta::GodotShape {
        T::godot_shape()
    }
}

impl<T> godot::prelude::Var for OnEditor<T>
where
    for<'v> T: ToGodot + FromGodot + 'v,
    Self: GodotConvert<Via = Option<T::Via>>,
    T::Via: Clone,
{
    type PubType = Self::Via;

    fn var_get(field: &Self) -> Self::Via {
        match field.value {
            ValueState::Invalid => None,
            ValueState::Valid(ref value) => Some(value.to_godot_owned()),
        }
    }

    fn var_set(field: &mut Self, value: Self::Via) {
        match value {
            Some(value) => field.value = ValueState::Valid(T::from_godot(value)),
            None => field.value = ValueState::Invalid,
        }
    }

    fn var_pub_get(field: &Self) -> Self::PubType {
        match field.value {
            ValueState::Invalid => None,
            ValueState::Valid(ref value) => Some(value.to_godot_owned()),
        }
    }

    fn var_pub_set(field: &mut Self, value: Self::PubType) {
        match value {
            Some(value) => field.value = ValueState::Valid(T::from_godot(value)),
            None => field.value = ValueState::Invalid,
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
