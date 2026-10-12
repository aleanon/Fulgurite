use fulgurite::Reloadable;

#[derive(Reloadable, Default)]
struct S {
    #[reload(kepe)]
    n: u32,
}

fn main() {}
