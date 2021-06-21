use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{parse_macro_input, Data, DeriveInput, Fields, Ident};

fn call_all(input: &DeriveInput, method_path: TokenStream, args: TokenStream) -> TokenStream {
    let ident = &input.ident;
    let mut body = quote! {};

    match &input.data {
        Data::Struct(s) => {
            let fields = &s.fields;

            for field in fields {
                let field_name = &field.ident;
                body.extend(quote! {
                    #method_path(&mut self.#field_name, #args)?;
                });
            }
        }
        Data::Enum(e) => {
            for variant in &e.variants {
                let variant_name = &variant.ident;
                let mut inner = TokenStream::new();
                let mut head = TokenStream::new();

                match &variant.fields {
                    Fields::Named(fields) => {
                        for field in &fields.named {
                            let field_name = &field.ident;
                            head.extend(quote! { #field_name, });
                            inner.extend(quote! { #method_path(#field_name, #args)?; });
                        }
                        head = quote! { { #head } };
                    }
                    Fields::Unnamed(fields) => {
                        for (i, _) in fields.unnamed.iter().enumerate() {
                            let field_name =
                                Ident::new(&format!("i{}", i), Span::call_site()).to_token_stream();
                            head.extend(quote! { #field_name, });
                            inner.extend(quote! { #method_path(#field_name, #args)?; });
                        }
                        head = quote! { (#head) };
                    }
                    Fields::Unit => {}
                }

                body.extend(quote! {
                    #ident::#variant_name #head => {
                        #inner
                    }
                })
            }

            body = quote! {
                match self {
                    #body
                }
            };
        }
        _ => panic!("Config must be struct or enum"),
    }

    body
}

#[proc_macro_derive(Config)]
pub fn config(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let ident = &input.ident;

    let resolve_body = call_all(
        &input,
        quote! { rd_interface::registry::ResolveNetRef::resolve },
        quote! { nets },
    );
    let get_dependency_set_body = call_all(
        &input,
        quote! { rd_interface::registry::ResolveNetRef::get_dependency_set },
        quote! { nets },
    );

    let expanded = quote! {
        impl rd_interface::registry::ResolveNetRef for #ident {
            fn resolve(&mut self, nets: &rd_interface::registry::NetMap) -> rd_interface::Result<()> {
                #resolve_body
                Ok(())
            }
            fn get_dependency_set(&mut self, nets: &mut std::collections::HashSet<String>) -> rd_interface::Result<()> {
                #get_dependency_set_body
                Ok(())
            }
        }
    };

    expanded.into()
}
