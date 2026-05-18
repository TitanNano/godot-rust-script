/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

mod export;
mod on_editor;
mod property_group;
mod signals;

use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::{collections::HashMap, fmt::Debug};

use godot::meta::conv::ByValue;
use godot::meta::error::CallErrorType;
use godot::meta::shape::GodotShape;
use godot::meta::{ClassId, FromGodot, GodotConvert, ToGodot};
use godot::obj::Inherits;
use godot::prelude::{ConvertError, Gd, Object, StringName, Variant};

pub use crate::runtime::Context;
use crate::runtime::RustScript;

pub use export::GodotScriptExport;
pub use on_editor::OnEditor;
pub use property_group::{
    ExportGroupBuilder, ExportSubgroupBuilder, ScriptExportGroup, ScriptExportSubgroup,
};
#[expect(deprecated)]
pub use signals::{ScriptSignal, Signal};

// ----------------------------------------- Godot Rust Script -----------------------------------------

pub trait GodotScript: Debug + GodotScriptImpl<ImplBase = Self::Base> {
    type Base: Inherits<Object>;

    const CLASS_NAME: &'static str;

    fn set(&mut self, name: StringName, value: Variant) -> bool;
    fn get(&self, name: StringName) -> Option<Variant>;
    fn call(
        &mut self,
        method: StringName,
        args: &[&Variant],
        context: Context<'_, Self>,
    ) -> Result<Variant, CallErrorType>;

    fn to_string(&self) -> String;
    fn property_state(&self) -> HashMap<StringName, Variant>;

    fn default_with_base(base: godot::prelude::Gd<godot::prelude::Object>) -> Self;
}

pub trait GodotScriptImpl {
    type ImplBase: Inherits<Object>;

    fn call_fn(
        &mut self,
        name: StringName,
        args: &[&Variant],
        context: Context<Self>,
    ) -> Result<Variant, godot::meta::error::CallErrorType>;
}

// ----------------------------------------- Rust Script Reference -----------------------------------------

/// A Godot rust script reference.
///
/// This type represents a Godot rust script that is known to have a script of type `T` attached. The script reference allows calling
/// public methods of the script without having to resort to using `Object::call`.
#[derive(Debug)]
pub struct Rs<T: GodotScript> {
    owner: Gd<T::Base>,
    script_ty: PhantomData<T>,
}

#[deprecated(note = "Has been renamed to Rs<T>.")]
pub type RsRef<T> = Rs<T>;

impl<T: GodotScript> Rs<T> {
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
            .map(|script| script.try_cast::<RustScript>())
            .transpose();

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

    /// Coerce a script reference into a trait object.
    ///
    /// The trait `RsDynify<dyn Trait>` must be implemented for a script type for it to be coerce-able. Implementing the trait requires a single line.
    ///
    /// ```rs
    /// # trait ScriptTrait {}
    /// #
    /// # #[derive(GodotScript)]
    /// # struct TestScript;
    /// #
    /// # #[godot_script_impl]
    /// # impl TestScript {}
    /// #
    /// impl RsDynify<dyn ScriptTrait> for TestScript {
    ///     fn coherce(source: Rs<Self>) -> Box<dyn ScriptTrait> {
    ///         Box::new(source) as Box<dyn ScriptTrait>
    ///     }
    /// }
    /// ```
    /// An attribute macro will likely be provided in the future.
    pub fn into_trait<Trait: ?Sized>(self) -> RsDyn<Trait>
    where
        T: RsDynify<Trait>,
    {
        RsDynify::wrap(self)
    }
}

impl<T: GodotScript> Deref for Rs<T> {
    type Target = Gd<T::Base>;

    fn deref(&self) -> &Self::Target {
        &self.owner
    }
}

impl<T: GodotScript> DerefMut for Rs<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.owner
    }
}

impl<T: GodotScript> Clone for Rs<T> {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            script_ty: PhantomData,
        }
    }
}

impl<T: GodotScript> GodotConvert for Rs<T> {
    type Via = Gd<T::Base>;

    fn godot_shape() -> GodotShape {
        Gd::<T::Base>::godot_shape()
    }
}

