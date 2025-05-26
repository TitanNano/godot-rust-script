/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

mod export;
mod on_editor;
mod signals;

use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::{collections::HashMap, fmt::Debug};

use godot::meta::{FromGodot, GodotConvert, ToGodot};
use godot::obj::Inherits;
use godot::prelude::{ConvertError, Gd, Object, StringName, Variant};

pub use crate::runtime::Context;

pub use export::GodotScriptExport;
pub use on_editor::OnEditor;
#[expect(deprecated)]
pub use signals::{ScriptSignal, Signal};

/// The primary trait of this library. This trait must be implemented by a struct to create a new rust script.
///
/// While it is possible, it's not intended that this trait is implemented by hand. Use the [derive macro](derive@crate::GodotScript) to
/// implement this trait.
pub trait GodotScript: Debug + GodotScriptImpl<ImplBase = Self::Base> {
    /// The base godot class of the script.
    ///
    /// It's currently not possible to use an other script as the base.
    type Base: Inherits<Object>;

    /// The globally unique class name of the rust script.
    const CLASS_NAME: &'static str;

    /// Set the value of a script property as a [`Variant`].
    ///
    /// This is called by the engine to interact with the script.
    fn set(&mut self, name: StringName, value: Variant) -> bool;

    /// Get the value of a script property as a [`Variant`].
    ///
    /// This is called by the engine to interact with the script.
    fn get(&self, name: StringName) -> Option<Variant>;

    /// Call a script method with arguments.
    ///
    /// The engine will use this to pass method calls from other scripts and engine callbacks.
    fn call(
        &mut self,
        method: StringName,
        args: &[&Variant],
        context: Context<'_, Self>,
    ) -> Result<Variant, godot::sys::GDExtensionCallErrorType>;

    /// String representation of the script.
    ///
    /// This is mostly used in debug formatting.
    fn to_string(&self) -> String;
    fn property_state(&self) -> HashMap<StringName, Variant>;

    /// Create the default state of the script.
    ///
    /// The base is passed no matter if it's required or not.
    fn default_with_base(base: godot::prelude::Gd<godot::prelude::Object>) -> Self;
}

/// Inditection to dispatch script method calls.
///
/// To support implementing method dispatch via macros the actual call dispatch logic is split into a separate trait.
pub trait GodotScriptImpl {
    /// The godot base class that is expected for the function call dispatch.
    ///
    /// This has to match the base of the [`GodotScript`] impl.
    type ImplBase: Inherits<Object>;

    /// Dispatch calls to script methods.
    ///
    /// Handles the dynamic dispatching of script method calls. Should be called by [`GodotScript::call`].
    fn call_fn(
        &mut self,
        name: StringName,
        args: &[&Variant],
        context: Context<Self>,
    ) -> Result<Variant, godot::sys::GDExtensionCallErrorType>;
}

#[derive(Debug)]
pub struct RsRef<T: GodotScript> {
    owner: Gd<T::Base>,
    script_ty: PhantomData<T>,
}

impl<T: GodotScript> RsRef<T> {
    pub(crate) fn new<B: Inherits<T::Base> + Inherits<Object>>(owner: Gd<B>) -> Self {
        Self {
            owner: owner.upcast(),
            script_ty: PhantomData,
        }
    }

    fn validate_script<O: Inherits<Object>>(owner: &Gd<O>) -> Option<GodotScriptCastError> {
        let script = owner
            .upcast_ref::<Object>()
            .get_script()
            .try_to::<Option<Gd<crate::runtime::RustScript>>>();

        let Ok(script) = script else {
            return Some(GodotScriptCastError::NotRustScript);
        };

        let Some(script) = script else {
            return Some(GodotScriptCastError::NoScriptAttached);
        };

        let class_name = script.bind().str_class_name();

        (class_name != T::CLASS_NAME).then(|| {
            GodotScriptCastError::ClassMismatch(T::CLASS_NAME, script.get_class().to_string())
        })
    }
}

impl<T: GodotScript> Deref for RsRef<T> {
    type Target = Gd<T::Base>;

    fn deref(&self) -> &Self::Target {
        &self.owner
    }
}

impl<T: GodotScript> DerefMut for RsRef<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.owner
    }
}

impl<T: GodotScript> Clone for RsRef<T> {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            script_ty: PhantomData,
        }
    }
}

impl<T: GodotScript> GodotConvert for RsRef<T> {
    type Via = Gd<T::Base>;
}

impl<T: GodotScript> FromGodot for RsRef<T>
where
    T::Base: Inherits<T::Base>,
{
    fn try_from_godot(via: Self::Via) -> Result<Self, godot::prelude::ConvertError> {
        via.try_to_script().map_err(ConvertError::with_error)
    }
}

impl<T: GodotScript> ToGodot for RsRef<T> {
    type ToVia<'v>
        = Gd<T::Base>
    where
        Self: 'v;

