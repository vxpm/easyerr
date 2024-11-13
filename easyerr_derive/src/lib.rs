mod enum_;
mod struct_;

use syn::{
    parse::Parse,
    parse_macro_input,
    spanned::Spanned,
    visit::{self, Visit},
    Error, Field, GenericParam, Generics, Ident, Item, Lifetime, LitStr, Type, TypePath,
};

#[derive(PartialEq, Eq)]
enum ErrorAttrArg {
    Str(LitStr),
    Transparent,
}

impl Parse for ErrorAttrArg {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ident: Result<Ident, _> = input.parse();
        if let Ok(ident) = ident {
            if ident == "transparent" {
                return Ok(Self::Transparent);
            }

            return Err(Error::new(
                input.span(),
                "unknown error argument. valid arguments are `transparent` or a format string.",
            ));
        }

        let format_str: Result<LitStr, _> = input.parse();
        if let Ok(format_str) = format_str {
            return Ok(Self::Str(format_str));
        }

        Err(Error::new(
            input.span(),
            "unknown error argument. valid arguments are `transparent` or a format string.",
        ))
    }
}

fn source_field_of<'f>(mut fields: impl Iterator<Item = &'f Field>) -> Option<&'f Field> {
    fields.find(|f| f.ident.as_ref().is_some_and(|i| i == "source"))
}

fn extract_source_field<'f>(fields: impl Iterator<Item = &'f Field>) -> (Vec<Field>, Option<Type>) {
    let mut source_ty = None;
    let fields = fields
        .filter(|&f| {
            f.ident.as_ref().is_some_and(|i| {
                if i == "source" {
                    source_ty = Some(f.ty.clone());
                    false
                } else {
                    true
                }
            })
        })
        .cloned()
        .collect();

    (fields, source_ty)
}

fn is_required_generic_for_type(ty: &Type, generic: &Ident) -> bool {
    struct PathVisitor<'g> {
        generic: &'g Ident,
        required: bool,
    }

    impl<'ast> Visit<'ast> for PathVisitor<'_> {
        fn visit_type_path(&mut self, node: &'ast TypePath) {
            if node.qself.is_none() {
                if let Some(first_segment) = node.path.segments.first() {
                    if first_segment.ident == *self.generic {
                        self.required = true;
                        return;
                    }
                }
            }

            visit::visit_type_path(self, node);
        }
    }

    let mut path_visitor = PathVisitor {
        generic,
        required: false,
    };

    path_visitor.visit_type(ty);
    path_visitor.required
}

fn is_required_lifetime_for_type(ty: &Type, lifetime: &Lifetime) -> bool {
    struct LifetimeVisitor<'l> {
        lifetime: &'l Lifetime,
        required: bool,
    }

    impl<'ast> Visit<'ast> for LifetimeVisitor<'_> {
        fn visit_lifetime(&mut self, node: &'ast Lifetime) {
            if node.ident == self.lifetime.ident {
                self.required = true;
                return;
            }

            visit::visit_lifetime(self, node);
        }
    }

    let mut lifetime_visitor = LifetimeVisitor {
        lifetime,
        required: false,
    };

    lifetime_visitor.visit_type(ty);
    lifetime_visitor.required
}

fn generics_required_by_type(generics: &Generics, ty: &Type) -> Vec<GenericParam> {
    let mut result = Vec::new();
    for generic in &generics.params {
        match generic {
            GenericParam::Lifetime(lifetime) => {
                if is_required_lifetime_for_type(ty, &lifetime.lifetime) {
                    result.push(generic.clone());
                }
            }
            GenericParam::Type(generic_ty) => {
                if is_required_generic_for_type(ty, &generic_ty.ident) {
                    result.push(generic.clone());
                }
            }
            GenericParam::Const(_) => todo!(),
        }
    }

    result
}

#[proc_macro_derive(Error, attributes(error))]
pub fn derive_error(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: Item = parse_macro_input!(input);

    let result = match input {
        Item::Enum(e) => enum_::derive_err_enum(&e),
        Item::Struct(s) => struct_::derive_err_struct(&s),
        _ => Err(syn::Error::new(input.span(), "Unsupported item")),
    };

    match result {
        Ok(ok) => ok.into(),
        Err(e) => e.into_compile_error().into(),
    }
}
