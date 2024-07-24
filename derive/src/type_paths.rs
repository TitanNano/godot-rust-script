/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use proc_macro2::TokenStream;
use quote::quote;

pub fn godot_types() -> TokenStream {
    quote!(::godot_rust_script::godot)
}

pub fn property_hints() -> TokenStream {
    let godot_types = godot_types();

    quote!(#godot_types::global::PropertyHint)
}

pub fn variant_ty() -> TokenStream {
    let godot_types = godot_types();

    quote!(#godot_types::prelude::Variant)
}

pub fn string_name_ty() -> TokenStream {
    let godot_types = godot_types();

    quote!(#godot_types::prelude::StringName)
}
