/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use darling::ast::Data;
use darling::util::{self, SpannedValue, WithOriginal};
use darling::{FromAttributes, FromDeriveInput, FromField, FromMeta};
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{LitStr, Meta, PatPath, Type};

use crate::type_paths::godot_types;

#[derive(FromMeta, Debug)]
pub struct FieldSignalOps(pub WithOriginal<Vec<syn::LitStr>, Meta>);

#[derive(FromAttributes, Debug)]
#[darling(attributes(export), forward_attrs)]
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
    storage: Option<WithOriginal<bool, Meta>>,
    custom: Option<WithOriginal<ExportCustomOps, Meta>>,
    #[darling(rename = "ty")]
    custom_type: Option<WithOriginal<LitStr, Meta>>,
    flatten: Option<()>,
}

impl FieldExportOps {
    pub fn is_flatten(&self) -> bool {
        self.flatten.is_some()
    }

    pub fn to_export_meta(&self, ty: &Type, span: Span) -> Result<ExportMetadata, TokenStream> {
        let godot_types = godot_types();
        let property_hints = quote!(#godot_types::global::PropertyHint);
        let property_usage = quote!(#godot_types::global::PropertyUsageFlags);
        let default_usage = quote!(#property_usage::SCRIPT_VARIABLE | #property_usage::EDITOR | #property_usage::STORAGE);

        let mut result: Option<ExportMetadata> = None;

        if let Some(color_no_alpha) = self.color_no_alpha.as_ref() {
            result = Some(ExportMetadata {
                field: "color_no_alpha",
                usage: quote_spanned!(color_no_alpha.original.span() => #default_usage),
                hint: quote_spanned!(color_no_alpha.original.span() => #property_hints::COLOR_NO_ALPHA),
                hint_string: quote_spanned!(color_no_alpha.original.span() => String::new()),
            });
        }

        if let Some(dir) = self.dir.as_ref() {
            const FIELD: &str = "dir";

            if let Some(active_meta) = result {
                return Self::error(dir.original.span(), active_meta.field, FIELD);
            }

            result = Some(ExportMetadata {
                field: FIELD,
                usage: quote_spanned!(dir.original.span() => #default_usage),
                hint: quote_spanned!(dir.original.span() => Some(#property_hints::DIR)),
                hint_string: quote_spanned!(dir.original.span() => Some(String::new())),
            });
        }

        if let Some(exp_list) = self.exp_easing.as_ref() {
            const FIELD: &str = "exp_easing";

            if let Some(active_meta) = result {
                return Self::error(exp_list.original.span(), active_meta.field, FIELD);
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

            result = Some(ExportMetadata {
                field: FIELD,
                usage: quote_spanned!(exp_list.original.span() => #default_usage),
                hint: quote_spanned!(exp_list.original.span() => Some(#property_hints::EXP_EASING)),
                hint_string: quote_spanned!(exp_list.original.span() => Some(String::from(#serialized_params))),
            });
        }

        if let Some(list) = self.file.as_ref() {
            const FIELD: &str = "file";

            if let Some(active_meta) = result {
                return Self::error(list.original.span(), active_meta.field, FIELD);
            }

            let filters = list
                .parsed
                .elems
                .iter()
                .map(String::from_expr)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| err.write_errors())?
                .join(",");

            result = Some(ExportMetadata {
                field: FIELD,
                usage: quote_spanned!(list.original.span() => #default_usage),
                hint: quote_spanned!(list.original.span() => Some(#property_hints::FILE)),
                hint_string: quote_spanned!(list.original.span() => Some(String::from(#filters))),
            });
        }

        if let Some(list) = self.enum_options.as_ref() {
            const FIELD: &str = "enum_options";

            if let Some(active_meta) = result {
                return Self::error(list.original.span(), active_meta.field, FIELD);
            }

            let flags = list
                .parsed
                .elems
                .iter()
                .map(String::from_expr)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| err.write_errors())?
                .join(",");

            result = Some(ExportMetadata {
                field: FIELD,
                usage: quote_spanned!(list.original.span() => #default_usage),
                hint: quote_spanned!(list.original.span() => Some(#property_hints::ENUM)),
                hint_string: quote_spanned!(list.original.span() => Some(String::from(#flags))),
            });
        }

        if let Some(list) = self.flags.as_ref() {
            const FIELD: &str = "flags";

            if let Some(active_meta) = result {
                return Self::error(list.original.span(), active_meta.field, FIELD);
            }

            let flags = list
                .parsed
                .elems
                .iter()
                .map(String::from_expr)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| err.write_errors())?
                .join(",");

            result = Some(ExportMetadata {
                field: FIELD,
                usage: quote_spanned!(list.original.span() => #default_usage),
                hint: quote_spanned!(list.original.span() => Some(#property_hints::FLAGS)),
                hint_string: quote_spanned!(list.original.span() => Some(String::from(#flags))),
            });
        }

        if let Some(global_dir) = self.global_dir.as_ref() {
            const FIELD: &str = "global_dir";

            if let Some(active_meta) = result {
                return Self::error(global_dir.original.span(), active_meta.field, FIELD);
            }

            result = Some(ExportMetadata {
                field: FIELD,
                usage: quote_spanned!(global_dir.original.span() => #default_usage),
                hint: quote_spanned!(global_dir.original.span() => Some(#property_hints::GLOBAL_DIR)),
                hint_string: quote_spanned!(global_dir.original.span() => Some(String::new())),
            });
        }

        if let Some(global_file) = self.global_file.as_ref() {
            const FIELD: &str = "global_file";

            if let Some(active_meta) = result {
                return Self::error(global_file.original.span(), active_meta.field, FIELD);
            }

            result = Some(ExportMetadata {
                field: FIELD,
                usage: quote_spanned!(global_file.original.span() => #default_usage),
                hint: quote_spanned!(global_file.original.span() => Some(#property_hints::GLOBAL_FILE)),
                hint_string: quote_spanned!(global_file.original.span() => Some(String::new())),
            });
        }

        if let Some(multiline) = self.multiline.as_ref() {
            const FIELD: &str = "multiline";

            if let Some(active_meta) = result {
                return Self::error(multiline.original.span(), active_meta.field, FIELD);
            }

            result = Some(ExportMetadata {
                field: FIELD,
                usage: quote_spanned!(multiline.original.span() => #default_usage),
                hint: quote_spanned!(multiline.original.span() => Some(#property_hints::MULTILINE)),
                hint_string: quote_spanned!(multiline.original.span() => Some(String::new())),
            });
        }

        if let Some(list) = self.node_path.as_ref() {
            const FIELD: &str = "node_path";

            if let Some(active_meta) = result {
                return Self::error(list.original.span(), active_meta.field, FIELD);
            }

            let types = list
                .parsed
                .elems
                .iter()
                .map(String::from_expr)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| err.write_errors())?
                .join(",");

            result = Some(ExportMetadata {
                field: FIELD,
                usage: quote_spanned!(list.original.span() => #default_usage),
                hint: quote_spanned!(list.original.span() => Some(#property_hints::NODE_PATH_VALID_TYPES)),
                hint_string: quote_spanned!(list.original.span() => Some(String::from(#types))),
            });
        }

        if let Some(text) = self.placeholder.as_ref() {
            const FIELD: &str = "placeholder";

            if let Some(active_meta) = result {
                return Self::error(text.original.span(), active_meta.field, FIELD);
            }

            let content = &text.parsed;

            result = Some(ExportMetadata {
                field: FIELD,
                usage: quote_spanned!(text.original.span() => #default_usage),
                hint: quote_spanned!(text.original.span() => Some(#property_hints::PLACEHOLDER_TEXT)),
                hint_string: quote_spanned!(text.original.span() => Some(String::from(#content))),
            });
        }

        if let Some(ops) = self.range.as_ref() {
            const FIELD: &str = "range";

            if let Some(active_meta) = result {
                return Self::error(ops.original.span(), active_meta.field, FIELD);
            }

            let step = ops.parsed.step.unwrap_or(1.0);
            let suffix = ops.parsed.suffix.as_deref().unwrap_or("");

            let hint_string = format!(
                "{},{},{},suffix:{}",
                ops.parsed.min, ops.parsed.max, step, suffix
            );

            result = Some(ExportMetadata {
                field: FIELD,
                usage: quote_spanned!(ops.original.span() => #default_usage),
                hint: quote_spanned!(ops.original.span() => Some(#property_hints::RANGE)),
                hint_string: quote_spanned!(ops.original.span() => Some(String::from(#hint_string))),
            });
        }

        if let Some(attr_ty) = self.custom_type.as_ref() {
            const FIELD: &str = "ty";

            if let Some(active_meta) = result {
                return Self::error(attr_ty.original.span(), active_meta.field, FIELD);
            }

            let attr_ty_raw = &attr_ty.parsed;

            let hint = quote_spanned!(attr_ty.original.span() => None);
            let hint_string =
                quote_spanned!(attr_ty.original.span() => Some(String::from(#attr_ty_raw)));

            result = Some(ExportMetadata {
                field: FIELD,
                usage: quote_spanned!(attr_ty.original.span() => #default_usage),
                hint,
                hint_string,
            });
        }

        if let Some(attr_storage) = self.storage.as_ref() {
            const FIELD: &str = "storage";

            if let Some(active_meta) = result {
                return Self::error(attr_storage.original.span(), active_meta.field, FIELD);
            }

            if !attr_storage.parsed {
                let err = syn::Error::new_spanned(
                    &attr_storage.original,
                    "storage can not be set to false",
                )
                .into_compile_error();

                return Err(err);
            }

            let hint = quote_spanned!(attr_storage.original.span() => None);
            let hint_string = quote_spanned!(attr_storage.original.span() => None);

            result = Some(ExportMetadata {
                field: FIELD,
                usage: quote_spanned!(attr_storage.original.span() => #property_usage::SCRIPT_VARIABLE | #property_usage::STORAGE),
                hint,
                hint_string,
            });
        }

        if let Some(attr_custom) = self.custom.as_ref() {
            const FIELD: &str = "custom";

            if let Some(active_meta) = result {
                return Self::error(attr_custom.original.span(), active_meta.field, FIELD);
            }

            let attr_hint = &attr_custom.parsed.hint;
            let attr_hint_str = &attr_custom.parsed.hint_string;

            let hint = quote_spanned!(attr_custom.original.span() => Some({ let hint: #property_hints = #attr_hint; hint }));
            let hint_string =
                quote_spanned!(attr_custom.original.span() => Some(String::from(#attr_hint_str)));

            result = Some(ExportMetadata {
                field: FIELD,
                usage: quote_spanned!(attr_custom.original.span() => #default_usage),
                hint,
                hint_string,
            });
        }

        let metadata = result.unwrap_or_else(|| ExportMetadata {
            field: "",
            usage: quote_spanned!(span => #default_usage),
            hint: quote_spanned!(span => None),
            hint_string: quote_spanned!(span => None),
        });

        let hint = &metadata.hint;
        let hint_string = &metadata.hint_string;

        let default_hint = quote_spanned!(ty.span() => <#ty as ::godot_rust_script::GodotScriptExport>::hint(#hint));
        let default_hint_string = quote_spanned!(ty.span() => <#ty as ::godot_rust_script::GodotScriptExport>::hint_string(#hint, #hint_string));

        Ok(ExportMetadata {
            field: metadata.field,
            usage: metadata.usage,
            hint: default_hint,
            hint_string: default_hint_string,
        })
    }

    fn error(span: Span, active_field: &str, field: &str) -> Result<ExportMetadata, TokenStream> {
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
    suffix: Option<String>,
}

#[derive(FromMeta, Debug)]
enum ExpEasingOpts {
    Attenuation,
    PositiveOnly,
}

#[derive(FromMeta, Debug)]
struct ExportCustomOps {
    hint: PatPath,
    hint_string: String,
}

#[derive(FromField, Debug)]
#[darling(forward_attrs(export, export_group, export_subgroup, prop, doc, signal))]
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
    pub data: Data<util::Ignored, SpannedValue<FieldOpts>>,
    pub base: Option<syn::Ident>,
    pub tool: Option<()>,
    pub attrs: Vec<syn::Attribute>,
}

#[derive(FromAttributes, Debug)]
#[darling(attributes(prop))]
pub struct PropertyOpts {
    pub get: Option<syn::Expr>,
    pub set: Option<syn::Expr>,
}

pub struct ExportMetadata {
    pub field: &'static str,
    pub usage: TokenStream,
    pub hint: TokenStream,
    pub hint_string: TokenStream,
}

#[derive(FromAttributes, Debug)]
#[darling(attributes(export_group))]
pub struct ExportGroup {
    pub name: String,
}

#[derive(FromAttributes, Debug)]
#[darling(attributes(export_subgroup))]
pub struct ExportSubgroup {
    pub name: String,
}

#[derive(FromDeriveInput, Debug)]
#[darling(supports(struct_any), attributes(script), forward_attrs(doc))]
pub struct ScriptPropertyGroupOpts {
    pub data: Data<util::Ignored, SpannedValue<FieldOpts>>,
}
