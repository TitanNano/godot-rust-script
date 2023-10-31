mod attribute_ops;
mod impl_attribute;
mod type_paths;

use attribute_ops::{FieldOpts, GodotScriptOpts};
use darling::{FromAttributes, FromDeriveInput};
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::{parse_macro_input, spanned::Spanned, DeriveInput};
use type_paths::{godot_types, string_name_ty, variant_ty};

use crate::attribute_ops::FieldExportOps;

#[proc_macro_derive(GodotScript, attributes(export, script))]
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
    let fields = opts.data.take_struct().unwrap().fields;

    let public_fields = fields
        .iter()
        .filter(|field| matches!(field.vis, syn::Visibility::Public(_)));

    let field_metadata_result: Result<TokenStream, TokenStream> = public_fields
        .clone()
        .map(|field| {
            let name = field
                .ident
                .as_ref()
                .map(|field| field.to_string())
                .unwrap_or_default();

            let ty = rust_to_variant_type(&field.ty)?;

            let exported = field
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("export"));

            let (hint, hint_string) = {
                let ops = FieldExportOps::from_attributes(&field.attrs)
                    .map_err(|err| err.write_errors())?;

                ops.hint(field.ident.span())?
            };

            let item = quote! {
                ::godot_rust_script::RustScriptPropDesc {
                    name: #name,
                    ty: #ty,
                    exported: #exported,
                    hint: #hint,
                    hint_string: #hint_string,
                },
            };

            Ok(item)
        })
        .collect();

    let field_metadata = match field_metadata_result {
        Ok(meta) => meta,
        Err(err) => return err.into(),
    };

    let get_fields_impl = derive_get_fields(public_fields.clone());
    let set_fields_impl = derive_set_fields(public_fields.clone());
    let properties_state_impl = derive_property_states_export(public_fields);
    let default_impl = derive_default_with_base(&fields);

    let output = quote! {
        impl ::godot_rust_script::GodotScript for #script_type_ident {
            #get_fields_impl

            #set_fields_impl

            fn call(&mut self, name: #string_name_ty, args: &[&#variant_ty]) -> ::std::result::Result<#variant_ty, #call_error_ty> {
                ::godot_rust_script::GodotScriptImpl::call_fn(self, name, args)
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
            vec![
                #field_metadata
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

                <#path as #godot_types::builtin::meta::GodotType>::Ffi::variant_type()
            }
        }),
        T::Verbatim(_) => Err(syn::Error::new(
            ty.span(),
            "not sure how to handle verbatim types yet!",
        )
        .into_compile_error()),
        T::Tuple(tuple) => {
            if tuple.elems.len() > 0 {
                return Err(syn::Error::new(
                    ty.span(),
                    format!("\"{}\" is not a supported type", quote!(#tuple)),
                )
                .into_compile_error());
            }

            Ok(quote_spanned! {
                tuple.span() => {
                    use #godot_types::sys::GodotFfi;

                    <#tuple as #godot_types::builtin::meta::GodotType>::Ffi::variant_type()
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

fn derive_default_with_base(field_opts: &[FieldOpts]) -> TokenStream {
    let godot_types = godot_types();
    let fields: TokenStream = field_opts
        .into_iter()
        .filter_map(|field| match field.ident.as_ref() {
            Some(ident) if ident.to_string() == "base" => {
                Some(quote_spanned!(ident.span() => #ident: base.cast(),))
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

fn derive_get_fields<'a>(public_fields: impl Iterator<Item = &'a FieldOpts> + 'a) -> TokenStream {
    let godot_types = godot_types();
    let string_name_ty = string_name_ty();
    let variant_ty = variant_ty();

    let get_field_dispatch: TokenStream = public_fields
        .filter_map(|field| field.ident.as_ref())
        .map(|field| {
            let field_name = field.to_string();

            quote! {
                #field_name => Some(#godot_types::prelude::ToGodot::to_variant(&self.#field)),
            }
        })
        .collect();

    quote! {
        fn get(&self, name: #string_name_ty) -> ::std::option::Option<#variant_ty> {
            match name.to_string().as_str() {
                #get_field_dispatch

                _ => None,
            }
        }
    }
}

fn derive_set_fields<'a>(public_fields: impl Iterator<Item = &'a FieldOpts> + 'a) -> TokenStream {
    let string_name_ty = string_name_ty();
    let variant_ty = variant_ty();
    let godot_types = godot_types();

    let set_field_dispatch: TokenStream = public_fields
        .filter_map(|field| field.ident.as_ref())
        .map(|field| {
            let field_name = field.to_string();

            quote_spanned! {
                field.span() =>
                #field_name => self.#field = #godot_types::prelude::FromGodot::from_variant(&value),
            }
        })
        .collect();

    quote! {
        fn set(&mut self, name: #string_name_ty, value: #variant_ty) -> bool {
            match name.to_string().as_str() {
                #set_field_dispatch

                _ => return false,
            }

            true
        }
    }
}

fn derive_property_states_export<'a>(
    public_fields: impl Iterator<Item = &'a FieldOpts> + 'a,
) -> TokenStream {
    let string_name_ty = string_name_ty();
    let variant_ty = variant_ty();

    let fetch_property_states: TokenStream = public_fields
        .filter_map(|field| field.ident.as_ref())
        .map(|field| {
            let field_name = field.to_string();
            let field_string_name = quote!(#string_name_ty::from(#field_name));

            quote! {
                (#field_string_name, self.get(#field_string_name).unwrap()),
            }
        })
        .collect();

    quote! {
        fn property_state(&self) -> ::std::collections::HashMap<#string_name_ty, #variant_ty> {
            ::std::collections::HashMap::from([
                #fetch_property_states
            ])
        }
    }
}

#[proc_macro_attribute]
pub fn godot_script_impl(
    args: proc_macro::TokenStream,
    body: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    impl_attribute::godot_script_impl(args, body)
}
