/// Conveniance macro for creating annotation types combining several annotations
#[macro_export]
macro_rules! annotation {
    {  $pub:vis struct $struct_name:ident $( < $( $param:ident ),* > )*
       {
           $( $ann_key:ident : $ann_type:ty ),* $( , )?

       }
       $( where $( $whereclause:tt )* )?

    } => {

        use std::borrow::Borrow as __Borrow;
        use $crate::annotations::ErasedAnnotation as __ErasedAnnotation;
        use $crate::annotations::Combine as __Combine;

        $pub struct $struct_name $( < $( $param ),* > )* {
            $ ( $ann_key : $ann_type ),*
        }

        impl<__T, $( $( $param ),* )* > From<__T> for $struct_name $( < $( $param ),* > )*
        where
            __T: Clone,
            $( $ann_type : From<__T> ),*
            $( , $( $whereclause )* )?

        {
            fn from(t: __T) -> Self {
                $struct_name {
                    $( $ann_key : t.clone().into() ),*
                }
            }
        }

        impl<__H, $( $( $param ),* )* > Content<__H> for $struct_name $( < $( $param ),* > )*
        where
            __H: ByteHash,
            $( $ann_type : Content<__H> ),*
            $( , $( $whereclause )* )?

        {
            fn persist(&mut self, sink: &mut Sink<__H>) -> io::Result<()> {
                $( self.$ann_key.persist(sink)? ; )*
                Ok(())
            }

            fn restore(source: &mut Source<__H>) -> io::Result<Self> {
                Ok($struct_name {
                    $( $ann_key : < $ann_type as Content<__H> >::restore(source)? , )*
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

        impl<__A, $( $( $param ),* )* > __Combine<__A> for $struct_name $( < $( $param ),* > )*
        where
            $( __A: __Borrow<$ann_type> ),* ,
            $( $( $whereclause )* )?
        {
            fn combine<__E>(elements: &[__E] ) -> Option<Self>     where
                __A: __Borrow<Self> + Clone,
                __E: __ErasedAnnotation<__A> {
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
