/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use darling::{FromAttributes, FromDeriveInput};
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{DeriveInput, parse_macro_input, spanned::Spanned};

use crate::{
    FieldExportOps, FieldOpts, attribute_ops::ScriptPropertyGroupOpts, godot_types,
    rust_to_variant_type,
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

            Some(DerivedProperty { get, set, metadata })
        })
        .collect();

    let getters: TokenStream = fields.iter().map(|field| &field.get).cloned().collect();
    let setters: TokenStream = fields.iter().map(|field| &field.set).cloned().collect();
    let metadata: TokenStream = fields
        .iter()
        .map(|field| &field.metadata)
        .cloned()
        .collect();

    let derive_errors = derive_errors.finish().err().map(|err| err.write_errors());
    let type_ident = &opts.ident;
    let builder = if flatten_subgroup {
        quote!(PropertyGroupBuilder)
    } else {
        quote!(PropertySubgroupBuilder)
    };

    quote! {
        #derive_errors

        impl #ident for #type_ident {
            const NAME: &'static str = "Property Group";

            fn get_property(&self, name: &str) -> godot::builtin::Variant {
                match name {
                    #getters
                    _ => Variant::nil(),
                }
            }

            fn set_property(&mut self, name: &str, value: godot::builtin::Variant) {
                match name {
                    #setters
                    _ => (),
                }
            }

            fn properties() -> ::godot_rust_script::#builder {
                ::godot_rust_script::#builder::new(Self::NAME, 2)
                    #metadata
            }

            fn export_property_states(
                &self,
                prefix: &'static str,
                state: &mut HashMap<StringName, godot::builtin::Variant>,
            ) {
                // export property states
                // state.insert(
                //     format!("{}_item1", prefix).as_str().into(),
                //     self.item1.to_variant(),
                // );
                // state.insert(
                //     format!("{}_item2", prefix).as_str().into(),
                //     self.item2.to_variant(),
                // );
            }
        }
    }
    .into()
}

struct DerivedProperty {
    get: TokenStream,
    set: TokenStream,
    metadata: TokenStream,
}

fn derive_property_get(field: &FieldOpts, flatten_subgroup: bool, is_flatten: bool) -> TokenStream {
    let field_ident = field.ident.as_ref().unwrap();
    let field_name = field_ident.to_string();
    let rust_ty = &field.ty;

    if flatten_subgroup && is_flatten {
        return quote_spanned! {
            field.ty.span() =>
            field_name if field_name.starts_with(concat!(#field_name, "_")) => <#rust_ty as ::godot_rust_script::ScriptPropertySubgroup>::get_property(
                &self.#field_ident,
                field_name.trim_start_matches(concat!(#field_name, "_")),
            ).to_variant(),
        };
    }

    quote_spanned! {
        field.ty.span() =>
        #field_name => ::godot_rust_script::GetScriptProperty::get_property(&self.#field_ident).to_variant(),
    }
}

fn derive_property_set(field: &FieldOpts, flatten_subgroup: bool, is_flatten: bool) -> TokenStream {
    let field_ident = field.ident.as_ref().unwrap();
    let field_name = field_ident.to_string();
    let rust_ty = &field.ty;

    if flatten_subgroup && is_flatten {
        return quote_spanned! {
            field.ty.span() =>
            field_name if field_name.starts_with(concat!(#field_name, "_")) => <#rust_ty as ::godot_rust_script::ScriptPropertySubgroup>::set_property(
                &mut self.#field_ident,
                field_name.trim_start_matches(concat!(#field_name, "_")),
                FromGodot::try_from_variant(&value).unwrap()
            ),
        };
    }

    quote_spanned! {
        field.ty.span() =>
        #field_name => ::godot_rust_script::SetScriptProperty::set_property(&mut self.#field_ident, FromGodot::try_from_variant(&value).unwrap()),
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
                <#rust_ty as ::godot_rust_script::ScriptPropertySubgroup>::properties()
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

fn export_config(field: &FieldOpts, span: Span) -> Result<FieldExportOps, darling::Error> {
    FieldExportOps::from_attributes(&field.attrs).map_err(|err| err.with_span(&span))
}
