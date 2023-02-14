use kdl::{KdlDocument, KdlEntry, KdlIdentifier, KdlNode, KdlValue};

use crate::{Length, Sbor, Sref};

// TODO: consider this: a KdlNode has .trailing and .leading stuff, but the actual
// span of the KdlNode is everything between, not containing the .trailing etc.

impl Length for KdlIdentifier {
    fn length(&self) -> u32 {
        self.repr().map_or(self.value().length(), |s| s.length())
    }
}
impl Length for KdlEntry {
    fn inner_length(&self) -> u32 {
        self.name().map_or(0, |t| t.length() + 1)
            + self.ty().map_or(0, |t| t.length() + 2)
            + self
                .value_repr()
                .map_or_else(|| self.value().length(), |t| t.length())
    }
    fn leading(&self) -> u32 {
        self.leading().length()
    }
    fn trailing(&self) -> u32 {
        self.trailing().length()
    }
    fn length(&self) -> u32 {
        self.inner_length() + Length::leading(self) + Length::trailing(self)
    }
}
impl Length for KdlDocument {
    fn inner_length(&self) -> u32 {
        self.nodes().length()
    }
    fn leading(&self) -> u32 {
        self.leading().length()
    }
    fn trailing(&self) -> u32 {
        self.trailing().length()
    }
    fn length(&self) -> u32 {
        self.inner_length() + Length::leading(self) + Length::trailing(self)
    }
}
impl Length for KdlNode {
    fn inner_length(&self) -> u32 {
        self.ty().map_or(0, |t| t.length() + 2)
            + self.name().length()
            + self.entries().length()
            + self.before_children().length()
            + self.children().map_or(0, |t| t.length() + 2)
    }
    fn leading(&self) -> u32 {
        self.leading().length()
    }
    fn trailing(&self) -> u32 {
        self.trailing().length()
    }
    fn length(&self) -> u32 {
        self.inner_length() + Length::leading(self) + Length::trailing(self)
    }
}
impl Length for KdlValue {
    fn length(&self) -> u32 {
        let must_escape = ['\n', '\\', '"', '\r', '\t', '\u{08}', '\u{0C}'];
        match self {
            KdlValue::Base10Float(value) => {
                let clean = match () {
                    () if value == &f64::INFINITY => f64::MAX,
                    () if value == &f64::NEG_INFINITY => -f64::MAX,
                    () if value.is_nan() => 0.0,
                    () => *value,
                };
                format!("{clean:?}").len() as u32
            }
            KdlValue::Bool(true) => 4,
            KdlValue::Bool(false) => 5,
            KdlValue::Null => 4,
            KdlValue::RawString(_) => format!("{self}").len() as u32,
            KdlValue::String(s) => (s.len() + 2 + s.matches(must_escape).count()) as u32,
            KdlValue::Base2(value) => 2 + (64 - value.leading_zeros()),
            KdlValue::Base8(value) => 2 + ((64 - value.leading_zeros()) / 3),
            KdlValue::Base16(value) => 2 + ((64 - value.leading_zeros()) / 4),
            KdlValue::Base10(value) => format!("{value:?}").len() as u32,
        }
    }
}
impl<'a> Sref<'a, KdlIdentifier> {
    pub fn sref_str(self) -> Sref<'a, str> {
        self.map(|t| t.value())
    }
}
macro_rules! offset {
    // Compute lengths for offset
    (@length $sel:ident optional ($before:expr, $after:expr) $method:ident) => (
        $sel.inner.$method().map_or(0, |t| t.length() + $before + $after)
    );
    (@length $sel:ident fallback($alt:ident) $method:ident) => (
        $sel.inner
            .$alt()
            .map_or_else(|| $sel.inner.$method().length(), |t| t.length())
    );
    (@length $sel:ident hidden $method:ident) => ($sel.inner.$method().length());
    (@length $sel:ident proxy $method:ident) => ($sel.inner.$method().length());
    (@length $sel:ident array $method:ident) => ($sel.inner.$method().length());
    // Compute offsets based on previously encoutered fields
    // We call this branch from inside the method definitions because you need access
    // to `self`.
    ($sel:ident [$(, $modif:ident ($($mod_args:tt)?) $method:ident )*]) => (
        $sel.offset $(+ offset!(@length $sel $modif $($mod_args)? $method))*
    );
}
macro_rules! method {
    // method proxies
    ($a:lifetime array $offsets:tt $method:ident $type:ty) => (
        pub fn $method(&self) -> impl Iterator<Item = Sref<$a, $type>> + $a {
            let offset = offset!(self $offsets);
            self.inner.$method().iter().scan(offset, |offset, elem| {
                let current_offset = *offset;
                *offset += elem.length();
                Some(Sbor::new(elem, current_offset))
            })
        }
    );
    ($a:lifetime hidden $_1:tt  $_2:ident) => ();
    ($a:lifetime optional($before:expr, $_:expr) $offsets:tt $method:ident $type:ty) => (
        pub fn $method(&self) -> Option<Sref<$a, $type>> {
            let offset = offset!(self $offsets) + $before;
            self.inner.$method().map(|t| Sbor::new(t, offset))
        }
    );
    ($a:lifetime proxy $offsets:tt $method:ident $type:ty) => (
        pub fn $method(&self) -> Sref<$a, $type> {
            let offset = offset!(self $offsets);
            Sbor::new(self.inner.$method(), offset)
        }
    );
    ($a:lifetime fallback($_:ident) $offsets:tt $method:ident $type:ty ) => (
        pub fn $method(&self) -> Sref<$a, $type> {
            let offset = offset!(self $offsets);
            Sbor::new(self.inner.$method(), offset)
        }
    );
}
#[cfg(feature = "mappable-rc-impls")]
use crate::{Smarc, Smrc};
#[cfg(feature = "mappable-rc-impls")]
use mappable_rc::{Marc, Mrc};

