#![feature(custom_derive, plugin)]
#![plugin(serde_macros)]

extern crate deluge;
extern crate serde;

use deluge::rencode::{decode, encode};
use deluge::rencode::Value;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug)]
struct Msg {
    name: Value,
    code: Value,
}

fn main() {
    let orig = Msg {
        name: Value::String("abc".into()),
        code: Value::I64(-133),
    };
    let data = encode(orig).unwrap();
    let dest: HashMap<String, Value> = decode(&data[..]).unwrap();
    println!("{:#?}", dest);
}
