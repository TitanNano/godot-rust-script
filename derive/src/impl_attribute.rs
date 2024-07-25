/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use proc_macro2::TokenStream;
use quote::{quote, quote_spanned, ToTokens};
use syn::{parse_macro_input, spanned::Spanned, FnArg, ImplItem, ItemImpl, ReturnType, Type};

use crate::{
    is_context_type, rust_to_variant_type,
    type_paths::{godot_types, property_hints, string_name_ty, variant_ty},
};

pub fn godot_script_impl(
    _args: proc_macro::TokenStream,
    body: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let body = parse_macro_input!(body as ItemImpl);

    let godot_types = godot_types();
    let string_name_ty = string_name_ty();
    let variant_ty = variant_ty();
    let call_error_ty = quote!(#godot_types::sys::GDExtensionCallErrorType);
    let property_hints = property_hints();

    let current_type = &body.self_ty;

    let result: Result<Vec<(TokenStream, TokenStream)>, _> = body
        .items
        .iter()
        .filter_map(|item| match item {
            ImplItem::Fn(fnc) => Some(fnc),
            _ => None,
        })
        .filter(|fnc| matches!(fnc.vis, syn::Visibility::Public(_)))
        .map(|fnc| {
            let fn_name = &fnc.sig.ident;
            let fn_name_str = fn_name.to_string();
            let fn_return_ty_rust = match &fnc.sig.output {
                ty @ ReturnType::Default => syn::parse2::<Type>(quote_spanned!(ty.span() => ())).map_err(|err| err.into_compile_error())?,
                ReturnType::Type(_, ty) => (**ty).to_owned(),
            };
            let fn_return_ty = rust_to_variant_type(&fn_return_ty_rust)?;
            let is_static = !fnc.sig.inputs.iter().any(|arg| matches!(arg, FnArg::Receiver(_)));

            let args: Vec<(TokenStream, TokenStream)> = fnc.sig.inputs
                .iter()
                .filter_map(|arg| match arg {
                    syn::FnArg::Typed(arg) => Some(arg),
                    syn::FnArg::Receiver(_) => None
                })
                .enumerate()
                .map(|(index, arg)| {
                    let arg_name = arg.pat.as_ref();
                    let arg_type = rust_to_variant_type(arg.ty.as_ref()).unwrap();

                    is_context_type(arg.ty.as_ref()).then(|| {
                        (
                            quote!(),

                            quote_spanned!(arg.span() => ctx,)
                        )
                    }).unwrap_or_else(|| {
                        (
                            quote_spanned! {
                                arg.span() =>
                                ::godot_rust_script::RustScriptPropDesc {
                                    name: stringify!(#arg_name),
                                    ty: #arg_type,
                                    exported: false,
                                    hint: #property_hints::NONE,
                                    hint_string: "",
                                    description: "",
                                },
                            },

                            quote_spanned! {
                                arg.span() =>
                                #godot_types::prelude::FromGodot::try_from_variant(
                                    args.get(#index).ok_or(#godot_types::sys::GDEXTENSION_CALL_ERROR_TOO_FEW_ARGUMENTS)?
                                ).map_err(|err| {
                                    #godot_types::global::godot_error!("failed to convert variant for argument {} of {}: {}", stringify!(#arg_name), #fn_name_str,  err);
                                    #godot_types::sys::GDEXTENSION_CALL_ERROR_INVALID_ARGUMENT
                                })?,
                            }
                        )
                    })
                })
                .collect();

            let arg_count = args.len();

            let (args_meta, args): (TokenStream, TokenStream) = args.into_iter().unzip();


            let dispatch = quote_spanned! {
                fnc.span() =>
                #fn_name_str => {
                    if args.len() > #arg_count {
                        return Err(#godot_types::sys::GDEXTENSION_CALL_ERROR_TOO_MANY_ARGUMENTS);
                    }

                    Ok(#godot_types::prelude::ToGodot::to_variant(&self.#fn_name(#args)))
                },
            };

            let method_flag = if is_static {
                quote!(#godot_types::global::MethodFlags::STATIC)
            } else {
                quote!(#godot_types::global::MethodFlags::NORMAL)
            };

            let description = fnc.attrs.iter()
                .filter(|attr| attr.path().is_ident("doc"))
                .map(|attr| attr.meta.require_name_value().unwrap().value.to_token_stream())
                .reduce(|mut acc, ident| {
                    acc.extend(quote!(, "\n", ));
                    acc.extend(ident);
                    acc
                });

            let metadata = quote_spanned! {
                fnc.span() =>
                ::godot_rust_script::RustScriptMethodDesc {
                    name: #fn_name_str,
                    arguments: Box::new([#args_meta]),
                    return_type: ::godot_rust_script::RustScriptPropDesc {
                        name: #fn_name_str,
                        ty: #fn_return_ty,
                        exported: false,
                        hint: #property_hints::NONE,
                        hint_string: "",
                        description: "",
                    },
                    flags: #method_flag,
                    description: concat!(#description),
                },
            };

            Ok((dispatch, metadata))
        })
        .collect();

    let (method_dispatch, method_metadata): (TokenStream, TokenStream) = match result {
        Ok(r) => r.into_iter().unzip(),
        Err(err) => return err,
    };

    let trait_impl = quote_spanned! {
        current_type.span() =>
        impl ::godot_rust_script::GodotScriptImpl for #current_type {
            #[allow(unused_variables)]
            fn call_fn(&mut self, name: #string_name_ty, args: &[&#variant_ty], ctx: ::godot_rust_script::Context) -> ::std::result::Result<#variant_ty, #call_error_ty> {
                match name.to_string().as_str() {
                    #method_dispatch

                    _ => Err(#godot_types::sys::GDEXTENSION_CALL_ERROR_INVALID_METHOD),
                }
            }
        }
    };

    let metadata = quote! {
        ::godot_rust_script::register_script_methods!(
            #current_type,
            vec![
                #method_metadata
            ]
        );
    };

    quote! {
        #body

        #trait_impl

        #metadata
    }
    .into()
}
