use proc_macro::TokenStream as CompilerTokenStream;

use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput, Fields};

#[proc_macro_derive(AsStd140)]
pub fn derive_as_std140(input: CompilerTokenStream) -> CompilerTokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let visibility = input.vis;

    let name = input.ident;
    let std140_name = format_ident!("Std140{}", name);
    let alignment_mod_name = format_ident!("Std140{}Alignment", name);

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => fields,
            Fields::Unnamed(_) => panic!("Tuple structs are not supported"),
            Fields::Unit => panic!("Unit structs are not supported"),
        },
        Data::Enum(_) | Data::Union(_) => panic!("Only structs are supported"),
    };

    let mut alignment_calculators = Vec::new();
    let mut std140_fields = Vec::new();
    let mut initializer = Vec::new();

    for (index, field) in fields.named.iter().enumerate() {
        let field_name = field.ident.as_ref().unwrap();
        let field_ty = &field.ty;

        let align_name = format_ident!("_{}_align", field_name);

        let offset_accumulation = fields.named.iter().take(index).map(|field| {
            let ty = &field.ty;
            quote!(offset += ::std::mem::size_of::<#ty>();)
        });

        alignment_calculators.push(quote! {
            pub const fn #align_name() -> usize {
                let mut offset = 0;
                #( #offset_accumulation )*

                ::crevice::internal::align_offset(
                    offset,
                    ::std::mem::align_of::<<<#field_ty as ::crevice::std140::AsStd140>::Std140Type as ::crevice::std140::Std140>::Alignment>()
                )
            }
        });

        std140_fields.push(quote! {
            #align_name: [u8; #alignment_mod_name::#align_name()],
            #field_name: <#field_ty as ::crevice::std140::AsStd140>::Std140Type,
        });

        initializer.push(quote!(#field_name: self.#field_name.as_std140()));
    }

    // Build the output, possibly using quasi-quotation
    let expanded = quote! {
        #[allow(non_snake_case)]
        mod #alignment_mod_name {
            use super::*;

            #( #alignment_calculators )*
        }

        #[derive(Debug, Clone, Copy)]
        #[derive(::crevice::type_layout::TypeLayout)]
        #[repr(C)]
        #visibility struct #std140_name #ty_generics #where_clause {
            #( #std140_fields )*
        }

        unsafe impl #impl_generics ::crevice::bytemuck::Zeroable for #std140_name #ty_generics #where_clause {}
        unsafe impl #impl_generics ::crevice::bytemuck::Pod for #std140_name #ty_generics #where_clause {}

        unsafe impl #impl_generics ::crevice::std140::Std140 for #std140_name #ty_generics #where_clause {
            type Alignment = ::crevice::alignment::Align16;
        }

        impl #impl_generics ::crevice::std140::AsStd140 for #name #ty_generics #where_clause {
            type Std140Type = #std140_name;

            fn as_std140(&self) -> Self::Std140Type {
                Self::Std140Type {
                    #( #initializer, )*

                    ..::crevice::bytemuck::Zeroable::zeroed()
                }
            }
        }
    };

    // Hand the output tokens back to the compiler
    CompilerTokenStream::from(expanded)
}
