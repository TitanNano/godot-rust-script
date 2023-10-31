use proc_macro2::TokenStream;
use quote::quote;

pub fn godot_types() -> TokenStream {
    quote!(::godot_rust_script::godot)
}

pub fn property_hints() -> TokenStream {
    let godot_types = godot_types();

    quote!(#godot_types::engine::global::PropertyHint)
}

pub fn variant_ty() -> TokenStream {
    let godot_types = godot_types();

    quote!(#godot_types::prelude::Variant)
}

pub fn string_name_ty() -> TokenStream {
    let godot_types = godot_types();

    quote!(#godot_types::prelude::StringName)
}