    fn to_godot(&self) -> Self::ToVia<'_> {
        self.deref().clone()
    }
}

impl<'v, T: GodotScript> ::godot::prelude::Var for RsRef<T>
where
    Self: GodotConvert<Via = <Self as ToGodot>::ToVia<'v>>,
    Self: 'v,
{
    fn get_property(&self) -> Self::Via {
        <Self as ToGodot>::to_godot(self)
    }

    fn set_property(&mut self, value: Self::Via) {
        <Self as FromGodot>::from_godot(value);
    }
}

/// Script downcasting error
///
/// This error can occour when trying to downcast an object into a specifc script. If the desired script is actually attached to the object
/// has to be verified at runtime.
#[derive(thiserror::Error, Debug)]
pub enum GodotScriptCastError {
    /// Occours when an object doesn't have a script attached at runtime.
    #[error("Object has no script attached!")]
    NoScriptAttached,

    /// Occours when the attached script is not a `RustScript`.
    #[error("Script attached to object is not a RustScript!")]
    NotRustScript,

    /// Occours when the attached `RustScript` class does not match at runtime.
    #[error(
        "Script attached to object does not match expected script class `{0}` but found `{1}`!"
    )]
    ClassMismatch(&'static str, String),
}

/// This trait allows casting engine objects / types into a [`RsRef`].
pub trait CastToScript<T: GodotScript> {
    /// Falibly Cast the object into a rust script reference.
    ///
    /// The error can be handled if the conversion fails.
    fn try_to_script(&self) -> Result<RsRef<T>, GodotScriptCastError>;

    /// Falibly cast the object into a rust script reference without incrementing the ref-count.
    ///
    /// This is a bit more efficient than [`try_to_script`].
    /// The error can be handled if the conversion fails.
    fn try_into_script(self) -> Result<RsRef<T>, GodotScriptCastError>;

    /// Cast the object into a rust script reference.
    ///
    /// # Panics
    /// - if the expected script is not attached to the object at runtime.
    fn to_script(&self) -> RsRef<T>;

    /// Cast the object into a rust script reference without incrementing the ref-count.
    ///
    /// # Panics
    /// - if the expected script is not attached to the object at runtime.
    fn into_script(self) -> RsRef<T>;
}

impl<T: GodotScript, B: Inherits<T::Base> + Inherits<Object>> CastToScript<T> for Gd<B> {
    fn try_to_script(&self) -> Result<RsRef<T>, GodotScriptCastError> {
        if let Some(err) = RsRef::<T>::validate_script(self) {
            return Err(err);
        }

        Ok(RsRef::new(self.clone()))
    }

    fn try_into_script(self) -> Result<RsRef<T>, GodotScriptCastError> {
        if let Some(err) = RsRef::<T>::validate_script(&self) {
            return Err(err);
        }

        Ok(RsRef::new(self))
    }

    fn to_script(&self) -> RsRef<T> {
        self.try_to_script().unwrap_or_else(|err| {
            panic!(
                "`{}` was assumed to have rust script `{}`, but this was not the case at runtime!\nError: {}",
                B::class_name(),
                T::CLASS_NAME,
                err,
            );
        })
    }

    fn into_script(self) -> RsRef<T> {
        self.try_into_script().unwrap_or_else(|err| {
            panic!(
                "`{}` was assumed to have rust script `{}`, but this was not the case at runtime!\nError: {}",
                B::class_name(),
                T::CLASS_NAME,
                err
            );
        })
    }
}

/// Script property access indirection
///
/// gdext uses this kind of indirection to allow conversion of the actual property value into a godot compatible type when accessing the
/// property from the engine. This Trait separates the `::godot::prelude::Var` trait into it's get and set components for more granular
/// requirements on the property types.
pub trait GetScriptProperty: GodotConvert {
    /// Get the value of a script property as it's godot engine type.
    fn get_property(&self) -> Self::Via;
}

/// Script property write indirection
///
/// gdext uses this kind of indirection to allow conversion of the actual property value from a godot compatible type when setting the
/// property from the engine. This Trait separates the `::godot::prelude::Var` trait into it's get and set components for more granular
/// requirements on the property types.
pub trait SetScriptProperty: GodotConvert {
    /// Set the value of a script property as it's godot engine type.
    fn set_property(&mut self, value: Self::Via);
}

/// Unified property init strategy.
///
/// Most of the time we can initialize a script property with the `Default` trait. To support cases where `Default` is not implemented we
/// can manually implement this trait.
pub trait InitScriptProperty {
    /// Initialize the default value of a script property.
    fn init_property() -> Self;
}

impl<T> GetScriptProperty for T
where
    T: godot::prelude::Var,
{
    fn get_property(&self) -> Self::Via {
        T::get_property(self)
    }
}

impl<T> SetScriptProperty for T
where
    T: godot::prelude::Var,
{
    fn set_property(&mut self, value: Self::Via) {
        T::set_property(self, value);
    }
}