impl<T: GodotScript> FromGodot for Rs<T>
where
    T::Base: Inherits<T::Base>,
{
    fn try_from_godot(via: Self::Via) -> Result<Self, godot::prelude::ConvertError> {
        via.try_to_script().map_err(ConvertError::with_error)
    }
}

impl<T: GodotScript> ToGodot for Rs<T> {
    type Pass = ByValue;

    fn to_godot(&self) -> Self::Via {
        self.deref().clone()
    }
}

impl<T: GodotScript> ::godot::prelude::Var for Rs<T>
where
    Self: GodotConvert,
{
    type PubType = Self::Via;

    fn var_get(field: &Self) -> Self::Via {
        <Self as ToGodot>::to_godot(field)
    }

    fn var_set(field: &mut Self, value: Self::Via) {
        *field = <Self as FromGodot>::from_godot(value);
    }

    fn var_pub_get(field: &Self) -> Self::PubType {
        Self::var_get(field)
    }

    fn var_pub_set(field: &mut Self, value: Self::PubType) {
        Self::var_set(field, value);
    }
}

// ----------------------------------------- Rust Script Trait Object Ref -----------------------------------------

/// Godot rust script reference as a trait object.
///
/// It is possible to implement traits for Godot rust scripts.
#[derive(Debug)]
pub struct RsDyn<T: ?Sized> {
    owner: Box<T>,
}

/// Support trait for dynamic trait coercion.
///
/// Any script type that implements a trait must also implement this trait for the script trait to be accessible.
///
/// See [`Rs::into_trait`].
#[diagnostic::on_unimplemented(
    message = "Unable to create trait object {T} from Rs<{Self}>",
    label = "Conversion not implemented",
    note = "If {T} is implemented for Rs<{Self}>, you also have to implement RsDynify<{T}> for {Self}"
)]
pub trait RsDynify<T: ?Sized>: Sized + GodotScript {
    fn coerce(source: Rs<Self>) -> Box<T>;

    fn wrap(value: Rs<Self>) -> RsDyn<T> {
        RsDyn {
            owner: Self::coerce(value),
        }
    }
}

impl<T: ?Sized> Deref for RsDyn<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.owner
    }
}

impl<T: ?Sized> DerefMut for RsDyn<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.owner
    }
}

#[derive(thiserror::Error, Debug)]
pub enum GodotScriptCastError {
    #[error("Object has no script attached!")]
    NoScriptAttached,

    #[error("Script attached to object is not a RustScript!")]
    NotRustScript,

    #[error(
        "Script attached to object does not match expected script class `{0}` but found `{1}`!"
    )]
    ClassMismatch(&'static str, String),
}

pub trait CastToScript {
    type Base: Inherits<Object>;

    fn try_to_script<T: GodotScript>(&self) -> Result<Rs<T>, GodotScriptCastError>
    where
        Self::Base: Inherits<T::Base>;
    fn try_into_script<T: GodotScript>(self) -> Result<Rs<T>, GodotScriptCastError>
    where
        Self::Base: Inherits<T::Base>;

    fn to_script<T: GodotScript>(&self) -> Rs<T>
    where
        Self::Base: Inherits<T::Base>;

    fn into_script<T: GodotScript>(self) -> Rs<T>
    where
        Self::Base: Inherits<T::Base>;
}

impl<B: Inherits<Object>> CastToScript for Gd<B> {
    type Base = B;

    fn try_to_script<T: GodotScript>(&self) -> Result<Rs<T>, GodotScriptCastError>
    where
        Self::Base: Inherits<T::Base>,
    {
        if let Some(err) = Rs::<T>::validate_script(self) {
            return Err(err);
        }

        Ok(Rs::new(self.clone()))
    }

    fn try_into_script<T: GodotScript>(self) -> Result<Rs<T>, GodotScriptCastError>
    where
        Self::Base: Inherits<T::Base>,
    {
        if let Some(err) = Rs::<T>::validate_script(&self) {
            return Err(err);
        }

        Ok(Rs::new(self))
    }

    fn to_script<T: GodotScript>(&self) -> Rs<T>
    where
        Self::Base: Inherits<T::Base>,
    {
        self.try_to_script().unwrap_or_else(|err| {
            panic!(
                "`{}` was assumed to have rust script `{}`, but this was not the case at runtime!\nError: {}",
                B::class_id(),
                T::CLASS_NAME,
                err,
            );
        })
    }

