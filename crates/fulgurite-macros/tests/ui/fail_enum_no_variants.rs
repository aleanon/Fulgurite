//! M55/T55.2 (R16-F4): an enum widget with no variants is refused. It has
//! nothing to show, and the table would have only its own node.
use fulgurite::prelude::*;

#[derive(StaticTree)]
enum Nothing {}

fn main() {}
