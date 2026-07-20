//! NAT traversal: STUN reflexive discovery and candidate gathering.
//! Phase C1 ships STUN + candidates; hole-punching arrives in C4.

pub mod candidate;
pub mod stun;
