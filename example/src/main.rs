use askama::Template;

fn main() {
    println!("Hello, world!");
}

#[derive(Template)]
#[template(path = "index.html")]
pub struct Page {
    pub age: u32,
    pub name: String,
}
