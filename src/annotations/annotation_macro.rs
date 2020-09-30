// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

/// Conveniance macro for creating annotation types combining several annotations
#[macro_export]
macro_rules! annotation {
    {
        $(#[$outer:meta])*
        $pub:vis struct $struct_name:ident $( < $( $param:ident ),* > )*
        {
            $( $ann_key:ident : $ann_type:ty ),* $( , )?
        }
        $( where $( $whereclause:tt )* )?

    } => {

        use std::borrow::Borrow as __Borrow;
        use $crate::annotations::ErasedAnnotation as __ErasedAnnotation;
        use $crate::annotations::Combine as __Combine;
        use $crate::{
            Content as __Content,
            Sink as __Sink,
            Source as __Source,
            ByteHash as __ByteHash
        };

        $(#[$outer])*
        $pub struct $struct_name $( < $( $param ),* > )* {
            $ ( $ann_key : $ann_type ),*
        }

        impl<'a, T, $( $( $param ),* )* > From<&'a T> for $struct_name $( < $( $param ),* > )*
        where
            T: Clone,
            $( for<'any> &'any T: Into<$ann_type> ),*
            $( , $( $whereclause )* )?

        {
            fn from(t: &T) -> Self {
                $struct_name {
                    $( $ann_key : t.into() ),*
                }
            }
        }

        impl<H, $( $( $param ),* )* > __Content<H> for $struct_name $( < $( $param ),* > )*
        where
            H: __ByteHash,
            $( $ann_type : __Content<H> ),*
            $( , $( $whereclause )* )?

        {
            fn persist(&mut self, sink: &mut __Sink<H>) -> io::Result<()> {
                $( self.$ann_key.persist(sink)? ; )*
                Ok(())
            }

            fn restore(source: &mut __Source<H>) -> io::Result<Self> {
                Ok($struct_name {
                    $( $ann_key : < $ann_type as __Content<H> >::restore(source)? , )*
                })

            }
        }

        impl<$( $( $param ),* )* > Clone for $struct_name $( < $( $param ),* > )*
        where
            $( $ann_type : Clone ),*
            $( , $( $whereclause )* )?

        {
            fn clone(&self) -> Self {
                $struct_name {
                    $( $ann_key : self.$ann_key.clone() ),*
                }
            }
        }

        impl<A, $( $( $param ),* )* > __Combine<A> for $struct_name $( < $( $param ),* > )*
        where
            $( A: __Borrow<$ann_type> ),* ,
            $( $( $whereclause )* )?
        {
            fn combine<E>(elements: &[E] ) -> Option<Self>     where
                A: __Borrow<Self> + Clone,
                E: __ErasedAnnotation<A> {
                Some($struct_name {
                    $(
                        $ann_key : if let Some(combined) = < $ann_type >::combine(elements) {
                            combined
                        } else {
                            return None
                        }
                    ),*
                })
            }
        }

        macro_rules! impl_borrow {
            ($sub_ann_key:ident : $sub_ann_type:ty) => {
                impl<$( $( $param ),* )* > __Borrow<$sub_ann_type>

                    for $struct_name $( < $( $param ),* > )*
                    $( where $( $whereclause )* )? {
                    fn borrow(&self) -> & $sub_ann_type {
                        &self.$sub_ann_key
                    }
                }
            }
        }

        macro_rules! impl_as_ref {
            ($sub_ann_key:ident : $sub_ann_type:ty) => {
                impl<$( $( $param ),* )* > AsRef<$sub_ann_type>
                    for $struct_name $( < $( $param ),* > )*
                    $( where $( $whereclause )* )?
                {
                    fn as_ref(&self) -> & $sub_ann_type {
                        &self.$sub_ann_key
                    }
                }
            }
        }

        macro_rules! impl_borrow_ref {
            ($sub_ann_key:ident : $sub_ann_type:ty) => {
                impl<'__a, $( $( $param ),* )* > __Borrow<$sub_ann_type>

                    for &'__a $struct_name $( < $( $param ),* > )*
                {
                    fn borrow(&self) -> & $sub_ann_type {
                        &self.$sub_ann_key
                    }
                }
            }
        }

        $(
            impl_as_ref! { $ann_key : $ann_type }
            impl_borrow! { $ann_key : $ann_type }
            impl_borrow_ref! { $ann_key : $ann_type }
        )*
    }
}
