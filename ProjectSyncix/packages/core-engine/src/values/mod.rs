//! Property values: parsing what a person typed, and the JSON shape on the wire.

pub(crate) mod colour;
pub(crate) mod parse;
pub(crate) mod wire;

pub(crate) use colour::*;
pub(crate) use parse::*;
pub(crate) use wire::*;
