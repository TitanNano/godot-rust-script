/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

mod attribute_ops;
mod enums;
mod impl_attribute;
mod type_paths;

use attribute_ops::{FieldOpts, GodotScriptOpts};
use darling::{util::SpannedValue, FromAttributes, FromDeriveInput};
use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned, ToTokens};
use syn::{parse_macro_input, spanned::Spanned, DeriveInput, Ident, Type};
use type_paths::{godot_types, property_hints, string_name_ty, variant_ty};

use crate::attribute_ops::{FieldExportOps, PropertyOpts};

#[proc_macro_derive(GodotScript, attributes(export, script, prop, signal))]
pub fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let opts = GodotScriptOpts::from_derive_input(&input).unwrap();

    let godot_types = godot_types();
    let variant_ty = variant_ty();
    let string_name_ty = string_name_ty();
    let call_error_ty = quote!(#godot_types::sys::GDExtensionCallErrorType);

    let base_class = opts
        .base
        .map(|ident| quote!(#ident))
        .unwrap_or_else(|| quote!(::godot_rust_script::godot::prelude::RefCounted));

    let script_type_ident = opts.ident;
    let class_name = script_type_ident.to_string();
    let fields = opts.data.take_struct().unwrap().fields;

    let (
        field_metadata,
        signal_metadata,
        get_fields_dispatch,
        set_fields_dispatch,
        export_field_state,
    ): (
        TokenStream,
        TokenStream,
        TokenStream,
        TokenStream,
        TokenStream,
    ) = fields
        .iter()
        .map(|field| {
            let signal_attr = field
                .attrs
                .iter()
                .find(|attr| attr.path().is_ident("signal"));
            let export_attr = field
                .attrs
                .iter()
                .find(|attr| attr.path().is_ident("export"));

            let is_public = matches!(field.vis, syn::Visibility::Public(_))
                || field.attrs.iter().any(|attr| attr.path().is_ident("prop"));
            let is_exported = export_attr.is_some();
            let is_signal = signal_attr.is_some();

            let field_metadata = match (is_public, is_exported, is_signal) {
                (false, false, _) | (true, false, true) => TokenStream::default(),
                (false, true, _) => {
                    let err = compile_error("Only public fields can be exported!", export_attr);

                    quote! {#err,}
                }
                (true, _, false) => {
                    derive_field_metadata(field, is_exported).unwrap_or_else(|err| err)
                }
                (true, true, true) => {
                    let err = compile_error("Signals can not be exported!", export_attr);

                    quote! {#err,}
                }
            };

            let get_field_dispatch = is_public.then(|| derive_get_field_dispatch(field));
            let set_field_dispatch =
                (is_public && !is_signal).then(|| derive_set_field_dispatch(field));
            let export_field_state =
                (is_public && !is_signal).then(|| derive_property_state_export(field));

            let signal_metadata = match (is_public, is_signal) {
                (false, false) | (true, false) => TokenStream::default(),
                (true, true) => derive_signal_metadata(field),
                (false, true) => {
                    let err = compile_error("Signals must be public!", signal_attr);

                    quote! {#err,}
                }
            };

            (
                field_metadata,
                signal_metadata,
                get_field_dispatch.to_token_stream(),
                set_field_dispatch.to_token_stream(),
                export_field_state.to_token_stream(),
            )
        })
        .multiunzip();

    let get_fields_impl = derive_get_fields(get_fields_dispatch);
    let set_fields_impl = derive_set_fields(set_fields_dispatch);
    let properties_state_impl = derive_property_states_export(export_field_state);
    let default_impl = derive_default_with_base(&fields);

    let description = opts
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .map(|attr| {
            attr.meta
                .require_name_value()
                .unwrap()
                .value
                .to_token_stream()
        })
        .reduce(|mut acc, lit| {
            acc.extend(quote!(,"\n",));
            acc.extend(lit);
            acc
        });

    let output = quote! {
        impl ::godot_rust_script::GodotScript for #script_type_ident {
            type Base = #base_class;

            const CLASS_NAME: &'static str = #class_name;

            #get_fields_impl

            #set_fields_impl

            fn call(&mut self, name: #string_name_ty, args: &[&#variant_ty], ctx: ::godot_rust_script::Context<Self>) -> ::std::result::Result<#variant_ty, #call_error_ty> {
                ::godot_rust_script::GodotScriptImpl::call_fn(self, name, args, ctx)
            }

            fn to_string(&self) -> String {
                format!("{:?}", self)
            }

            #properties_state_impl

            #default_impl
        }

        ::godot_rust_script::register_script_class!(
            #script_type_ident,
            #base_class,
            concat!(#description),
            vec![
                #field_metadata
            ],
            vec![
                #signal_metadata
            ]
        );

    };

    output.into()
}

fn rust_to_variant_type(ty: &syn::Type) -> Result<TokenStream, TokenStream> {
    use syn::Type as T;

    let godot_types = godot_types();

    match ty {
        T::Path(path) => Ok(quote_spanned! {
            ty.span() => {
                use #godot_types::sys::GodotFfi;
                use #godot_types::meta::GodotType;

                <<#path as #godot_types::meta::GodotConvert>::Via as GodotType>::Ffi::variant_type()
            }
        }),
        T::Verbatim(_) => Err(syn::Error::new(
            ty.span(),
            "not sure how to handle verbatim types yet!",
        )
        .into_compile_error()),
        T::Tuple(tuple) => {
            if !tuple.elems.is_empty() {
                return Err(syn::Error::new(
                    ty.span(),
                    format!("\"{}\" is not a supported type", quote!(#tuple)),
                )
                .into_compile_error());
            }

            Ok(quote_spanned! {
                tuple.span() => {
                    use #godot_types::sys::GodotFfi;
                    use #godot_types::meta::GodotType;

                    <<#tuple as #godot_types::meta::GodotConvert>::Via as GodotType>::Ffi::variant_type()
                }
            })
        }
        _ => Err(syn::Error::new(
            ty.span(),
            format!("\"{}\" is not a supported type", quote!(#ty)),
        )
        .into_compile_error()),
    }
}

fn is_context_type(ty: &syn::Type) -> bool {
    let syn::Type::Path(path) = ty else {
        return false;
    };

    path.path
        .segments
        .last()
        .map(|segment| segment.ident == "Context")
        .unwrap_or(false)
}

fn derive_default_with_base(field_opts: &[SpannedValue<FieldOpts>]) -> TokenStream {
    let godot_types = godot_types();
    let fields: TokenStream = field_opts
        .iter()
        .filter_map(|field| match field.ident.as_ref() {
            Some(ident) if *ident == "base" => {
                Some(quote_spanned!(ident.span() => #ident: base.clone().cast(),))
            },

            Some(ident) if field.attrs.iter().any(|attr| attr.path().is_ident("signal")) => {
                Some(quote_spanned!(ident.span() => #ident: ::godot_rust_script::ScriptSignal::new(base.clone(), stringify!(#ident)),))
            }

            Some(ident) => Some(quote_spanned!(ident.span() => #ident: Default::default(),)),
            None => None,
        })
        .collect();

    quote! {
        fn default_with_base(base: #godot_types::prelude::Gd<#godot_types::prelude::Object>) -> Self {
            Self {
                #fields
            }
        }
    }
}

fn derive_get_field_dispatch(field: &SpannedValue<FieldOpts>) -> TokenStream {
    let godot_types = godot_types();

    let field_ident = field.ident.as_ref().unwrap();
    let field_name = field_ident.to_string();

    let opts = match PropertyOpts::from_attributes(&field.attrs) {
        Ok(opts) => opts,
        Err(err) => return err.write_errors(),
    };

    let accessor = match opts.get {
        Some(getter) => quote_spanned!(getter.span()=> #getter(&self)),
        None => quote_spanned!(field_ident.span()=> self.#field_ident),
    };

    quote_spanned! {field.ty.span()=>
        #[allow(clippy::needless_borrow)]
        #field_name => Some(#godot_types::prelude::ToGodot::to_variant(&#accessor)),
    }
}

fn derive_get_fields(get_field_dispatch: TokenStream) -> TokenStream {
    let string_name_ty = string_name_ty();
    let variant_ty = variant_ty();

    quote! {
        fn get(&self, name: #string_name_ty) -> ::std::option::Option<#variant_ty> {
            match name.to_string().as_str() {
                #get_field_dispatch

                _ => None,
            }
        }
    }
}

fn derive_set_field_dispatch(field: &SpannedValue<FieldOpts>) -> TokenStream {
    let godot_types = godot_types();

    let field_ident = field.ident.as_ref().unwrap();
    let field_name = field_ident.to_string();

    let opts = match PropertyOpts::from_attributes(&field.attrs) {
        Ok(opts) => opts,
        Err(err) => return err.write_errors(),
    };

    let variant_value = quote_spanned!(field.ty.span()=> #godot_types::prelude::FromGodot::try_from_variant(&value));

    let assignment = match opts.set {
        Some(setter) => quote_spanned!(setter.span()=> #setter(self, local_value)),
        None => quote_spanned!(field.ty.span() => self.#field_ident = local_value),
    };

    quote! {
        #field_name => {
            let local_value = match #variant_value {
                Ok(v) => v,
                Err(_) => return false,
            };

            #assignment;
            true
        },
    }
}

fn derive_set_fields(set_field_dispatch: TokenStream) -> TokenStream {
    let string_name_ty = string_name_ty();
    let variant_ty = variant_ty();

    quote! {
        fn set(&mut self, name: #string_name_ty, value: #variant_ty) -> bool {
            match name.to_string().as_str() {
                #set_field_dispatch

                _ => false,
            }
        }
    }
}

fn derive_property_state_export(field: &SpannedValue<FieldOpts>) -> TokenStream {
    let string_name_ty = string_name_ty();

    let Some(ident) = field.ident.as_ref() else {
        return Default::default();
    };

    let field_name = ident.to_string();
    let field_string_name = quote!(#string_name_ty::from(#field_name));

    quote! {
        (#field_string_name, self.get(#field_string_name).unwrap()),
    }
}

fn derive_property_states_export(fetch_property_states: TokenStream) -> TokenStream {
    let string_name_ty = string_name_ty();
    let variant_ty = variant_ty();

    quote! {
        fn property_state(&self) -> ::std::collections::HashMap<#string_name_ty, #variant_ty> {
            ::std::collections::HashMap::from([
                #fetch_property_states
            ])
        }
    }
}

fn derive_field_metadata(
    field: &SpannedValue<FieldOpts>,
    is_exported: bool,
) -> Result<TokenStream, TokenStream> {
    let property_hint_ty = property_hints();
    let name = field
        .ident
        .as_ref()
        .map(|field| field.to_string())
        .unwrap_or_default();

    let ty = rust_to_variant_type(&field.ty)?;

    let (hint, hint_string) = is_exported
        .then(|| {
            let ops =
                FieldExportOps::from_attributes(&field.attrs).map_err(|err| err.write_errors())?;

            ops.hint(&field.ty)
        })
        .transpose()?
        .unwrap_or_else(|| {
            (
                quote_spanned!(field.span()=> #property_hint_ty::NONE),
                quote_spanned!(field.span()=> String::new()),
            )
        });

    let description = get_field_description(field);
    let item = quote! {
        ::godot_rust_script::private_export::RustScriptPropDesc {
            name: #name,
            ty: #ty,
            exported: #is_exported,
            hint: #hint,
            hint_string: #hint_string,
            description: concat!(#description),
        },
    };

    Ok(item)
}

fn get_field_description(field: &FieldOpts) -> Option<TokenStream> {
    field
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .map(|attr| {
            attr.meta
                .require_name_value()
                .unwrap()
                .value
                .to_token_stream()
        })
        .reduce(|mut acc, comment| {
            acc.extend(quote!(, "\n", ));
            acc.extend(comment);
            acc
        })
}

fn derive_signal_metadata(field: &SpannedValue<FieldOpts>) -> TokenStream {
    let signal_name = field
        .ident
        .as_ref()
        .map(|ident| ident.to_string())
        .unwrap_or_default();
    let signal_description = get_field_description(field);
    let signal_type = &field.ty;

    quote! {
        ::godot_rust_script::private_export::RustScriptSignalDesc {
            name: #signal_name,
            arguments: <#signal_type as ::godot_rust_script::ScriptSignal>::argument_desc(),
            description: concat!(#signal_description),
        },
    }
}

#[proc_macro_attribute]
pub fn godot_script_impl(
    args: proc_macro::TokenStream,
    body: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    impl_attribute::godot_script_impl(args, body)
}

fn compile_error(message: &str, tokens: impl ToTokens) -> TokenStream {
    syn::Error::new_spanned(tokens, message).into_compile_error()
}

fn extract_ident_from_type(impl_target: &syn::Type) -> Result<Ident, TokenStream> {
    match impl_target {
        Type::Array(_) => Err(compile_error("Arrays are not supported!", impl_target)),
        Type::BareFn(_) => Err(compile_error(
            "Bare functions are not supported!",
            impl_target,
        )),
        Type::Group(_) => Err(compile_error("Groups are not supported!", impl_target)),
        Type::ImplTrait(_) => Err(compile_error("Impl traits are not suppored!", impl_target)),
        Type::Infer(_) => Err(compile_error("Infer is not supported!", impl_target)),
        Type::Macro(_) => Err(compile_error("Macro types are not supported!", impl_target)),
        Type::Never(_) => Err(compile_error("Never type is not supported!", impl_target)),
        Type::Paren(_) => Err(compile_error("Unsupported type!", impl_target)),
        Type::Path(ref path) => Ok(path.path.segments.last().unwrap().ident.clone()),
        Type::Ptr(_) => Err(compile_error(
            "Pointer types are not supported!",
            impl_target,
        )),
        Type::Reference(_) => Err(compile_error("References are not supported!", impl_target)),
        Type::Slice(_) => Err(compile_error("Slices are not supported!", impl_target)),
        Type::TraitObject(_) => Err(compile_error(
            "Trait objects are not supported!",
            impl_target,
        )),
        Type::Tuple(_) => Err(compile_error("Tuples are not supported!", impl_target)),
        Type::Verbatim(_) => Err(compile_error("Verbatim is not supported!", impl_target)),
        _ => Err(compile_error("Unsupported type!", impl_target)),
    }
}

#[proc_macro_derive(GodotScriptEnum, attributes(script_enum))]
pub fn script_enum_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    enums::script_enum_derive(input)
}
