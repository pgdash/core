use std::fmt::Display;

trait DatabaseRow {
    fn get_str(&self, name: &str) -> &str;
    fn get_opt_str(&self, name: &str) -> Option<&str>;
}

struct MockRow {
    data: String,
}

impl DatabaseRow for MockRow {
    fn get_str(&self, name: &str) -> &str {
        &self.data
    }
    fn get_opt_str(&self, name: &str) -> Option<&str> {
        Some(&self.data)
    }
}

fn main() {
    let row = MockRow { data: "test".to_string() };
    println!("{}", row.get_str("a"));
}
