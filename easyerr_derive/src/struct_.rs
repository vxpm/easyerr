use crate::{extract_source_field, generics_required_by_type, source_field_of, ErrorAttrArg};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashSet;
use syn::{parse_quote, spanned::Spanned, Error, Fields, Generics, ItemStruct, Visibility};

fn generate_ctx(struct_: &ItemStruct) -> Result<TokenStream, Error> {
    let (mut ctx_fields, Some(source_field_ty)) = extract_source_field(struct_.fields.iter())
    else {
        return Err(Error::new(
            struct_.span(),
            "can't generate context for struct without source",
        ));
    };

    ctx_fields
        .iter_mut()
        .for_each(|f| f.vis = Visibility::Public(syn::token::Pub { span: f.vis.span() }));

    let used_generics = struct_
        .fields
        .iter()
        .flat_map(|f| generics_required_by_type(&struct_.generics, &f.ty))
        .collect::<HashSet<_>>()
        .into_iter();
    let used_generics: Generics = parse_quote! {
        <#(#used_generics),*>
    };

    let struct_ident_str = struct_.ident.to_string();
    let ctx_ident_str = struct_ident_str
        .strip_suffix("Error")
        .unwrap_or(&struct_ident_str);
    let ctx_ident = format_ident!("{}Ctx", ctx_ident_str);
    let (ctx_impl_generics, ctx_ty_generics, ctx_where_clause) = used_generics.split_for_impl();

    let ty_ident = &struct_.ident;
    let (ty_impl_generics, ty_ty_generics, ty_where_clause) = struct_.generics.split_for_impl();

    let struct_def = quote! {
        struct #ctx_ident #ctx_impl_generics #ctx_where_clause {
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
                #ty_ident {
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

fn generate_struct_display_impl(struct_: &ItemStruct) -> Result<TokenStream, Error> {
    let struct_display_attr = struct_
        .attrs
        .iter()
        .find(|a| a.path().is_ident("error"))
        .ok_or(Error::new(
            struct_.span(),
            "struct is missing #[error(...)] attribute",
        ))?;

    let display_attr_arg = struct_display_attr.parse_args::<ErrorAttrArg>()?;
    let display = match display_attr_arg {
        ErrorAttrArg::Str(struct_display_str) => match &struct_.fields {
            Fields::Named(f) => {
                let fields = f.named.iter().map(|f| &f.ident);
                quote! {
                    let Self{ #(#fields),* } = self;
                    write!(f, #struct_display_str)
                }
            }
            Fields::Unnamed(f) => {
                let fields = f
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(i, _)| format_ident!("f{i}"));
                quote! {
                    let Self(#(#fields),*) = self;
                    write!(f, #struct_display_str)
                }
            }
            Fields::Unit => {
                quote! {
                    write!(f, #struct_display_str)
                }
            }
        },
        ErrorAttrArg::Transparent => {
            if source_field_of(struct_.fields.iter()).is_none() {
                return Err(Error::new(
                    struct_display_attr.span(),
                    "can't use `transparent` display on a struct with no source field",
                ));
            }

            quote! {
                self.source.fmt(f)
            }
        }
    };

    let struct_ident = &struct_.ident;
    let (impl_generics, ty_generics, where_clause) = struct_.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics core::fmt::Display for #struct_ident #ty_generics #where_clause {
            #[allow(unused_variables)]
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                #display
            }
        }
    })
}

fn generate_struct_error_impl(struct_: &ItemStruct) -> Result<TokenStream, Error> {
    let ty_ident = &struct_.ident;
    let (impl_generics, ty_generics, where_clause) = struct_.generics.split_for_impl();

    let source = if source_field_of(struct_.fields.iter()).is_some() {
        let error_attr = struct_
            .attrs
            .iter()
            .find(|a| a.path().is_ident("error"))
            .ok_or(Error::new(
                struct_.span(),
                "struct is missing #[error(...)] attribute",
            ))?;
        let error_attr_arg = error_attr.parse_args::<ErrorAttrArg>()?;

        if error_attr_arg == ErrorAttrArg::Transparent {
            quote! { self.source.source() }
        } else {
            quote! { Some(&self.source) }
        }
    } else {
        quote! { None }
    };

    Ok(quote! {
        impl #impl_generics ::core::error::Error for #ty_ident #ty_generics #where_clause {
            fn source(&self) -> Option<&(dyn ::core::error::Error + 'static)> {
                #source
            }
        }
    })
}

pub fn derive_err_struct(struct_: &ItemStruct) -> Result<TokenStream, Error> {
    let ctx_struct = source_field_of(struct_.fields.iter())
        .is_some()
        .then(|| generate_ctx(struct_))
        .transpose()?;
    let display_impl = generate_struct_display_impl(struct_)?;
    let error_impl = generate_struct_error_impl(struct_)?;

    Ok(quote! {
        #ctx_struct
        #display_impl
        #error_impl
    })
}
