use crate::{extract_source_field, generics_required_by_type, source_field_of, ErrorAttrArg};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use std::collections::HashSet;
use syn::{
    parse_quote, spanned::Spanned, Error, Fields, FieldsNamed, Generics, ItemEnum, Type, Variant,
    Visibility,
};

fn generate_empty_named_variant_ctx(
    enum_: &ItemEnum,
    variant: &Variant,
    source_field_ty: &Type,
) -> TokenStream {
    let ctx_ident = &variant.ident;
    let ty_ident = &enum_.ident;
    let (ty_impl_generics, ty_ty_generics, ty_where_clause) = enum_.generics.split_for_impl();

    let struct_def = quote! {
        pub(super) struct #ctx_ident;
    };

    let ctx_impl = quote! {
        impl #ty_impl_generics ::easyerr::ErrorContext for #ctx_ident #ty_where_clause {
            type Source = #source_field_ty;
            type Err = #ty_ident #ty_ty_generics;

            #[inline(always)]
            fn add_to_source(self, source: Self::Source) -> #ty_ident #ty_ty_generics {
                #ty_ident::#ctx_ident {
                    source,
                }
            }
        }
    };

    quote! {
        #struct_def
        #ctx_impl
    }
}

fn generate_named_variant_ctx(
    enum_: &ItemEnum,
    variant: &Variant,
    fields: &FieldsNamed,
) -> Result<TokenStream, Error> {
    let (mut ctx_fields, Some(source_field_ty)) = extract_source_field(fields.named.iter()) else {
        return Err(Error::new(
            variant.span(),
            "can't generate context for variant without source",
        ));
    };

    if ctx_fields.is_empty() {
        return Ok(generate_empty_named_variant_ctx(
            enum_,
            variant,
            &source_field_ty,
        ));
    }

    ctx_fields
        .iter_mut()
        .for_each(|f| f.vis = Visibility::Public(syn::token::Pub { span: f.vis.span() }));

    let used_generics = fields
        .named
        .iter()
        .flat_map(|f| generics_required_by_type(&enum_.generics, &f.ty))
        .collect::<HashSet<_>>()
        .into_iter();

    let used_generics: Generics = parse_quote! {
        <#(#used_generics),*>
    };

    let ty_ident = &enum_.ident;
    let (ty_impl_generics, ty_ty_generics, ty_where_clause) = enum_.generics.split_for_impl();

    let ctx_ident = &variant.ident;
    let (ctx_impl_generics, ctx_ty_generics, ctx_where_clause) = used_generics.split_for_impl();

    let struct_def = quote! {
        pub(super) struct #ctx_ident #ctx_impl_generics #ctx_where_clause {
            #(#ctx_fields),*
        }
    };

    let ctx_fields_extract = ctx_fields.iter().map(|f| {
        let f_name = &f.ident;
        quote! {
            #f_name: self.#f_name
        }
    });

    let ctx_impl = quote! {
        impl #ty_impl_generics ::easyerr::ErrorContext for #ctx_ident #ctx_ty_generics #ty_where_clause {
            type Source = #source_field_ty;
            type Err = #ty_ident #ty_ty_generics;

            #[inline(always)]
            fn add_to_source(self, source: Self::Source) -> #ty_ident #ty_ty_generics {
                #ty_ident::#ctx_ident {
                    source,
                    #(#ctx_fields_extract),*
                }
            }
        }
    };

    Ok(quote! {
        #struct_def
        #ctx_impl
    })
}

fn generate_variant_ctx(enum_: &ItemEnum, variant: &Variant) -> Result<Option<TokenStream>, Error> {
    match &variant.fields {
        Fields::Named(f) => source_field_of(f.named.iter())
            .is_some()
            .then(|| generate_named_variant_ctx(enum_, variant, f))
            .transpose(),
        Fields::Unnamed(_) => Ok(None),
        Fields::Unit => Ok(None),
    }
}

