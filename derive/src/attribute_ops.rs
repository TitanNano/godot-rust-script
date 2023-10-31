use darling::{ast::Data, util, FromAttributes, FromDeriveInput, FromField, FromMeta};
use proc_macro2::{Span, TokenStream};
use quote::quote;

use crate::type_paths::godot_types;

#[derive(FromAttributes, Debug)]
#[darling(attributes(export))]
pub struct FieldExportOps {
    #[darling(default)]
    color_no_alpha: bool,
    #[darling(default)]
    dir: bool,
    exp_easing: Option<syn::ExprArray>,
    file: Option<syn::ExprArray>,
    flags: Option<syn::ExprArray>,
    #[darling(default)]
    global_dir: bool,
    #[darling(default)]
    global_file: bool,
    #[darling(default)]
    multiline: bool,
    node_path: Option<syn::ExprArray>,
    placeholder: Option<String>,
    range: Option<ExportRangeOps>,
}

impl FieldExportOps {
    pub fn hint(&self, span: Span) -> Result<(TokenStream, String), TokenStream> {
        let godot_types = godot_types();
        let property_hints = quote!(#godot_types::engine::global::PropertyHint);
        let mut result: Option<(&str, TokenStream, String)> = None;

        if self.color_no_alpha {
            result = Some((
                "color_no_alpha",
                quote!(#property_hints::PROPERTY_HINT_COLOR_NO_ALPHA),
                String::new(),
            ));
        }

        if self.dir {
            let field = "dir";

            if let Some((active_field, _, _)) = result {
                return Self::error(span, active_field, field);
            }

            result = Some((
                field,
                quote!(#property_hints::PROPERTY_HINT_DIR),
                String::new(),
            ));
        }

        if let Some(exp_list) = self.exp_easing.as_ref() {
            let field = "exp_easing";

            if let Some((active_field, _, _)) = result {
                return Self::error(span, active_field, field);
            }

            let parsed_params = exp_list
                .elems
                .iter()
                .map(|item| ExpEasingOpts::from_expr(item))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| err.write_errors())?;

            let serialized_params: Vec<_> = parsed_params
                .into_iter()
                .map(|item| match item {
                    ExpEasingOpts::Attenuation => "atenuation",
                    ExpEasingOpts::PositiveOnly => "positive_only",
                })
                .collect();

            result = Some((
                field,
                quote!(#property_hints::PROPERTY_HINT_EXP_EASING),
                serialized_params.join(","),
            ));
        }

        if let Some(list) = self.file.as_ref() {
            let field = "file";

            if let Some((active_field, _, _)) = result {
                return Self::error(span, active_field, field);
            }

            let filters = list
                .elems
                .iter()
                .map(|item| String::from_expr(item))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| err.write_errors())?;

            result = Some((
                field,
                quote!(#property_hints::PROPERTY_HINT_FILE),
                filters.join(","),
            ));
        }

        if self.flags.is_some() {
            let field = "flags";

            if let Some((active_field, _, _)) = result {
                return Self::error(span, active_field, field);
            }

            result = Some((
                field,
                quote!(#property_hints::PROPERTY_HINT_FLAGS),
                String::new(),
            ));
        }

        if self.global_dir {
            let field = "global_dir";

            if let Some((active_field, _, _)) = result {
                return Self::error(span, active_field, field);
            }

            result = Some((
                field,
                quote!(#property_hints::PROPERTY_HINT_GLOBAL_DIR),
                String::new(),
            ));
        }

        if self.global_file {
            let field = "global_file";

            if let Some((active_field, _, _)) = result {
                return Self::error(span, active_field, field);
            }

            result = Some((
                field,
                quote!(#property_hints::PROPERTY_HINT_GLOBAL_FILE),
                String::new(),
            ));
        }

        if self.multiline {
            let field = "multiline";

            if let Some((active_field, _, _)) = result {
                return Self::error(span, active_field, field);
            }

            result = Some((
                field,
                quote!(#property_hints::PROPERTY_HINT_MULTILINE),
                String::new(),
            ));
        }

        if let Some(list) = self.node_path.as_ref() {
            let field = "node_path";

            if let Some((active_field, _, _)) = result {
                return Self::error(span, active_field, field);
            }

            let types = list
                .elems
                .iter()
                .map(|item| String::from_expr(item))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| err.write_errors())?;

            result = Some((
                field,
                quote!(#property_hints::PROPERTY_HINT_NODE_PATH_VALID_TYPES),
                types.join(","),
            ));
        }

        if let Some(text) = self.placeholder.as_ref() {
            let field = "placeholder";

            if let Some((active_field, _, _)) = result {
                return Self::error(span, active_field, field);
            }

            result = Some((
                field,
                quote!(#property_hints::PROPERTY_HINT_PLACEHOLDER_TEXT),
                text.to_owned(),
            ));
        }

        if let Some(ops) = self.range.as_ref() {
            let field = "range";

            if let Some((active_field, _, _)) = result {
                return Self::error(span, active_field, field);
            }

            let step = ops.step.unwrap_or(1.0);

            result = Some((
                field,
                quote!(#property_hints::PROPERTY_HINT_RANGE),
                format!("{},{},{}", ops.min, ops.max, step),
            ));
        }

        let result = result
            .map(|(_, tokens, hint_string)| (tokens, hint_string))
            .unwrap_or_else(|| (quote!(#property_hints::PROPERTY_HINT_NONE), String::new()));

        Ok(result)
    }

    fn error(
        span: Span,
        active_field: &str,
        field: &str,
    ) -> Result<(TokenStream, String), TokenStream> {
        let err = syn::Error::new(
            span,
            format!("{} is not compatible with {}", active_field, field),
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
#[darling(forward_attrs(export))]
pub struct FieldOpts {
    pub ident: Option<syn::Ident>,
    pub attrs: Vec<syn::Attribute>,
    pub vis: syn::Visibility,
    pub ty: syn::Type,
}

#[derive(FromDeriveInput)]
#[darling(supports(struct_any), attributes(script))]
pub struct GodotScriptOpts {
    pub ident: syn::Ident,
    pub data: Data<util::Ignored, FieldOpts>,
    pub base: Option<syn::Ident>,
}