macro_rules! mrc_method {
    // method proxies
    ($b:ident, $rc:ident, array $offsets:tt $method:ident $type:ty) => (
        pub fn $method(&self) -> impl Iterator<Item = $b<$type>> {
            let mut offset = offset!(self $offsets);
            let mut i = 0;
            let inner = self.inner.clone();
            std::iter::from_fn(move || {
                let current_offset = offset;
                let elem = $rc::try_map(inner.clone(), |t| t.$method().get(i)).ok()?;
                offset += elem.length();
                i += 1;
                Some(Sbor::new(elem, current_offset))
            })
        }
    );
    ($b:ident, $rc:ident, optional($before:expr, $_:expr) $offsets:tt $method:ident $type:ty) => (
        pub fn $method(&self) -> Option<$b<$type>> {
            let offset = offset!(self $offsets) + $before;
            $rc::try_map(self.inner.clone(), |t| t.$method())
                .map(|t| Sbor::new(t, offset))
                .ok()
        }
    );
    ($b:ident, $rc:ident, proxy $offsets:tt $method:ident $type:ty) => (
        pub fn $method(&self) -> $b<$type> {
            let offset = offset!(self $offsets);
            Sbor::new($rc::map(self.inner.clone(), |t| t.$method()), offset)
        }
    );
    ($b:ident, $rc:ident, fallback($_:ident) $offsets:tt $method:ident $type:ty ) => (
        pub fn $method(&self) -> $b<$type> {
            let offset = offset!(self $offsets);
            Sbor::new($rc::map(self.inner.clone(), |t| t.$method()), offset)
        }
    );
    ($_3:ident, $_4:ident, hidden $_1:tt  $_2:ident) => ();
}
macro_rules! impl_spanned_proxies {
    (
        @methods_mrc $b:ident, $rc:ident [$($offsets:tt)*]
        #[$modif:ident $($mod_args:tt)?]
        fn $method:ident () $(-> $type:ty)?;
        $($whatever:tt)*
    ) => (
        mrc_method!{ $b, $rc, $modif $($mod_args)? [$($offsets)*] $method $($type)?}
        impl_spanned_proxies!{
            @methods_mrc $b, $rc [$($offsets)*, $modif ($($mod_args)?) $method]
            $($whatever)*
        }
    );
    (
        @methods $a:lifetime [$($offsets:tt)*]
        #[$modif:ident $($mod_args:tt)?]
        fn $method:ident () $(-> $type:ty)?;
        $($whatever:tt)*
    ) => (
        method!{ $a $modif $($mod_args)? [$($offsets)*] $method $($type)?}
        impl_spanned_proxies!{
            @methods $a [$($offsets)*, $modif ($($mod_args)?) $method]
            $($whatever)*
        }
    );
    (@methods $_1:lifetime $_:tt) => ();
    (@methods_mrc $_1:ident, $_2:ident $_:tt) => ();
    (impl Sbor<$type:ident> {$($methods:tt)*} ) => (
        impl<'a> Sref<'a, $type> {
            impl_spanned_proxies! { @methods 'a [] $($methods)* }
        }
        #[cfg(feature = "mappable-rc-impls")]
        impl Smrc<$type> {
            impl_spanned_proxies! { @methods_mrc  Smrc, Mrc [] $($methods)* }
        }
        #[cfg(feature = "mappable-rc-impls")]
        impl Smarc<$type> {
            impl_spanned_proxies! { @methods_mrc Smarc, Marc [] $($methods)* }
        }
    );
}
impl_spanned_proxies! {
    impl Sbor<KdlDocument> {
        #[hidden] fn leading();
        #[array]  fn nodes() -> KdlNode;
        #[hidden] fn trailing();
    }
}
impl_spanned_proxies! {
    impl Sbor<KdlNode> {
        #[hidden] fn leading();
        #[optional(1,1)] fn ty() -> KdlIdentifier;
        #[proxy] fn name() -> KdlIdentifier;
        #[array] fn entries() -> KdlEntry;
        #[hidden] fn before_children();
        #[optional(1,1)] fn children() -> KdlDocument;
        #[hidden] fn trailing();
    }
}
impl_spanned_proxies! {
    impl Sbor<KdlEntry> {
        #[hidden] fn leading();
        #[optional(0, 1)] fn name() -> KdlIdentifier;
        #[optional(1, 1)] fn ty() -> KdlIdentifier;
        #[fallback(value_repr)] fn value() -> KdlValue;
        #[hidden] fn trailing();
    }
}
