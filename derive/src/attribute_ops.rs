/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use darling::ast::Data;
use darling::util::{self, WithOriginal};
use darling::{FromAttributes, FromDeriveInput, FromField, FromMeta};
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{LitStr, Meta, Type};

use crate::type_paths::godot_types;

#[derive(FromAttributes, Debug)]
#[darling(attributes(export))]
pub struct FieldExportOps {
    color_no_alpha: Option<WithOriginal<bool, Meta>>,
    dir: Option<WithOriginal<bool, Meta>>,
    exp_easing: Option<WithOriginal<syn::ExprArray, Meta>>,
    file: Option<WithOriginal<syn::ExprArray, Meta>>,
    enum_options: Option<WithOriginal<syn::ExprArray, Meta>>,
    flags: Option<WithOriginal<syn::ExprArray, Meta>>,
    global_dir: Option<WithOriginal<bool, Meta>>,
    global_file: Option<WithOriginal<(), Meta>>,
    multiline: Option<WithOriginal<(), Meta>>,
    node_path: Option<WithOriginal<syn::ExprArray, Meta>>,
    placeholder: Option<WithOriginal<String, Meta>>,
    range: Option<WithOriginal<ExportRangeOps, Meta>>,
    #[darling(rename = "ty")]
    custom_type: Option<WithOriginal<LitStr, Meta>>,
}

