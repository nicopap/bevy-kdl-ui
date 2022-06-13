//! Wrapper for cleaner access to kdl type sizes.
//!
//! God I hate this, it's going to be very inneficient most likely,
//! if every time I query for size, I have to walk through the entire
//! document and check each value sizes and add up everything :/
use kdl::{KdlDocument, KdlIdentifier, KdlNode};
use multierr_span::Smarc;

pub type SpannedIdent = Smarc<KdlIdentifier>;
pub(crate) type SpannedDocument = Smarc<KdlDocument>;
pub(crate) type SpannedNode = Smarc<KdlNode>;
