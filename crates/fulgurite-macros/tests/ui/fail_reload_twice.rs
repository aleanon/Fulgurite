//! M55/T55.2 (R16-F4): a field's reload mode given twice is refused. The
//! last one used to win, so `#[reload(keep)] #[reload(fresh)]` dropped
//! the value the first attribute asked to keep.
use fulgurite::Reloadable;

#[derive(Reloadable, Default)]
struct Panel {
    #[reload(keep)]
    #[reload(fresh)]
    count: i64,
    #[reload(keep, reset)]
    other: i64,
}

fn main() {}
