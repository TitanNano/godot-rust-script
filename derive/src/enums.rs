/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use darling::{
    ast::Data,
    util::{Ignored, WithOriginal},
    FromDeriveInput, FromVariant,
};
use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::{parse_macro_input, spanned::Spanned, DeriveInput, Ident, Meta, Visibility};

use crate::type_paths::{convert_error_ty, godot_types, property_hints};

#[derive(FromDeriveInput)]
#[darling(supports(enum_unit), attributes(script_enum))]
struct EnumDeriveInput {
    vis: Visibility,
    ident: Ident,
    export: Option<WithOriginal<(), Meta>>,
    data: Data<EnumVariant, Ignored>,
}

#[derive(FromVariant)]
struct EnumVariant {
    ident: Ident,
}

pub fn script_enum_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let godot_types = godot_types();
    let convert_error = convert_error_ty();
    let property_hints = property_hints();

    let input = parse_macro_input!(input as DeriveInput);
    let input = EnumDeriveInput::from_derive_input(&input).unwrap();

    let enum_ident = input.ident;
    let enum_as_try_from = quote_spanned! {enum_ident.span()=> <#enum_ident as TryFrom<Self::Via>>};
    let enum_from_self = quote_spanned! {enum_ident.span()=> <Self::Via as From<&#enum_ident>>};
    let enum_error_ident = Ident::new(&format!("{}Error", enum_ident), enum_ident.span());
    let enum_visibility = input.vis;

    let variants = input.data.take_enum().unwrap();

    let (from_variants, into_variants, hint_strings): (TokenStream, TokenStream, Vec<_>) = variants
        .iter()
        .enumerate()
        .map(|(index, variant)| {
            let variant_ident = &variant.ident;
            let index = index as u8;

            (
                quote_spanned! {variant_ident.span()=> #enum_ident::#variant_ident => #index,},
                quote_spanned! {variant_ident.span()=> #index => Ok(#enum_ident::#variant_ident),},
                format!("{variant_ident}:{index}"),
            )
        })
        .multiunzip();
    let enum_property_hint_str = hint_strings.join(",");

    let derive_export = input.export.map(|export| {
        quote_spanned! {export.original.span()=>
            impl ::godot_rust_script::GodotScriptExport for #enum_ident {
                fn hint(custom: Option<#property_hints>) -> #property_hints {
                    if let Some(custom) = custom {
                        return custom;
                    }

                    #property_hints::ENUM
                }

                fn hint_string(_custom_hint: Option<#property_hints>, custom_string: Option<String>) -> String {
                    if let Some(custom_string) = custom_string {
                        return custom_string;
                    }

                    String::from(#enum_property_hint_str)
                }
            }
        }
    });

    let derived = quote! {
        impl #godot_types::meta::FromGodot for #enum_ident {
            fn try_from_godot(via: Self::Via) -> Result<Self, #convert_error> {
                #enum_as_try_from::try_from(via)
                    .map_err(|err| #convert_error::with_error_value(err, via))
            }
        }

        impl #godot_types::meta::ToGodot for #enum_ident {
            type Pass = ::godot::meta::ByValue;

            fn to_godot(&self) -> Self::Via {
                #enum_from_self::from(self)
            }
        }

        impl #godot_types::meta::GodotConvert for #enum_ident {
            type Via = u8;
        }

        impl GodotScriptEnum for #enum_ident {}

        impl From<&#enum_ident> for u8 {
            fn from(value: &#enum_ident) -> Self {
                match value {
                    #from_variants
                }
            }
        }

        #[derive(Debug)]
        #enum_visibility struct #enum_error_ident(u8);

        impl ::std::fmt::Display for #enum_error_ident {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "Enum value {} is out of range.", self.0)
            }
        }

        impl ::std::error::Error for #enum_error_ident {}

        impl TryFrom<u8> for #enum_ident {
            type Error = #enum_error_ident;

            fn try_from(value: u8) -> ::std::result::Result<Self, Self::Error> {
                match value {
                    #into_variants
                    _ => Err(#enum_error_ident(value)),
                }
            }
        }

        impl #godot_types::prelude::Var for #enum_ident {
            fn get_property(&self) -> Self::Via {
                self.into()
            }

            fn set_property(&mut self, value: Self::Via) {
                *self = #godot_types::meta::FromGodot::try_from_godot(value).unwrap();
            }
        }

        #derive_export
    };

    derived.into()
}