impl<T> InitScriptProperty for T
where
    T: Default,
{
    fn init_property() -> Self {
        Default::default()
    }
}

/// Defines the root module for rust scripts. All scripts must be in submodules of the root module.
///
/// There must be a script root module in your project for Godot Rust Script to work. Using multiple root modules is currently not supported.
///
/// # Example
/// ```ignore
/// # use godot_rust_script::define_script_root;
/// // Example script root: src/scripts/mod.rs
///
/// // define your script modules that contain `RustScript` structs.
/// mod player;
/// mod mob;
///
/// define_script_root!();
/// ```
#[macro_export]
macro_rules! define_script_root {
    () => {
        #[no_mangle]
        #[doc(hidden)]
        pub fn __godot_rust_script_init(
        ) -> ::std::vec::Vec<$crate::private_export::RustScriptMetaData> {
            use $crate::godot::obj::EngineEnum;
            use $crate::private_export::*;

            let lock = $crate::private_export::SCRIPT_REGISTRY
                .lock()
                .expect("unable to aquire mutex lock");

            $crate::private_export::assemble_metadata(lock.iter())
        }

        #[doc(hidden)]
        pub const __GODOT_RUST_SCRIPT_SRC_ROOT: &str = $crate::private_export::concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src",
            $crate::private_export::replace!(
                $crate::private_export::unwrap!($crate::private_export::strip_prefix!(
                    module_path!(),
                    $crate::private_export::replace!(env!("CARGO_PKG_NAME"), "-", "_")
                )),
                "::",
                "/"
            ),
        );
    };
}

/// DEPRECATED: This macro has been renamed to [define_script_root].
#[deprecated = "Has been renamed to define_script_root!()"]
#[macro_export]
macro_rules! setup_library {
    () => {
        ::godot_rust_script::define_script_root!();
    };
}

pub trait GodotScriptEnum: GodotConvert + FromGodot + ToGodot {}

/// Initialize the rust script runtime. This should be part of your `ExtensionLibrary::on_level_init` function.
///
/// # Example
/// ```
/// # use godot::init::{gdextension, InitLevel, ExtensionLibrary};
/// #
/// # mod scripts {
/// #     pub const __GODOT_RUST_SCRIPT_SRC_ROOT: &str = "/dummy/root";
/// #
/// #     pub fn __godot_rust_script_init() -> Vec<godot_rust_script::private_export::RustScriptMetaData> {
/// #         unimplemented!()
/// #     }
/// # }
/// #
/// struct Lib;
///
/// #[gdextension]
/// unsafe impl ExtensionLibrary for Lib {
///     fn on_level_init(level: InitLevel) {
///         match level {
///             InitLevel::Core => (),
///             InitLevel::Servers => (),
///             InitLevel::Scene => godot_rust_script::init!(scripts),
///             InitLevel::Editor => (),
///         }
///     }
///  
///  #  fn on_level_deinit(level: InitLevel) {
///  #      match level {
///  #          InitLevel::Editor => (),
///  #          InitLevel::Scene => godot_rust_script::deinit!(),
///  #          InitLevel::Servers => (),
///  #          InitLevel::Core => (),
///  #      }
///  #  }
/// }
/// ````
#[macro_export]
macro_rules! init {
    ($scripts_module:tt) => {
        $crate::RustScriptExtensionLayer::initialize(
            $scripts_module::__godot_rust_script_init,
            $scripts_module::__GODOT_RUST_SCRIPT_SRC_ROOT,
        )
    };
}

/// Deinitialize the rust script runtime. This should be part of your `ExtensionLibrary::on_level_deinit` function.
///
/// # Example
/// ```
/// # use godot::init::{gdextension, InitLevel, ExtensionLibrary};
/// #
/// # mod scripts {
/// #     pub const __GODOT_RUST_SCRIPT_SRC_ROOT: &str = "/dummy/root";
/// #
/// #     pub fn __godot_rust_script_init() -> Vec<godot_rust_script::private_export::RustScriptMetaData> {
/// #         unimplemented!()
/// #     }
/// # }
/// #
/// struct Lib;
///
/// #[gdextension]
/// unsafe impl ExtensionLibrary for Lib {
/// #   fn on_level_init(level: InitLevel) {
/// #       match level {
/// #           InitLevel::Core => (),
/// #           InitLevel::Servers => (),
/// #           InitLevel::Scene => godot_rust_script::init!(scripts),
/// #           InitLevel::Editor => (),
/// #       }
/// #   }
/// #
///     fn on_level_deinit(level: InitLevel) {
///         match level {
///             InitLevel::Editor => (),
///             InitLevel::Scene => godot_rust_script::deinit!(),
///             InitLevel::Servers => (),
///             InitLevel::Core => (),
///         }
///     }
/// }
/// ````
#[macro_export]
macro_rules! deinit {
    () => {
        $crate::RustScriptExtensionLayer::deinitialize()
    };
}
