//! Wrapper for cleaner access to kdl type sizes.
//!
//! God I hate this, it's going to be very inneficient most likely,
//! if every time I query for size, I have to walk through the entire
//! document and check each value sizes and add up everything :/
use kdl::{KdlDocument, KdlEntry, KdlIdentifier, KdlNode};
use multierr_span::{Smarc, Sref};

pub type SpannedIdent<'a> = Sref<'a, KdlIdentifier>;
pub type SpannedEntry<'a> = Sref<'a, KdlEntry>;
pub(crate) type SpannedDocument = Smarc<KdlDocument>;
pub(crate) type SpannedNode = Smarc<KdlNode>;
