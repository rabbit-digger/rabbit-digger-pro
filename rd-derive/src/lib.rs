use proc_macro2::{Literal, Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{parse_macro_input, Data, DeriveInput, Fields, Ident};

fn call_all(input: &DeriveInput, method_path: TokenStream, args: TokenStream) -> TokenStream {
    let ident = &input.ident;
    let mut body = quote! {};

    match &input.data {
        Data::Struct(s) => {
            let fields = &s.fields;

            for field in fields {
                let field_name = &field.ident.clone().unwrap();
                let name = Literal::string(&field_name.to_string());
                body.extend(quote! {
                    ctx.push(#name);
                    #method_path(&mut self.#field_name, #args)?;
                    ctx.pop();
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
                            let field_name = &field.ident.clone().unwrap();
                            let name = Literal::string(&field_name.to_string());
                            head.extend(quote! { #field_name, });
                            inner.extend(quote! {
                                ctx.push(#name);
                                #method_path(#field_name, #args)?;
                                ctx.pop();
                            });
                        }
                        head = quote! { { #head } };
                    }
                    Fields::Unnamed(fields) => {
                        for (i, _f) in fields.unnamed.iter().enumerate() {
                            let field_name =
                                Ident::new(&format!("i{}", i), Span::call_site()).to_token_stream();
                            let name = Literal::string(&field_name.to_string());
                            head.extend(quote! { #field_name, });
                            inner.extend(quote! {
                                ctx.push(#name);
                                #method_path(#field_name, #args)?;
                                ctx.pop();
                            });
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
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let visitor_body = call_all(
        &input,
        quote! { rd_interface::config::Config::visit },
        quote! { ctx, visitor },
    );

    let expanded = quote! {
        impl #impl_generics rd_interface::config::Config for #ident #ty_generics #where_clause {
            fn visit(&mut self, ctx: &mut rd_interface::config::VisitorContext, visitor: &mut impl rd_interface::config::Visitor) -> rd_interface::Result<()> {
                #visitor_body
                Ok(())
            }
        }
    };

    expanded.into()
}

#[proc_macro_attribute]
pub fn rd_config(
    _metadata: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input: TokenStream = input.into();
    let output = quote! {
        #[derive(serde::Serialize, serde::Deserialize, rd_interface::Config, rd_interface::schemars::JsonSchema)]
        #input
    };
    output.into()
}
