use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput};

#[proc_macro_derive(Config)]
pub fn config(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let ident = input.ident;
    let mut resolve_body = quote! {};

    let fields = match input.data {
        Data::Struct(s) => s.fields,
        _ => panic!("Config must be Struct"),
    };

    for field in fields {
        let field_name = field.ident.unwrap();
        let line = quote! {
            rd_interface::registry::ResolveNetRef::resolve(&mut self.#field_name, nets)?;
        };
        resolve_body.extend(line);
    }

    let expanded = quote! {
        impl rd_interface::registry::ResolveNetRef for #ident {
            fn resolve(&mut self, nets: &rd_interface::registry::NetMap) -> rd_interface::Result<()> {
                #resolve_body
                Ok(())
            }
        }
    };

    TokenStream::from(expanded)
}
