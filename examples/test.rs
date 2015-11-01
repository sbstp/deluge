#![feature(custom_derive, plugin)]
#![plugin(serde_macros)]

extern crate deluge;
extern crate serde;

use deluge::rencode::{decode, encode};

#[derive(Serialize, Deserialize, Debug)]
struct Msg {
    name: String,
    code: i32,
}

fn main() {
    let orig = Msg {
        name: "bob".into(),
        code: -133,
    };
    let data = encode(orig).unwrap();
    let dest: Msg = decode(&data[..]).unwrap();
    println!("{:#?}", dest);
}
