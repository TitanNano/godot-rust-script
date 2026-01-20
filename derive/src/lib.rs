/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

mod attribute_ops;
mod enums;
mod impl_attribute;
mod property_group;
mod type_paths;

use darling::{FromAttributes, FromDeriveInput, FromMeta, util::SpannedValue};
use itertools::Itertools;
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote, quote_spanned};
use syn::{DeriveInput, Ident, Type, parse_macro_input, spanned::Spanned};

use crate::attribute_ops::{
    ExportGroup, ExportMetadata, ExportSubgroup, FieldExportOps, FieldOpts, FieldSignalOps,
    GodotScriptOpts, PropertyOpts,
};
use crate::property_group::{
    dispatch_property_group_get, dispatch_property_group_set, dispatch_property_group_state_export,
};
use crate::type_paths::{godot_types, property_hints, property_usage, string_name_ty, variant_ty};

#[proc_macro_derive(
    GodotScript,
    attributes(export, export_group, export_subgroup, script, prop, signal)
)]
pub fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let opts = match GodotScriptOpts::from_derive_input(&input) {
        Ok(opts) => opts,
        Err(err) => {
            return err.write_errors().into();
        }
    };

    let godot_types = godot_types();
    let variant_ty = variant_ty();
    let string_name_ty = string_name_ty();
    let call_error_ty = quote!(#godot_types::meta::error::CallErrorType);

    let base_class = opts
        .base
        .map(|ident| quote!(#ident))
        .unwrap_or_else(|| quote!(::godot_rust_script::godot::prelude::RefCounted));

    let script_type_ident = opts.ident;
    let class_name = script_type_ident.to_string();
    let is_tool = opts.tool.is_some();
    let fields = opts.data.take_struct().unwrap().fields;

    let (
        (field_metadata, field_errors),
        signal_metadata,
        get_fields_dispatch,
        set_fields_dispatch,
        export_field_state,
    ): (
        (TokenStream, TokenStream),
        (TokenStream, TokenStream),
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
            let is_signal = signal_attr.is_some();
            let export_ops = export_attr
                .is_some()
                .then(|| {
                    let ops = FieldExportOps::from_attributes(&field.attrs)
                        .map_err(|err| err.write_errors())?;

                    Result::<FieldExportOps, TokenStream>::Ok(ops)
                })
                .transpose()
                .unwrap();
            let is_flatten = export_ops
                .as_ref()
                .map(|ops| ops.is_flatten())
                .unwrap_or(false);

            let field_metadata = match (is_public, export_ops, is_signal) {
                (false, None, _) | (true, None, true) => {
                    (TokenStream::default(), TokenStream::default())
                }
                (false, Some(_), _) => {
                    let err = compile_error("Only public fields can be exported!", export_attr);

                    (TokenStream::default(), err)
                }
                (true, export_ops, false) => derive_field_metadata(field, export_ops)
                    .map(|tokens| (tokens, TokenStream::default()))
                    .unwrap_or_else(|err| (TokenStream::default(), err)),
                (true, Some(_), true) => {
                    let err = compile_error("Signals can not be exported!", export_attr);

                    (TokenStream::default(), err)
                }
            };

            let get_field_dispatch =
                is_public.then(|| derive_get_field_dispatch(field, is_flatten));
            let set_field_dispatch =
                (is_public && !is_signal).then(|| derive_set_field_dispatch(field, is_flatten));
            let export_field_state =
                (is_public && !is_signal).then(|| derive_property_state_export(field, is_flatten));

            let signal_metadata = match (is_public, is_signal) {
                (false, false) | (true, false) => (TokenStream::default(), TokenStream::default()),
                (true, true) => derive_signal_metadata(field),
                (false, true) => {
                    let err = compile_error("Signals must be public!", signal_attr);

                    (err, TokenStream::default())
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

    let (signal_metadata, signal_const_assert) = signal_metadata;

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

        #signal_const_assert
        #field_errors

        ::godot_rust_script::register_script_class!(
            #script_type_ident,
            #base_class,
            concat!(#description),
            #is_tool,
            builder => {
                #field_metadata
                #signal_metadata
            }
        );

    };

    output.into()
}

pub(crate) fn rust_to_variant_type(ty: &syn::Type) -> Result<TokenStream, darling::Error> {
    use syn::Type as T;

    let godot_types = godot_types();

    match ty {
        T::Path(path) => Ok(quote_spanned! {
            ty.span() => {
                use #godot_types::sys::GodotFfi;
                use #godot_types::meta::GodotType;

                <<#path as #godot_types::meta::GodotConvert>::Via as GodotType>::Ffi::VARIANT_TYPE.variant_as_nil()
            }
        }),
        T::Verbatim(_) => Err(
            darling::Error::custom("not sure how to handle verbatim types yet!")
                .with_span(&ty.span()),
        ),
        T::Tuple(tuple) => {
            if !tuple.elems.is_empty() {
                return Err(darling::Error::custom(format!(
                    "\"{}\" is not a supported type",
                    quote!(#tuple)
                ))
                .with_span(&ty.span()));
            }

            Ok(quote_spanned! {
                tuple.span() => {
                    use #godot_types::sys::GodotFfi;
                    use #godot_types::meta::GodotType;

                    <<#tuple as #godot_types::meta::GodotConvert>::Via as GodotType>::Ffi::VARIANT_TYPE.variant_as_nil()
                }
            })
        }
        _ => Err(
            darling::Error::custom(format!("\"{}\" is not a supported type", quote!(#ty)))
                .with_span(&ty.span()),
        ),
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

fn derive_get_field_dispatch(field: &SpannedValue<FieldOpts>, is_flatten: bool) -> TokenStream {
    let godot_types = godot_types();

    let field_ident = field.ident.as_ref().unwrap();
    let field_name = field_ident.to_string();

    let opts = match PropertyOpts::from_attributes(&field.attrs) {
        Ok(opts) => opts,
        Err(err) => return err.write_errors(),
    };

    if is_flatten {
        return dispatch_property_group_get(
            property_group::PropertyGroupType::Group,
            field_ident,
            &field.ty,
        );
    }

    let accessor = match opts.get {
        Some(getter) => quote_spanned!(getter.span()=> #getter(&self)),
        None => quote_spanned!(field_ident.span()=> self.#field_ident),
    };

    quote_spanned! {field.ty.span()=>
        #[allow(clippy::needless_borrow)]
        #field_name => Some(#godot_types::prelude::ToGodot::to_variant(&::godot_rust_script::GetScriptProperty::get_property(&#accessor))),
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

fn derive_set_field_dispatch(field: &SpannedValue<FieldOpts>, is_flatten: bool) -> TokenStream {
    let godot_types = godot_types();

    let field_ident = field.ident.as_ref().unwrap();
    let field_name = field_ident.to_string();

    let opts = match PropertyOpts::from_attributes(&field.attrs) {
        Ok(opts) => opts,
        Err(err) => return err.write_errors(),
    };

    if is_flatten {
        return dispatch_property_group_set(
            property_group::PropertyGroupType::Group,
            field_ident,
            &field.ty,
        );
    }

    let variant_value = quote_spanned!(field.ty.span()=> #godot_types::prelude::FromGodot::try_from_variant(&value));

    let assignment = match opts.set {
        Some(setter) => quote_spanned!(setter.span()=> #setter(self, local_value)),
        None => {
            quote_spanned!(field.ty.span() => ::godot_rust_script::SetScriptProperty::set_property(&mut self.#field_ident, local_value))
        }
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

fn derive_property_state_export(field: &SpannedValue<FieldOpts>, is_flatten: bool) -> TokenStream {
    if is_flatten {
        return dispatch_property_group_state_export(
            property_group::PropertyGroupType::Group,
            field,
        );
    }

    dispatch_property_export(field)
}

fn derive_property_states_export(fetch_property_states: TokenStream) -> TokenStream {
    let string_name_ty = string_name_ty();
    let variant_ty = variant_ty();

    quote! {
        fn property_state(&self) -> ::std::collections::HashMap<#string_name_ty, #variant_ty> {
            let mut state = ::std::collections::HashMap::new();

            #fetch_property_states

            state
        }
    }
}

fn derive_field_metadata(
    field: &SpannedValue<FieldOpts>,
    export_ops: Option<FieldExportOps>,
) -> Result<TokenStream, TokenStream> {
    let godot_types = godot_types();
    let property_hint_ty = property_hints();
    let property_usage_ty = property_usage();
    let name = field
        .ident
        .as_ref()
        .map(|field| field.to_string())
        .unwrap_or_default();

    let rust_ty = &field.ty;
    let ty = rust_to_variant_type(&field.ty).map_err(|err| err.write_errors())?;
    let group = derive_export_group(field).transpose()?;
    let subgroup = derive_export_subgroup(field).transpose()?;

    if let Some(ref export_ops) = export_ops
        && export_ops.is_flatten()
    {
        return Ok(quote_spanned! { field.span() =>
            builder.add_property_group(<#rust_ty as ::godot_rust_script::ScriptPropertyGroup>::properties().build(concat!(#name, "_"), ""));
        });
    }

    let ExportMetadata {
        field: _,
        usage,
        hint,
        hint_string,
    } = export_ops
        .map(|ops| {
            let span = field
                .attrs
                .iter()
                .find(|attr| attr.path().is_ident("export"))
                .expect("FieldExportOps already succeded")
                .span();

            ops.to_export_meta(&field.ty, span)
        })
        .transpose()
        .map_err(|err| err.write_errors())?
        .unwrap_or_else(|| ExportMetadata {
            field: "",
            usage: quote_spanned!(field.span() => #property_usage_ty::SCRIPT_VARIABLE),
            hint: quote_spanned!(field.span()=> #property_hint_ty::NONE),
            hint_string: quote_spanned!(field.span()=> String::new()),
        });

    let description = get_field_description(field);
    let item = quote_spanned! { field.span() =>
        #group
        #subgroup
        builder.add_property(::godot_rust_script::private_export::RustScriptPropDesc {
            name: #name.into(),
            ty: #ty,
            class_name: <<#rust_ty as #godot_types::meta::GodotConvert>::Via as #godot_types::meta::GodotType>::class_id(),
            usage: #usage,
            hint: #hint,
            hint_string: #hint_string,
            description: concat!(#description),
        });
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

fn derive_signal_metadata(field: &SpannedValue<FieldOpts>) -> (TokenStream, TokenStream) {
    let signal_name = field
        .ident
        .as_ref()
        .map(|ident| ident.to_string())
        .unwrap_or_default();
    let signal_description = get_field_description(field);
    let signal_type = &field.ty;
    let signal_ops = match field
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("signal"))
        .and_then(|attr| match &attr.meta {
            syn::Meta::Path(_) => None,
            syn::Meta::List(_) => Some(FieldSignalOps::from_meta(&attr.meta)),
            syn::Meta::NameValue(_) => Some(Err(darling::Error::custom(
                "Signal attribute does not support assigning a value!",
            )
            .with_span(&attr.meta))),
        })
        .transpose()
    {
        Ok(ops) => ops,
        Err(err) => return (TokenStream::default(), err.write_errors()),
    };

    let const_assert = signal_ops.as_ref().map(|ops| {
        let count = ops.0.parsed.len();

        quote_spanned! { ops.0.original.span() =>
            const _: () = {
                assert!(<#signal_type>::ARG_COUNT == #count as u8, "argument names do not match number of arguments.");
            };
        }
    });

    let argument_names = signal_ops
        .map(|names| {
            let span = names.0.original.span();
            #[expect(unstable_name_collisions)]
            let names: TokenStream = names
                .0
                .parsed
                .iter()
                .map(|name| name.to_token_stream())
                .intersperse(quote!(,).into_token_stream())
                .collect();

            quote_spanned! { span =>  Some(&[#names]) }
        })
        .unwrap_or_else(|| quote!(None));

    let metadata = quote! {
        builder.add_signal(::godot_rust_script::private_export::RustScriptSignalDesc {
            name: #signal_name,
            arguments: <#signal_type>::argument_desc(#argument_names),
            description: concat!(#signal_description),
        });
    };

    (metadata, const_assert.unwrap_or_default())
}

fn derive_export_group(
    field: &SpannedValue<FieldOpts>,
) -> Option<Result<TokenStream, TokenStream>> {
    let godot_types = godot_types();
    let property_usage_ty = property_usage();
    let property_hint_ty = property_hints();

    field
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("export_group"))
        .map(|first_attr| {
            let group =
                ExportGroup::from_attributes(&field.attrs).map_err(|err| err.write_errors())?;

            let group_name = group.name;

            Ok(quote_spanned! { first_attr.span() =>
                builder.add_property(::godot_rust_script::private_export::RustScriptPropDesc {
                    name: #group_name.into(),
                    ty: #godot_types::sys::VariantType::NIL,
                    class_name: #godot_types::meta::ClassId::none(),
                    usage: #property_usage_ty::GROUP,
                    hint: #property_hint_ty::NONE,
                    hint_string: String::new(),
                    description: "",
                });
            })
        })
}

fn derive_export_subgroup(
    field: &SpannedValue<FieldOpts>,
) -> Option<Result<TokenStream, TokenStream>> {
    let godot_types = godot_types();
    let property_usage_ty = property_usage();
    let property_hint_ty = property_hints();

    field
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("export_subgroup"))
        .map(|first_attr| {
            let group =
                ExportSubgroup::from_attributes(&field.attrs).map_err(|err| err.write_errors())?;

            let group_name = group.name;

            Ok(quote_spanned! { first_attr.span() =>
                builder.add_property(::godot_rust_script::private_export::RustScriptPropDesc {
                    name: #group_name.into(),
                    ty: #godot_types::sys::VariantType::NIL,
                    class_name: #godot_types::meta::ClassId::none(),
                    usage: #property_usage_ty::SUBGROUP,
                    hint: #property_hint_ty::NONE,
                    hint_string: String::new(),
                    description: "",
                });
            })
        })
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
        Type::Path(path) => Ok(path.path.segments.last().unwrap().ident.clone()),
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

pub(crate) fn dispatch_property_export(field: &SpannedValue<FieldOpts>) -> TokenStream {
    let string_name_ty = string_name_ty();
    let field_name = field.ident.as_ref().unwrap().to_string();
    let field_string_name = quote!(#string_name_ty::from(#field_name));

    quote_spanned! { field.span() =>
        state.insert(#field_string_name, self.get(#field_string_name).unwrap());
    }
}

#[proc_macro_derive(GodotScriptEnum, attributes(script_enum))]
pub fn script_enum_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    enums::script_enum_derive(input)
}

/// Derive an implementation of [`ScriptPropertyGroup`](godot_rust_script::ScriptPropertyGroup).
///
/// Automatically generate an implementation of the `ScriptPropertyGroup` trait. The export attributes of the [`GodotScript`] derive macro
/// are supported here as well. See the other derive macro for details.
#[proc_macro_derive(
    ScriptPropertyGroup,
    attributes(export, export_group, export_subgroup, script, prop, signal)
)]
pub fn derive_property_group(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    property_group::derive_property_group(
        Ident::new("ScriptPropertyGroup", Span::call_site()),
        input,
        true,
    )
}

/// Derive an implementation of [`ScriptPropertySubgroup`](godot_rust_script::ScriptPropertyGroup).
///
/// Automatically generate an implementation of the `ScriptPropertySubgroup` trait. The export attributes of the [`GodotScript`] derive macro
/// are supported here as well. See the other derive macro for details.
#[proc_macro_derive(
    ScriptPropertySubgroup,
    attributes(export, export_group, export_subgroup, script, prop, signal)
)]
pub fn derive_property_subgroup(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    property_group::derive_property_group(
        Ident::new("ScriptPropertySubgroup", Span::call_site()),
        input,
        false,
    )
}
