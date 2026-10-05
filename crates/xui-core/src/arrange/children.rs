#![forbid(unsafe_code)]

//! [`IntoChildren`]: several layout children in one call.

use super::{Entry, IntoEntry};

/// Several children for [`Layout::children`](super::Layout::children): a
/// tuple of up to twelve builders, layouts or entries, or a `Vec` of any one
/// kind of them (for children made in a loop).
///
/// An empty `Vec` has no element type to infer, so name it:
/// `Vec::<Entry<_>>::new()`.
pub trait IntoChildren<M: 'static> {
    /// Appends each child to `entries`, in order.
    fn push_into(self, entries: &mut Vec<Entry<M>>);
}

impl<M: 'static, T: IntoEntry<M>> IntoChildren<M> for Vec<T> {
    fn push_into(self, entries: &mut Vec<Entry<M>>) {
        entries.extend(self.into_iter().map(IntoEntry::into_entry));
    }
}

/// Implements [`IntoChildren`] for a tuple of [`IntoEntry`] values.
macro_rules! tuple_children {
    ($($name:ident),+) => {
        impl<M: 'static, $($name: IntoEntry<M>),+> IntoChildren<M> for ($($name,)+) {
            #[allow(non_snake_case)]
            fn push_into(self, entries: &mut Vec<Entry<M>>) {
                let ($($name,)+) = self;
                $(entries.push($name.into_entry());)+
            }
        }
    };
}

tuple_children!(A);
tuple_children!(A, B);
tuple_children!(A, B, C);
tuple_children!(A, B, C, D);
tuple_children!(A, B, C, D, E);
tuple_children!(A, B, C, D, E, F);
tuple_children!(A, B, C, D, E, F, G);
tuple_children!(A, B, C, D, E, F, G, H);
tuple_children!(A, B, C, D, E, F, G, H, I);
tuple_children!(A, B, C, D, E, F, G, H, I, J);
tuple_children!(A, B, C, D, E, F, G, H, I, J, K);
tuple_children!(A, B, C, D, E, F, G, H, I, J, K, L);