    fn into_script<T: GodotScript>(self) -> Rs<T>
    where
        Self::Base: Inherits<T::Base>,
    {
        self.try_into_script().unwrap_or_else(|err| {
            panic!(
                "`{}` was assumed to have rust script `{}`, but this was not the case at runtime!\nError: {}",
                B::class_id(),
                T::CLASS_NAME,
                err
            );
        })
    }
}

/// Script property access indirection
///
/// Gdext uses this kind of indirection to allow conversion of the actual property value into a Godot compatible type when accessing the
/// property from the engine. This Trait separates the `::godot::prelude::Var` trait into it's get and set components for more granular
/// requirements on the property types.
pub trait GetScriptProperty: GodotConvert {
    fn get_property(&self) -> Self::Via;
}

/// Script property write indirection
///
/// Gdext uses this kind of indirection to allow conversion of the actual property value from a Godot compatible type when setting the
/// property from the engine. This Trait separates the `::godot::prelude::Var` trait into it's get and set components for more granular
/// requirements on the property types.
pub trait SetScriptProperty: GodotConvert {
    fn set_property(&mut self, value: Self::Via);
}

/// Unified property init strategy.
///
/// Most of the time we can initialize a script property with the `Default` trait. To support cases where `Default` is not implemented we
/// can manually implement this trait.
pub trait InitScriptProperty {
    fn init_property() -> Self;
}

impl<T> GetScriptProperty for T
where
    T: godot::prelude::Var,
{
    fn get_property(&self) -> Self::Via {
        T::var_get(self)
    }
}

impl<T> SetScriptProperty for T
where
    T: godot::prelude::Var,
{
    fn set_property(&mut self, value: Self::Via) {
        T::var_set(self, value);
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

#[macro_export]
macro_rules! define_script_root {
    () => {
        #[unsafe(no_mangle)]
        pub fn __godot_rust_script_init()
        -> ::std::vec::Vec<$crate::private_export::RustScriptMetaData> {
            use $crate::godot::obj::EngineEnum;
            use $crate::private_export::*;

            let lock = $crate::private_export::SCRIPT_REGISTRY
                .lock()
                .expect("unable to aquire mutex lock");

            $crate::private_export::assemble_metadata(lock.iter())
        }

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

#[deprecated = "Has been renamed to define_script_root!()"]
#[macro_export]
macro_rules! setup_library {
    () => {
        ::godot_rust_script::define_script_root!();
    };
}

pub trait GodotScriptEnum: GodotConvert + FromGodot + ToGodot {}

#[macro_export]
macro_rules! init {
    ($scripts_module:tt) => {
        $crate::RustScriptExtensionLayer::initialize(
            $scripts_module::__godot_rust_script_init,
            $scripts_module::__GODOT_RUST_SCRIPT_SRC_ROOT,
        )
    };
}

#[macro_export]
macro_rules! deinit {
    () => {
        $crate::RustScriptExtensionLayer::deinitialize()
    };
}

pub fn class_id_for_shape(shape: GodotShape) -> ClassId {
    match shape {
        GodotShape::Variant => ClassId::none(),
        GodotShape::Builtin {
            variant_type: _,
            metadata: _,
        } => ClassId::none(),
        GodotShape::Class {
            class_id,
            heritage: _,
            is_nullable: _,
        } => class_id,
        GodotShape::TypedArray { element } => class_id_for_shape(element.into_outer()),
        GodotShape::TypedDictionary { key: _, value: _ } => ClassId::none(),
        GodotShape::Enum {
            variant_type: _,
            enumerators: _,
            godot_name,
            is_bitfield: _,
        } => godot_name
            .map(ClassId::new_dynamic)
            .unwrap_or_else(ClassId::none),
        GodotShape::Custom {
            variant_type: _,
            var_hint: _,
            export_hint: _,
            class_name,
            usage_flags: _,
            metadata: _,
        } => class_name
            .map(ClassId::new_dynamic)
            .unwrap_or_else(ClassId::none),
        _ => ClassId::none(),
    }
}
