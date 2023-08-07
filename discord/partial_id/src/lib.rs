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
    let partial_ty = Ident::new(&format!("Partial{}", ty), Span::call_site());

    let fields = filter_fields(match data {
        syn::Data::Struct(ref s) => &s.fields,
        _ => panic!("Field can only be derived for structs"),
    });

    let id_typ = &fields
        .iter()
        .find(|(_, ident, _)| ident.to_string() == "id")
        .unwrap()
        .2;

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
    let convert_none_branch = fields.iter().map(|(_vis, ident, _ty)| {
        if ident.to_string() == "id" {
            quote! {
                #ident: src
            }
        } else {
            quote! {
                #ident: ::core::option::Option::None
            }
        }
    });

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let tokens = quote! {

        #derive
        #vis struct #partial_ty #generics {
            #(#field_var),*
        }

        impl #impl_generics ::core::convert::From<#ty #ty_generics> for #partial_ty #ty_generics
            #where_clause
        {
            fn from(src: #ty #ty_generics) -> Self {
                Self {
                    #(#convert_branch),*
                }
            }
        }

        impl #impl_generics ::core::convert::From<#id_typ> for #partial_ty #ty_generics
            #where_clause
        {
            fn from(src: #id_typ) -> Self {
                Self {
                    #(#convert_none_branch),*
                }
            }
        }

        impl #impl_generics #ty #ty_generics
            #where_clause
        {
            #vis async fn update(&mut self, client: &crate::request::Bot) -> crate::request::Result<()> {
                *self = crate::request::Request::request(crate::request::HttpRequest::get(crate::resource::Endpoint::uri(&self.id)), client).await?;
                crate::request::Result::Ok(())
            }
        }

        impl #impl_generics #partial_ty #ty_generics
            #where_clause
        {
            #vis async fn update(&mut self, client: &crate::request::Bot) -> crate::request::Result<()> {
                let full: #ty = crate::request::Request::request(crate::request::HttpRequest::get(crate::resource::Endpoint::uri(&self.id)), client).await?;
                *self = full.into();
                crate::request::Result::Ok(())
            }
            #vis async fn get_field<T>(&mut self, client: &crate::request::Bot, f: fn(&Self) -> &::core::option::Option<T>) -> crate::request::Result<&T> {
                crate::request::Result::Ok(match f(self) {
                    ::core::option::Option::Some(_) => {
                        f(self).as_ref().unwrap()
                    }
                    ::core::option::Option::None => {
                        self.update(client).await?;
                        f(self).as_ref().unwrap()
                    }
                })
            }
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
