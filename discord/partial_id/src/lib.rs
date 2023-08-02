use core::iter::FromIterator;
use proc_macro2::{Ident, Span};
use quote::{quote, ToTokens};
use syn::{DeriveInput, Fields, Type, Visibility};

#[proc_macro_derive(Partial)]
pub fn derive_partial(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let DeriveInput {
        attrs,
        vis,
        ident: ty,
        generics,
        data,
        ..
    } = syn::parse(input).unwrap();
    let mut derives = Vec::new();
    for attr in attrs.into_iter() {
        if attr.path.is_ident("derive") {
            derives.push(attr.into_token_stream());
        }
    }
    let derive = if derives.is_empty() {
        proc_macro2::TokenStream::new()
    } else {
        proc_macro2::TokenStream::from_iter(derives.into_iter())
    };
    let partial_ident = Ident::new(&format!("Partial{}", ty), Span::call_site());

    let fields = filter_fields(match data {
        syn::Data::Struct(ref s) => &s.fields,
        _ => panic!("Field can only be derived for structs"),
    });

    let field_var = fields.iter().map(|(vis, ident, ty)| {
        if ident.to_string() == "id" {
            quote! {
                #vis #ident: #ty
            }
        } else {
            quote! {
                #vis #ident: ::core::option::Option<#ty>
            }
        }
    });
    let convert_branch = fields.iter().map(|(_vis, ident, _ty)| {
        if ident.to_string() == "id" {
            quote! {
                #ident: src.#ident
            }
        } else {
            quote! {
                #ident: ::core::option::Option::Some(src.#ident)
            }
        }
    });

    let get_field = fields.iter().map(|(vis, ident, ty)| {
        if ident.to_string() == "id" {
            quote!{}
        } else {
            let get_ident = Ident::new(&format!("get_{}", ident), Span::call_site());
            let get_ident_mut = Ident::new(&format!("get_{}_mut", ident), Span::call_site());
            quote! {
                #vis async fn #get_ident(&mut self, client: &crate::request::Discord) -> crate::request::Result<&#ty> {
                    crate::request::Result::Ok(match self.#ident {
                        ::core::option::Option::Some(ref v) => v,
                        ::core::option::Option::None => {
                            crate::resource::Updatable::update(self, client).await?;
                            self.#ident.as_ref().unwrap()
                        }
                    })
                }
                #vis async fn #get_ident_mut(&mut self, client: &crate::request::Discord) -> crate::request::Result<&mut #ty> {
                    crate::request::Result::Ok(match self.#ident {
                        ::core::option::Option::Some(ref mut v) => v,
                        ::core::option::Option::None => {
                            crate::resource::Updatable::update(self, client).await?;
                            self.#ident.as_mut().unwrap()
                        }
                    })
                }
            }
        }
    });

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let tokens = quote! {
        #derive
        #vis struct #partial_ident #ty_generics
            #where_clause
        {
            #(#field_var),*
        }

        impl #impl_generics ::core::convert::From<#ty #ty_generics> for #partial_ident #ty_generics
            #where_clause
        {
            fn from(src: #ty #ty_generics) -> #partial_ident #ty_generics {
                #partial_ident {
                    #(#convert_branch),*
                }
            }
        }

        impl #impl_generics #partial_ident #ty_generics
            #where_clause
        {
            #(#get_field)*
        }
    };
    tokens.into()
}

fn filter_fields(fields: &Fields) -> Vec<(Visibility, Ident, Type)> {
    fields
        .iter()
        .filter_map(|field| {
            if field.ident.is_some() {
                let field_vis = field.vis.clone();
                let field_ident = field.ident.as_ref().unwrap().clone();
                let field_ty = field.ty.clone();
                Some((field_vis, field_ident, field_ty))
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
}