impl FieldExportOps {
    pub fn hint(&self, ty: &Type) -> Result<(TokenStream, TokenStream), TokenStream> {
        let godot_types = godot_types();
        let property_hints = quote!(#godot_types::global::PropertyHint);

        let mut result: Option<(&str, TokenStream, TokenStream)> = None;

        if let Some(color_no_alpha) = self.color_no_alpha.as_ref() {
            result = Some((
                "color_no_alpha",
                quote_spanned!(color_no_alpha.original.span() => #property_hints::COLOR_NO_ALPHA),
                quote_spanned!(color_no_alpha.original.span() => String::new()),
            ));
        }

        if let Some(dir) = self.dir.as_ref() {
            let field = "dir";

            if let Some((active_field, _, _)) = result {
                return Self::error(dir.original.span(), active_field, field);
            }

            result = Some((
                field,
                quote_spanned!(dir.original.span() => Some(#property_hints::DIR)),
                quote_spanned!(dir.original.span() => Some(String::new())),
            ));
        }

        if let Some(exp_list) = self.exp_easing.as_ref() {
            let field = "exp_easing";

            if let Some((active_field, _, _)) = result {
                return Self::error(exp_list.original.span(), active_field, field);
            }

            let parsed_params = exp_list
                .parsed
                .elems
                .iter()
                .map(ExpEasingOpts::from_expr)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| err.write_errors())?;

            let serialized_params = parsed_params
                .into_iter()
                .map(|item| match item {
                    ExpEasingOpts::Attenuation => "atenuation",
                    ExpEasingOpts::PositiveOnly => "positive_only",
                })
                .collect::<Vec<_>>()
                .join(",");

            result = Some((
                field,
                quote_spanned!(exp_list.original.span() => Some(#property_hints::EXP_EASING)),
                quote_spanned!(exp_list.original.span() => Some(String::from(#serialized_params))),
            ));
        }

        if let Some(list) = self.file.as_ref() {
            let field = "file";

            if let Some((active_field, _, _)) = result {
                return Self::error(list.original.span(), active_field, field);
            }

            let filters = list
                .parsed
                .elems
                .iter()
                .map(String::from_expr)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| err.write_errors())?
                .join(",");

            result = Some((
                field,
                quote_spanned!(list.original.span() => Some(#property_hints::FILE)),
                quote_spanned!(list.original.span() => Some(String::from(#filters))),
            ));
        }

        if let Some(list) = self.enum_options.as_ref() {
            let field = "enum_options";

            if let Some((active_field, _, _)) = result {
                return Self::error(list.original.span(), active_field, field);
            }

            let flags = list
                .parsed
                .elems
                .iter()
                .map(String::from_expr)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| err.write_errors())?
                .join(",");

            result = Some((
                field,
                quote_spanned!(list.original.span() => Some(#property_hints::ENUM)),
                quote_spanned!(list.original.span() => Some(String::from(#flags))),
            ));
        }

        if let Some(list) = self.flags.as_ref() {
            let field = "flags";

            if let Some((active_field, _, _)) = result {
                return Self::error(list.original.span(), active_field, field);
            }

            let flags = list
                .parsed
                .elems
                .iter()
                .map(String::from_expr)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| err.write_errors())?
                .join(",");

            result = Some((
                field,
                quote_spanned!(list.original.span() => Some(#property_hints::FLAGS)),
                quote_spanned!(list.original.span() => Some(String::from(#flags))),
            ));
        }

        if let Some(global_dir) = self.global_dir.as_ref() {
            let field = "global_dir";

            if let Some((active_field, _, _)) = result {
                return Self::error(global_dir.original.span(), active_field, field);
            }

            result = Some((
                field,
                quote_spanned!(global_dir.original.span() => Some(#property_hints::GLOBAL_DIR)),
                quote_spanned!(global_dir.original.span() => Some(String::new())),
            ));
        }

        if let Some(global_file) = self.global_file.as_ref() {
            let field = "global_file";

            if let Some((active_field, _, _)) = result {
                return Self::error(global_file.original.span(), active_field, field);
            }

            result = Some((
                field,
                quote_spanned!(global_file.original.span() => Some(#property_hints::GLOBAL_FILE)),
                quote_spanned!(global_file.original.span() => Some(String::new())),
            ));
        }

        if let Some(multiline) = self.multiline.as_ref() {
            let field = "multiline";

            if let Some((active_field, _, _)) = result {
                return Self::error(multiline.original.span(), active_field, field);
            }

            result = Some((
                field,
                quote_spanned!(multiline.original.span() => Some(#property_hints::MULTILINE)),
                quote_spanned!(multiline.original.span() => Some(String::new())),
            ));
        }

        if let Some(list) = self.node_path.as_ref() {
            let field = "node_path";

            if let Some((active_field, _, _)) = result {
                return Self::error(list.original.span(), active_field, field);
            }

            let types = list
                .parsed
                .elems
                .iter()
                .map(String::from_expr)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| err.write_errors())?
                .join(",");

            result = Some((
                field,
                quote_spanned!(list.original.span() => Some(#property_hints::NODE_PATH_VALID_TYPES)),
                quote_spanned!(list.original.span() => Some(String::from(#types))),
            ));
        }

        if let Some(text) = self.placeholder.as_ref() {
            let field = "placeholder";

            if let Some((active_field, _, _)) = result {
                return Self::error(text.original.span(), active_field, field);
            }

            let content = &text.parsed;

            result = Some((
                field,
                quote_spanned!(text.original.span() => Some(#property_hints::PLACEHOLDER_TEXT)),
                quote_spanned!(text.original.span() => Some(String::from(#content))),
            ));
        }

        if let Some(ops) = self.range.as_ref() {
            let field = "range";

            if let Some((active_field, _, _)) = result {
                return Self::error(ops.original.span(), active_field, field);
            }

            let step = ops.parsed.step.unwrap_or(1.0);
            let hint_string = format!("{},{},{}", ops.parsed.min, ops.parsed.max, step);

            result = Some((
                field,
                quote_spanned!(ops.original.span() => Some(#property_hints::RANGE)),
                quote_spanned!(ops.original.span() => Some(String::from(#hint_string))),
            ));
        }

        if let Some(attr_ty) = self.custom_type.as_ref() {
            let field = "ty";

            if let Some((active_field, _, _)) = result {
                return Self::error(attr_ty.original.span(), active_field, field);
            }

            let attr_ty_raw = &attr_ty.parsed;

            let hint = quote_spanned!(ty.span() => None);
            let hint_string =
                quote_spanned!(attr_ty.original.span() => Some(String::from(#attr_ty_raw)));

            result = Some((field, hint, hint_string));
        }

        let (hint, hint_string) = result
            .map(|(_, tokens, hint_string)| (tokens, hint_string))
            .unwrap_or_else(|| (quote!(None), quote!(None)));

        let default_hint = quote_spanned!(ty.span() => <#ty as ::godot_rust_script::GodotScriptExport>::hint(#hint));
        let default_hint_string = quote_spanned!(ty.span() => <#ty as ::godot_rust_script::GodotScriptExport>::hint_string(#hint, #hint_string));

        Ok((default_hint, default_hint_string))
    }

    fn error(
        span: Span,
        active_field: &str,
        field: &str,
    ) -> Result<(TokenStream, TokenStream), TokenStream> {
        let err = syn::Error::new(
            span,
            format!("{} is not compatible with {}", field, active_field),
        )
        .into_compile_error();

        Err(err)
    }
}

#[derive(FromMeta, Debug)]
struct ExportRangeOps {
    min: f64,
    max: f64,
    step: Option<f64>,
}

#[derive(FromMeta, Debug)]
enum ExpEasingOpts {
    Attenuation,
    PositiveOnly,
}

#[derive(FromField, Debug)]
#[darling(forward_attrs(export, prop, doc, signal))]
pub struct FieldOpts {
    pub ident: Option<syn::Ident>,
    pub attrs: Vec<syn::Attribute>,
    pub vis: syn::Visibility,
    pub ty: syn::Type,
}

#[derive(FromDeriveInput, Debug)]
#[darling(supports(struct_any), attributes(script), forward_attrs(doc))]
pub struct GodotScriptOpts {
    pub ident: syn::Ident,
    pub data: Data<util::Ignored, FieldOpts>,
    pub base: Option<syn::Ident>,
    pub attrs: Vec<syn::Attribute>,
}

#[derive(FromAttributes, Debug)]
#[darling(attributes(prop))]
pub struct PropertyOpts {
    pub get: Option<syn::Expr>,
    pub set: Option<syn::Expr>,
}
