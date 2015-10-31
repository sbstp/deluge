extern crate deluge;

use deluge::rencode::encode;

fn main() {
    println!("{:#?}", encode(5000));
}
