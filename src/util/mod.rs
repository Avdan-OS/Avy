pub mod size;

use std::any::Any;

pub use size::Size;

pub trait AsAny {
    fn as_any(self: Box<Self>) -> Box<dyn Any>;
    fn as_any_ref(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

#[macro_export]
macro_rules! impl_as_any {
    ($ty: ty) => {
        impl $crate::util::AsAny for $ty {
            fn as_any(self: Box<Self>) -> Box<dyn std::any::Any> {
                Box::new(self) as Box<dyn std::any::Any>
            }
            fn as_any_ref(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }
    };
}
