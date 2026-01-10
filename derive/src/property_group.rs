/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use darling::{FromAttributes, FromDeriveInput};
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::{DeriveInput, parse_macro_input, spanned::Spanned};

use crate::{
    FieldExportOps, FieldOpts, attribute_ops::ScriptPropertyGroupOpts, godot_types,
    rust_to_variant_type,
};

#[inline(always)]
pub fn derive_property_group(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let opts = match ScriptPropertyGroupOpts::from_derive_input(&input) {
        Ok(opts) => opts,
        Err(err) => {
            return err.write_errors().into();
        }
    };

    let mut derive_errors = Vec::new();

    let fields: Vec<_> = opts
        .data
        .take_struct()
        .unwrap()
        .fields
        .into_iter()
        .filter_map(|field| {
            let get = derive_property_get(&field);
            let set = derive_property_set(&field);
            let metadata = match derive_property_metadata(&field) {
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

    let derive_errors: TokenStream = derive_errors.into_iter().collect();

    quote! {
        #derive_errors

        impl ScriptPropertyGroup for PropertyGroup {
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

            fn properties() -> ::godot_rust_script::PropertyGroupBuilder {
                ::godot_rust_script::PropertyGroupBuilder::new(Self::NAME, 2)
                    #metadata
            }

            fn export_property_states(
                &self,
                prefix: &'static str,
                state: &mut HashMap<StringName, godot::builtin::Variant>,
            ) {
                // export property states
                state.insert(
                    format!("{}_item1", prefix).as_str().into(),
                    self.item1.to_variant(),
                );
                state.insert(
                    format!("{}_item2", prefix).as_str().into(),
                    self.item2.to_variant(),
                );
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

fn derive_property_get(field: &FieldOpts) -> TokenStream {
    let field_ident = field.ident.as_ref().unwrap();
    let field_name = field_ident.to_string();

    quote_spanned! {
        field.ty.span() =>
        #field_name => ::godot_rust_script::GetScriptProperty::get_property(&self.#field_ident).to_variant(),
    }
}

fn derive_property_set(field: &FieldOpts) -> TokenStream {
    let field_ident = field.ident.as_ref().unwrap();
    let field_name = field_ident.to_string();

    quote_spanned! {
        field.ty.span() =>
        #field_name => ::godot_rust_script::SetScriptProperty::set_property(&mut self.#field_ident, FromGodot::try_from_variant(&value).unwrap()),
    }
}

fn derive_property_metadata(field: &FieldOpts) -> Result<TokenStream, TokenStream> {
    let godot_types = godot_types();

    let field_ident = field.ident.as_ref().unwrap();
    let field_ty = rust_to_variant_type(&field.ty)?;
    let rust_ty = &field.ty;
    let field_name = field_ident.to_string();
    let export_attr = field
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("export"));

    let export = export_attr
        .map(|attr| FieldExportOps::from_attributes(&field.attrs).map(|opts| (attr, opts)))
        .transpose()
        .map_err(|err| err.write_errors())?
        .map(|(attr, opts)| opts.to_export_meta(&field.ty, attr.meta.span()))
        .transpose()?;

    if let Some(export) = export.as_ref()
        && export.field.is_empty()
    {
        return Err(quote_spanned! { export_attr.unwrap().meta.span() =>
            compile_error!("Adding the #[export] attribute without additional parameters is useless inside ExportGroups");
        });
    }

    let hint = export
        .as_ref()
        .map(|exp| exp.hint.clone())
        .unwrap_or_else(|| quote!(PropertyHint::NONE));

    let hint_string = export
        .as_ref()
        .map(|exp| exp.hint_string.clone())
        .unwrap_or_else(|| quote!(String::new()));

    let usage = export.map(|exp| exp.usage).unwrap_or_else(|| {
        quote!(
            PropertyUsageFlags::SCRIPT_VARIABLE
                | PropertyUsageFlags::EDITOR
                | PropertyUsageFlags::STORAGE
        )
    });

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