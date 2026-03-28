/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use proc_macro2::TokenStream;
use quote::quote;

#[inline]
pub fn godot_types() -> TokenStream {
    quote!(::godot)
}

#[inline]
pub fn property_hints() -> TokenStream {
    let godot_types = godot_types();

    quote!(#godot_types::register::info::PropertyHint)
}

#[inline]
pub fn property_usage() -> TokenStream {
    let godot_types = godot_types();

    quote!(#godot_types::register::info::PropertyUsageFlags)
}

#[inline]
pub fn variant_ty() -> TokenStream {
    let godot_types = godot_types();

    quote!(#godot_types::prelude::Variant)
}

#[inline]
pub fn string_name_ty() -> TokenStream {
    let godot_types = godot_types();

    quote!(#godot_types::prelude::StringName)
}

#[inline]
pub fn convert_error_ty() -> TokenStream {
    let godot_types = godot_types();

    quote!(#godot_types::meta::error::ConvertError)
}

#[inline]
pub fn godot_shape() -> TokenStream {
    let godot_types = godot_types();

    quote!(#godot_types::meta::shape::GodotShape)
}
