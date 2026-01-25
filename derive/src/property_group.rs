/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use darling::{FromAttributes, FromDeriveInput, util::SpannedValue};
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{DeriveInput, parse_macro_input, spanned::Spanned};

use crate::{
    FieldExportOps, FieldOpts, attribute_ops::ScriptPropertyGroupOpts, godot_types,
    rust_to_variant_type, string_name_ty,
};

trait FlattenTuple<F> {
    fn flatten(self) -> F;
}

macro_rules! flatten_tuples {
    ($($items:ident),+; $last:ident) => {
        impl<$($items,)+ $last> FlattenTuple<($($items,)+ $last)> for (($($items,)+), $last) {
            fn flatten(self) -> ($($items,)+ $last) {
                #[allow(non_snake_case)]
                let (($($items,)+), $last) = self;

                ($($items,)+ $last)
            }
        }
    };
}

flatten_tuples!(T1; T2);
flatten_tuples!(T1, T2; T3);
flatten_tuples!(T1, T2, T3; T4);
flatten_tuples!(T1, T2, T3, T4; T5);
flatten_tuples!(T1, T2, T3, T4, T5; T6);
flatten_tuples!(T1, T2, T3, T4, T5, T6; T7);

macro_rules! merge_errors {
    ($first_result:expr, $($result:expr),+) => {
        {
            let mut errors = darling::Error::accumulator();

            let values = $first_result.map_err(|err| errors.push(err)).ok().map(|val| (val,))
                $(.zip(
                    $result.map_err(|err| errors.push(err)).ok()
                ).map(FlattenTuple::flatten))+;

            errors.finish_with(values).transpose().unwrap()
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum PropertyGroupType {
    Group,
    Subgroup,
}

impl PropertyGroupType {
    fn trait_ident(self) -> syn::Ident {
        match self {
            PropertyGroupType::Group => syn::Ident::new("ScriptExportGroup", Span::call_site()),
            PropertyGroupType::Subgroup => {
                syn::Ident::new("ScriptExportSubgroup", Span::call_site())
            }
        }
    }
}

#[inline(always)]
pub fn derive_property_group(
    ident: syn::Ident,
    input: proc_macro::TokenStream,
    flatten_subgroup: bool,
) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let opts = match ScriptPropertyGroupOpts::from_derive_input(&input) {
        Ok(opts) => opts,
        Err(err) => {
            return err.write_errors().into();
        }
    };

    let mut derive_errors = darling::Error::accumulator();

    let fields: Vec<_> = opts
        .data
        .take_struct()
        .unwrap()
        .fields
        .into_iter()
        .filter_map(|field| {
            let primary_export_attr = field
                .attrs
                .iter()
                .find(|attr| attr.path().is_ident("export"));

            let export_config = match export_config(
                &field,
                primary_export_attr
                    .map(|attr| attr.meta.span())
                    .unwrap_or_else(|| field.span()),
            ) {
                Ok(export) => export,
                Err(err) => {
                    derive_errors.push(err);
                    return None;
                }
            };

            let get = derive_property_get(&field, flatten_subgroup, export_config.is_flatten());
            let set = derive_property_set(&field, flatten_subgroup, export_config.is_flatten());
            let metadata = match derive_property_metadata(
                &field,
                &export_config,
                primary_export_attr,
                flatten_subgroup,
            ) {
                Ok(metadata) => metadata,
                Err(err) => {
                    derive_errors.push(err);
                    return None;
                }
            };
            let prop_export =
                derive_property_state_export(&field, flatten_subgroup, export_config.is_flatten());

            Some(DerivedProperty {
                get,
                set,
                metadata,
                prop_export,
            })
        })
        .collect();

    let prop_count = fields.len();
    let getters: TokenStream = fields.iter().map(|field| &field.get).cloned().collect();
    let setters: TokenStream = fields.iter().map(|field| &field.set).cloned().collect();
    let metadata: TokenStream = fields
        .iter()
        .map(|field| &field.metadata)
        .cloned()
        .collect();
    let prop_export: TokenStream = fields
        .iter()
        .map(|field| &field.prop_export)
        .cloned()
        .collect();

    let derive_errors = derive_errors.finish().err().map(|err| err.write_errors());
    let type_ident = &opts.ident;
    let builder = if flatten_subgroup {
        quote!(ExportGroupBuilder)
    } else {
        quote!(ExportSubgroupBuilder)
    };

    quote! {
        #derive_errors

        #[automatically_derived]
        impl #ident for #type_ident {
            const NAME: &'static str = "Property Group";

            fn get_property(&self, name: &str) -> ::std::option::Option<::godot::builtin::Variant> {
                match name {
                    #getters
                    _ => None,
                }
            }

            fn set_property(&mut self, name: &str, value: ::godot::builtin::Variant) -> bool {
                match name {
                    #setters
                    _ => false,
                }
            }

            fn properties() -> ::godot_rust_script::#builder {
                ::godot_rust_script::#builder::new(#prop_count)
                    #metadata
            }

            #[allow(clippy::needless_borrow)]
            fn export_property_states(
                &self,
                prefix: &'static str,
                mut state: &mut ::std::collections::HashMap<::godot::builtin::StringName, ::godot::builtin::Variant>,
            ) {
                #prop_export
            }
        }
    }
    .into()
}

struct DerivedProperty {
    get: TokenStream,
    set: TokenStream,
    metadata: TokenStream,
    prop_export: TokenStream,
}

