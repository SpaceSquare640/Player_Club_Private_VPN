//! Signaling comes in two independent flavors:
//! - Manual: Offer/Answer envelopes and the paste-robust blob codec (`blob`,
//!   `message`) — a user copies a blob out of band (chat, email, …).
//! - Networked (Phase G): a host runs [`server::SignalingServer`], other
//!   members connect over WebSocket, gatekept by network name + password
//!   (`protocol`). The client side and offer/answer relay land in later G
//!   phases; G.1 only establishes the member roster.

pub mod blob;
pub mod message;
pub mod protocol;
pub mod server;
