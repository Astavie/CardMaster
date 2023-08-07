use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_quote,
    spanned::Spanned,
    FnArg, ItemFn, ReturnType, Token, Type,
};

struct ResourceParams {
    result: Type,
    client: Option<Type>,
}

impl Parse for ResourceParams {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let result = input.parse()?;
        let client = if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;

            let ident: Ident = input.parse()?;
            if ident.to_string() != "client" {
                return Err(syn::Error::new(ident.span(), "invalid resource property"));
            }

            input.parse::<Token![=]>()?;
            Some(input.parse()?)
        } else {
            None
        };

        Ok(Self { result, client })
    }
}

fn resource_impl(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream2> {
    let params: ResourceParams = syn::parse(attr)?;
    let return_type = params.result;
    let client_type = match params.client {
        Some(t) => t,
        None => parse_quote!(crate::request::Discord),
    };

    let result_type = parse_quote!(
        ::std::pin::Pin<::std::boxed::Box<dyn ::futures_util::Future<Output = crate::request::Result<#return_type>> + ::std::marker::Send + 'resource>>
    );

    let mut get_request_fn: ItemFn = syn::parse(item)?;

    // get immediate fn signature
    let vis = get_request_fn.vis.clone();

    let mut sig = get_request_fn.sig.clone();
    let generic_params = sig.generics.params;
    sig.generics = parse_quote!(<'resource, #generic_params>);
    match &mut sig.output {
        ReturnType::Default => panic!(),
        ReturnType::Type(_, typ) => **typ = result_type,
    }

    // remove pattern matching from inputs and get names
    let inputs = sig
        .inputs
        .iter_mut()
        .skip(1) // skip self
        .enumerate()
        .map(|(n, arg)| match arg {
            FnArg::Receiver(_) => panic!(),
            FnArg::Typed(pt) => {
                let ty = &*pt.ty;
                match &*pt.pat {
                    syn::Pat::Ident(ident) => {
                        let id = &ident.ident;
                        let id_cloned = id.clone();
                        *arg = parse_quote!(#id: #ty);
                        id_cloned
                    }
                    _ => {
                        let id = Ident::new(&format!("arg__{}", n), pt.pat.span());
                        let id_cloned = id.clone();
                        *arg = parse_quote!(#id: #ty);
                        id_cloned
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    // insert client as input
    sig.inputs
        .insert(1, parse_quote!(client: &'resource #client_type));

    // set original fn name to {}_request
    let get_ident = get_request_fn.sig.ident;
    let get_request_ident = Ident::new(&format!("{}_request", get_ident), get_ident.span());
    get_request_fn.sig.ident = get_request_ident.clone();

    // return functions
    let tokens = quote! {
        #get_request_fn
        #vis #sig {
            crate::request::Request::request(self.#get_request_ident(#(#inputs),*), client)
        }
    };
    Ok(tokens)
}

#[proc_macro_attribute]
pub fn resource(attr: TokenStream, item: TokenStream) -> TokenStream {
    match resource_impl(attr, item) {
        Ok(params) => params.into(),
        Err(e) => return e.into_compile_error().into(),
    }
}