fn derive_property_get(field: &FieldOpts, flatten_subgroup: bool, is_flatten: bool) -> TokenStream {
    let field_ident = field.ident.as_ref().unwrap();
    let field_name = field_ident.to_string();
    let rust_ty = &field.ty;

    if flatten_subgroup && is_flatten {
        return dispatch_property_group_get(PropertyGroupType::Subgroup, field_ident, rust_ty);
    }

    quote_spanned! {
        field.ty.span() =>
        #field_name => ::std::option::Option::Some(::godot::meta::ToGodot::to_variant(&::godot_rust_script::GetScriptProperty::get_property(&self.#field_ident))),
    }
}

fn derive_property_set(field: &FieldOpts, flatten_subgroup: bool, is_flatten: bool) -> TokenStream {
    let field_ident = field.ident.as_ref().unwrap();
    let field_name = field_ident.to_string();
    let rust_ty = &field.ty;

    if flatten_subgroup && is_flatten {
        return dispatch_property_group_set(PropertyGroupType::Subgroup, field_ident, rust_ty);
    }

    quote_spanned! {
        field.ty.span() =>
        #field_name => {
            ::godot_rust_script::SetScriptProperty::set_property(&mut self.#field_ident, ::godot::meta::FromGodot::try_from_variant(&value).unwrap());
            true
        },
    }
}

fn derive_property_metadata(
    field: &FieldOpts,
    export_config: &FieldExportOps,
    primary_export_attr: Option<&syn::Attribute>,
    flatten_subgroup: bool,
) -> Result<TokenStream, darling::Error> {
    let godot_types = godot_types();
    let mut errors = darling::Error::accumulator();

    let field_ident = field.ident.as_ref().unwrap();
    let field_ty = rust_to_variant_type(&field.ty);
    let rust_ty = &field.ty;
    let field_name = field_ident.to_string();
    let export_meta = export_config.to_export_meta(
        &field.ty,
        primary_export_attr
            .map(|attr| attr.meta.span())
            .unwrap_or_else(|| field.ident.span()),
    );

    if let Some(attr) = primary_export_attr
        && let Ok(export_meta) = &export_meta
    {
        if export_meta.field.is_empty() && !export_config.is_flatten() {
            errors.push(darling::Error::custom(
                "Adding the #[export] attribute without additional parameters is useless inside ScriptExportGroups",
            ).with_span(&attr.meta.span()));
        }

        if !flatten_subgroup && let Some(span) = export_config.flatten_span() {
            errors
                .push(darling::Error::custom("Flattening is not supported here").with_span(&span));
        }
    }

    let (field_ty, export_meta, _) = merge_errors!(field_ty, export_meta, errors.finish())?;

    if flatten_subgroup && export_config.is_flatten() {
        return Ok(quote_spanned! {
            field.ty.span() =>
            .add_subgroup(
                #field_name,
                "",
                <#rust_ty as ::godot_rust_script::ScriptExportSubgroup>::properties()
            )
        });
    }

    let hint = export_meta.hint;
    let hint_string = export_meta.hint_string;
    let usage = export_meta.usage;

    Ok(quote_spanned! {
        field.ty.span() =>
        .add_property(::godot_rust_script::private_export::RustScriptPropDesc {
            class_name: <<#rust_ty as #godot_types::meta::GodotConvert>::Via as #godot_types::meta::GodotType>::class_id(),
            name: #field_name.into(),
            ty: #field_ty,
            hint: #hint,
            usage: #usage,
            hint_string: #hint_string,
            description: "",
        })
    })
}

fn derive_property_state_export(
    field: &SpannedValue<FieldOpts>,
    flatten_subgroup: bool,
    is_flatten: bool,
) -> TokenStream {
    if flatten_subgroup && is_flatten {
        return dispatch_property_group_state_export(PropertyGroupType::Subgroup, field);
    }

    let string_name_ty = string_name_ty();
    let field_name = field.ident.as_ref().unwrap().to_string();
    let field_string_name = quote!(#string_name_ty::from(#field_name));

    quote_spanned! { field.span() =>
        state.insert(#field_string_name, self.get_property(#field_name).unwrap());
    }
}

fn export_config(field: &FieldOpts, span: Span) -> Result<FieldExportOps, darling::Error> {
    FieldExportOps::from_attributes(&field.attrs).map_err(|err| err.with_span(&span))
}

pub(crate) fn dispatch_property_group_set(
    group_ty: PropertyGroupType,
    field_ident: &syn::Ident,
    field_ty: &syn::Type,
) -> TokenStream {
    let field_name = field_ident.to_string();
    let trait_ident = group_ty.trait_ident();

    quote_spanned! { field_ty.span() =>
        field_name if field_name.starts_with(concat!(#field_name, "_")) => {
            <#field_ty as ::godot_rust_script::#trait_ident>::set_property(
                &mut self.#field_ident,
                field_name.strip_prefix(concat!(#field_name, "_")).unwrap(),
                value,
            )
        }
    }
}

pub(crate) fn dispatch_property_group_get(
    group_ty: PropertyGroupType,
    field_ident: &syn::Ident,
    field_ty: &syn::Type,
) -> TokenStream {
    let field_name = field_ident.to_string();
    let trait_ident = group_ty.trait_ident();

    quote_spanned! {
        field_ty.span() =>
        field_name if field_name.starts_with(concat!(#field_name, "_")) => <#field_ty as ::godot_rust_script::#trait_ident>::get_property(
            &self.#field_ident,
            field_name.trim_start_matches(concat!(#field_name, "_")),
        ),
    }
}

pub(crate) fn dispatch_property_group_state_export(
    group_ty: PropertyGroupType,
    field: &SpannedValue<FieldOpts>,
) -> TokenStream {
    let trait_ident = group_ty.trait_ident();

    let field_ident = &field.ident.as_ref().unwrap();
    let field_name = field_ident.to_string();

    quote_spanned! { field.span() =>
        ::godot_rust_script::#trait_ident::export_property_states(&self.#field_ident, #field_name, &mut state);
    }
}
