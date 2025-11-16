/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::marker::PhantomData;

use godot::builtin::{
    Callable, Dictionary, GString, NodePath, StringName, Variant, Vector2, Vector3, Vector4,
};
use godot::classes::Object;
use godot::global::{Error, PropertyHint, PropertyUsageFlags};
use godot::meta::{ByValue, GodotConvert, GodotType, ToGodot};
use godot::obj::{Gd, GodotClass};

use crate::static_script_registry::RustScriptPropDesc;
use crate::{GodotScript, RsRef};

use super::GetScriptProperty;

pub trait SignalArguments {
    const COUNT: u8;

    fn to_variants(&self) -> Vec<Variant>;

    fn argument_desc(arg_names: Option<&[&'static str]>) -> Box<[RustScriptPropDesc]>;
}

impl SignalArguments for () {
    const COUNT: u8 = 0;

    fn to_variants(&self) -> Vec<Variant> {
        vec![]
    }

    fn argument_desc(_arg_names: Option<&[&'static str]>) -> Box<[RustScriptPropDesc]> {
        Box::new([])
    }
}

macro_rules! count_tts {
    (inner $sub:expr) => {1};
    ($($tts:expr)+) => {$(count_tts!(inner $tts) + )+ 0};
}

macro_rules! tuple_args {
    (impl $($arg: ident),+) => {
        impl<$($arg: ToGodot),+> SignalArguments for ($($arg,)+) {
            const COUNT: u8 = count_tts!($($arg)+);

            fn to_variants(&self) -> Vec<Variant> {
                #[allow(non_snake_case)]
                let ($($arg,)+) = self;

                vec![
                    $(ToGodot::to_variant($arg)),+
                ]
            }

            fn argument_desc(arg_names: Option<&[&'static str]>) -> Box<[RustScriptPropDesc]> {
                #[expect(non_snake_case)]
                let [$($arg),+] = arg_names.unwrap_or(&[$(stringify!($arg)),+]).try_into().unwrap(); //.unwrap_or_else(|| [$(stringify!($arg)),+]);

                Box::new([
                    $(signal_argument_desc!($arg, $arg)),+
                ])
            }
        }
    };

    (chop $($arg: ident);* | $next: ident $(, $tail: ident)*) => {
        tuple_args!(impl $($arg,)* $next);


        tuple_args!(chop $($arg;)* $next | $($tail),*);
    };

    (chop $($arg: ident);+ |) => {};

    ($($arg: ident),+) => {
        tuple_args!(chop | $($arg),+);
    }
}

macro_rules! single_args {
    (impl $arg: ty) => {
        impl SignalArguments for $arg {
            const COUNT: u8 = 1;

            fn to_variants(&self) -> Vec<Variant> {
                vec![self.to_variant()]
            }

            fn argument_desc(arg_names: Option<&[&'static str]>) -> Box<[RustScriptPropDesc]> {
                let [arg_name] = arg_names.unwrap_or_else(|| &["0"]).try_into().unwrap();

                Box::new([
                    signal_argument_desc!(arg_name, $arg),
                ])
            }
        }
    };

    ($($arg: ty),+) => {
        $(single_args!(impl $arg);)+
    };
}

macro_rules! signal_argument_desc {
    ($name:expr, $type:ty) => {
        RustScriptPropDesc {
            name: $name,
            ty: <<<$type as GodotConvert>::Via as GodotType>::Ffi as godot::sys::GodotFfi>::VARIANT_TYPE.variant_as_nil(),
            class_name: <<$type as GodotConvert>::Via as GodotType>::class_id(),
            usage: PropertyUsageFlags::NONE,
            hint: PropertyHint::NONE,
            hint_string: String::new(),
            description: "",
        }
    };
}

tuple_args!(A1, A2, A3, A4, A5, A6, A7, A8, A9, A10);
single_args!(
    bool, u8, u16, u32, u64, i8, i16, i32, i64, f32, f64, GString, StringName, NodePath, Vector2,
    Vector3, Vector4, Dictionary
);

impl<T: GodotClass> SignalArguments for Gd<T> {
    const COUNT: u8 = 1;

    fn to_variants(&self) -> Vec<Variant> {
        vec![self.to_variant()]
    }

    fn argument_desc(arg_names: Option<&[&'static str]>) -> Box<[RustScriptPropDesc]> {
        let name = arg_names
            .and_then(|list| list.first())
            .copied()
            .unwrap_or("0");

        Box::new([signal_argument_desc!(name, Self)])
    }
}

impl<T: GodotScript> SignalArguments for RsRef<T> {
    const COUNT: u8 = 1;

    fn to_variants(&self) -> Vec<Variant> {
        vec![self.to_variant()]
    }

    fn argument_desc(arg_names: Option<&[&'static str]>) -> Box<[RustScriptPropDesc]> {
        Box::new([signal_argument_desc!(
            arg_names
                .and_then(|list| list.first())
                .copied()
                .unwrap_or("0"),
            Self
        )])
    }
}

#[derive(Debug)]
pub struct ScriptSignal<T: SignalArguments> {
    host: Gd<Object>,
    name: &'static str,
    args: PhantomData<T>,
}

#[deprecated(
    note = "The Signal type has been deprecated and will be removed soon. Please use the ScriptSignal instead."
)]
pub type Signal<T> = ScriptSignal<T>;

impl<T: SignalArguments> ScriptSignal<T> {
    pub const ARG_COUNT: u8 = T::COUNT;

    pub fn new(host: Gd<Object>, name: &'static str) -> Self {
        Self {
            host,
            name,
            args: PhantomData,
        }
    }

    pub fn emit(&self, args: T) {
        self.host
            .clone()
            .emit_signal(self.name, &args.to_variants());
    }

    pub fn connect(&mut self, callable: Callable) -> Result<(), Error> {
        match self.host.connect(self.name, &callable) {
            Error::OK => Ok(()),
            error => Err(error),
        }
    }

    #[doc(hidden)]
    pub fn argument_desc(arg_names: Option<&[&'static str]>) -> Box<[RustScriptPropDesc]> {
        <T as SignalArguments>::argument_desc(arg_names)
    }

    pub fn name(&self) -> &str {
        self.name
    }
}

impl<T: SignalArguments> GodotConvert for ScriptSignal<T> {
    type Via = godot::builtin::Signal;
}

impl<T: SignalArguments> ToGodot for ScriptSignal<T> {
    type Pass = ByValue;

    fn to_godot(&self) -> Self::Via {
        godot::builtin::Signal::from_object_signal(&self.host, self.name)
    }
}

impl<T: SignalArguments> GetScriptProperty for ScriptSignal<T> {
    fn get_property(&self) -> Self::Via {
        self.to_godot()
    }
}
