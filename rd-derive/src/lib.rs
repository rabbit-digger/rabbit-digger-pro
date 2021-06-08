use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput};

#[proc_macro_derive(Config)]
pub fn config(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let ident = input.ident;
    let mut resolve_body = quote! {};
    let mut get_dependency_set_body = quote! {};

    match input.data {
        Data::Struct(s) => {
            let fields = s.fields;

            for field in fields {
                let field_name = field.ident.unwrap();
                resolve_body.extend(quote! {
                    rd_interface::registry::ResolveNetRef::resolve(&mut self.#field_name, nets)?;
                });
                get_dependency_set_body.extend(quote! {
                    rd_interface::registry::ResolveNetRef::get_dependency_set(&mut self.#field_name, nets)?;
                });
            }
        }
        Data::Enum(_e) => {
            // TODO: support enum
        }
        _ => panic!("Config must be struct or enum"),
    };

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

    TokenStream::from(expanded)
}