fn generate_variant_display_arm(variant: &Variant) -> Result<TokenStream, Error> {
    let variant_ident = &variant.ident;
    let error_attr = variant
        .attrs
        .iter()
        .find(|a| a.path().is_ident("error"))
        .ok_or(Error::new(
            variant.span(),
            "variant is missing #[error(...)] attribute",
        ))?;

    let error_attr_arg = error_attr.parse_args::<ErrorAttrArg>()?;
    let display = match error_attr_arg {
        ErrorAttrArg::Str(variant_display_str) => match &variant.fields {
            Fields::Named(f) => {
                let fields = f.named.iter().map(|f| &f.ident);
                quote! {
                    Self::#variant_ident { #(#fields),* } => {
                        write!(f, #variant_display_str)?;
                    }
                }
            }
            Fields::Unnamed(f) => {
                let fields = f
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(i, _)| format_ident!("f{i}"));
                quote! {
                    Self::#variant_ident(#(#fields),*) => {
                        write!(f, #variant_display_str)?;
                    }
                }
            }
            Fields::Unit => {
                quote! {
                    Self::#variant_ident => {
                        write!(f, #variant_display_str)?;
                    }
                }
            }
        },
        ErrorAttrArg::Transparent => {
            if source_field_of(variant.fields.iter()).is_none() {
                return Err(Error::new(
                    error_attr.span(),
                    "can't use `transparent` display on a variant with no source field",
                ));
            }

            quote! {
                Self::#variant_ident { source, .. } => {
                    source.fmt(f)?;
                }
            }
        }
    };

    Ok(display)
}

fn generate_enum_display_impl(enum_: &ItemEnum) -> Result<TokenStream, Error> {
    let match_arms: Result<Vec<_>, _> = enum_
        .variants
        .iter()
        .map(generate_variant_display_arm)
        .collect();
    let match_arms = match_arms?;

    let enum_ident = &enum_.ident;
    let (impl_generics, ty_generics, where_clause) = enum_.generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics std::fmt::Display for #enum_ident #ty_generics #where_clause {
            #[allow(unused_variables)]
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                match self {
                    #(#match_arms),*
                }

                Ok(())
            }
        }
    })
}

fn generate_variant_error_arm(variant: &Variant) -> Result<TokenStream, Error> {
    let variant_ident = &variant.ident;
    let display = match &variant.fields {
        Fields::Named(f) => {
            if let Some(f) = source_field_of(f.named.iter()) {
                let error_attr = variant
                    .attrs
                    .iter()
                    .find(|a| a.path().is_ident("error"))
                    .ok_or(Error::new(
                        variant.span(),
                        "variant is missing #[error(...)] attribute",
                    ))?;
                let error_attr_arg = error_attr.parse_args::<ErrorAttrArg>()?;
                if error_attr_arg == ErrorAttrArg::Transparent {
                    quote_spanned! {
                        f.ty.span() =>
                        Self::#variant_ident { source, .. } => {
                            source.source()
                        }
                    }
                } else {
                    quote_spanned! {
                        f.ty.span() =>
                            Self::#variant_ident { source, .. } => {
                                Some(source)
                            }
                    }
                }
            } else {
                quote! {
                    Self::#variant_ident { .. } => {
                        None
                    }
                }
            }
        }
        Fields::Unnamed(_) => {
            quote! {
                Self::#variant_ident(..) => {
                    None
                }
            }
        }
        Fields::Unit => {
            quote! {
                Self::#variant_ident => {
                    None
                }
            }
        }
    };

    Ok(display)
}

fn generate_enum_error_impl(enum_: &ItemEnum) -> Result<TokenStream, Error> {
    let ty_ident = &enum_.ident;
    let (impl_generics, ty_generics, where_clause) = enum_.generics.split_for_impl();
    let match_arms: Result<Vec<_>, _> = enum_
        .variants
        .iter()
        .map(generate_variant_error_arm)
        .collect();
    let match_arms = match_arms?;

    Ok(quote! {
        impl #impl_generics ::std::error::Error for #ty_ident #ty_generics #where_clause {
            fn source(&self) -> Option<&(dyn ::std::error::Error + 'static)> {
                match self {
                    #(#match_arms),*
                }
            }
        }
    })
}

pub fn derive_err_enum(enum_: &ItemEnum) -> Result<TokenStream, Error> {
    let contexts = enum_
        .variants
        .iter()
        .map(|v| generate_variant_ctx(enum_, v))
        .collect::<Result<Vec<_>, _>>()?;
    let display_impl = generate_enum_display_impl(enum_)?;
    let error_impl = generate_enum_error_impl(enum_)?;

    let module = (!contexts.is_empty()).then(|| {
        let enum_ident_str = enum_.ident.to_string();
        let module_ident_str = enum_ident_str
            .strip_suffix("Error")
            .unwrap_or(&enum_ident_str);
        let module_ident = format_ident!("{}Ctx", module_ident_str);

        quote! {
            #[allow(nonstandard_style)]
            mod #module_ident {
                use super::*;
                #(#contexts)*
            }
        }
    });

    Ok(quote! {
        #module
        #display_impl
        #error_impl
    })
}
