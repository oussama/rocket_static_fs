use std::fs::File;
use std::path::Path;

fn main() {
    if cfg!(feature = "test_embedded") {
        let test_package_path = concat!(env!("CARGO_MANIFEST_DIR"), "/target/test.package");
        if !Path::new(test_package_path).is_file() {
            File::create(test_package_path).unwrap();
        }
    }
}
