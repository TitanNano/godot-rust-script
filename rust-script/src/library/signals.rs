/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::marker::PhantomData;

use super::RustScriptPropDesc;
use godot::{
    builtin::{
        meta::{GodotConvert, GodotType, ToGodot},
        Callable, Dictionary, GString, NodePath, StringName, Variant, Vector2, Vector3,
    },
    engine::{
        global::{Error, PropertyHint},
        Object,
    },
    obj::Gd,
};

pub trait ScriptSignal {
    type Args: SignalArguments;

    fn new(host: Gd<Object>, name: &'static str) -> Self;

    fn emit(&self, args: Self::Args);

    fn connect(&mut self, callable: Callable) -> Result<(), Error>;

    fn argument_desc() -> Box<[RustScriptPropDesc]>;

    fn name(&self) -> &str;
}

pub trait SignalArguments {
    fn count() -> u8;

    fn to_variants(&self) -> Vec<Variant>;

    fn argument_desc() -> Box<[RustScriptPropDesc]>;
}

impl SignalArguments for () {
    fn count() -> u8 {
        0
    }

    fn to_variants(&self) -> Vec<Variant> {
        vec![]
    }

    fn argument_desc() -> Box<[RustScriptPropDesc]> {
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
            fn count() -> u8 {
                count_tts!($($arg)+)
            }

            fn to_variants(&self) -> Vec<Variant> {
                #[allow(non_snake_case)]
                let ($($arg,)+) = self;

                vec![
                    $(ToGodot::to_variant($arg)),+
                ]
            }

            fn argument_desc() -> Box<[RustScriptPropDesc]> {
                Box::new([
                    $(signal_argument_desc!("0", $arg)),+
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
            fn count() -> u8 {
                1
            }

            fn to_variants(&self) -> Vec<Variant> {
                vec![self.to_variant()]
            }

            fn argument_desc() -> Box<[RustScriptPropDesc]> {
                Box::new([
                    signal_argument_desc!("0", $arg),
                ])
            }
        }
    };

    ($($arg: ty),+) => {
        $(single_args!(impl $arg);)+
    };
}

macro_rules! signal_argument_desc {
    ($name:literal, $type:ty) => {
        RustScriptPropDesc {
            name: $name,
            ty: <<<$type as GodotConvert>::Via as GodotType>::Ffi as godot::sys::GodotFfi>::variant_type(),
            exported: false,
            hint: PropertyHint::NONE,
            hint_string: "",
            description: "",
        }
    };
}

tuple_args!(A1, A2, A3, A4, A5, A6, A7, A8, A9, A10);
single_args!(
    bool, u8, u16, u32, u64, i8, i16, i32, i64, f64, GString, StringName, NodePath, Vector2,
    Vector3, Dictionary
);

#[derive(Debug)]
pub struct Signal<T: SignalArguments> {
    host: Gd<Object>,
    name: &'static str,
    args: PhantomData<T>,
}

impl<T: SignalArguments> ScriptSignal for Signal<T> {
    type Args = T;

    fn new(host: Gd<Object>, name: &'static str) -> Self {
        Self {
            host,
            name,
            args: PhantomData,
        }
    }

    fn emit(&self, args: Self::Args) {
        self.host
            .clone()
            .emit_signal(StringName::from(self.name), &args.to_variants());
    }

    fn connect(&mut self, callable: Callable) -> Result<(), Error> {
        match self.host.connect(self.name.into(), callable) {
            Error::OK => Ok(()),
            error => Err(error),
        }
    }

    fn argument_desc() -> Box<[RustScriptPropDesc]> {
        <T as SignalArguments>::argument_desc()
    }

    fn name(&self) -> &str {
        self.name
    }
}

impl<T: SignalArguments> GodotConvert for Signal<T> {
    type Via = godot::builtin::Signal;
}

impl<T: SignalArguments> ToGodot for Signal<T> {
    fn to_godot(&self) -> Self::Via {
        godot::builtin::Signal::from_object_signal(&self.host, self.name)
    }
}
