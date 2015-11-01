use serde::de::{self, Deserialize, Deserializer, Error};
use serde::ser::{self, Serialize, Serializer};
use std::collections::BTreeMap;
use std::collections::btree_map;
use std::slice;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    None,
    I64(i64),
    U64(u64),
    F64(f64),
    Bool(bool),
    String(String),
    List(Vec<Value>),
    Dict(BTreeMap<String, Value>),
}

impl Serialize for Value {

    fn serialize<S: Serializer>(&self, serializer: &mut S) -> Result<(), S::Error> {
        match *self {
            Value::None => serializer.visit_none(),
            Value::Bool(v) => serializer.visit_bool(v),
            Value::I64(v) => serializer.visit_i64(v),
            Value::U64(v) => serializer.visit_u64(v),
            Value::F64(v) => serializer.visit_f64(v),
            Value::String(ref v) => serializer.visit_str(&v),
            Value::List(ref v) => serializer.visit_seq(SeqSerializer {
                iter: v.iter(),
                len: v.len(),
            }),
            Value::Dict(ref v) => serializer.visit_map(MapSerializer {
                iter: v.iter(),
                len: v.len(),
            }),
        }
    }
}

struct SeqSerializer<'a> {
    iter: slice::Iter<'a, Value>,
    len: usize,
}

impl<'a> ser::SeqVisitor for SeqSerializer<'a> {

    fn visit<S: Serializer>(&mut self, serializer: &mut S) -> Result<Option<()>, S::Error> {
        match self.iter.next() {
            Some(elem) => {
                try!(serializer.visit_seq_elt(elem));
                Ok(Some(()))
            }
            None => return Ok(None),
        }
    }

    fn len(&self) -> Option<usize> {
        Some(self.len)
    }

}

struct MapSerializer<'a> {
    iter: btree_map::Iter<'a, String, Value>,
    len: usize,
}

impl<'a> ser::MapVisitor for MapSerializer<'a> {

    fn visit<S: Serializer>(&mut self, serializer: &mut S) -> Result<Option<()>, S::Error> {
        match self.iter.next() {
            Some((key, val)) => {
                try!(serializer.visit_map_elt(key, val));
                Ok(Some(()))
            }
            None => return Ok(None),
        }
    }

    fn len(&self) -> Option<usize> {
        Some(self.len)
    }

}

struct ValueVisitor;

impl de::Visitor for ValueVisitor {

    type Value = Value;

    fn visit_none<E: Error>(&mut self) -> Result<Self::Value, E> {
        Ok(Value::None)
    }

    fn visit_bool<E: Error>(&mut self, v: bool) -> Result<Self::Value, E> {
        Ok(Value::Bool(v))
    }

    fn visit_i64<E: Error>(&mut self, v: i64) -> Result<Self::Value, E> {
        Ok(Value::I64(v))
    }

    fn visit_u64<E: Error>(&mut self, v: u64) -> Result<Self::Value, E> {
        Ok(Value::U64(v))
    }

    fn visit_f64<E: Error>(&mut self, v: f64) -> Result<Self::Value, E> {
        Ok(Value::F64(v))
    }

    fn visit_str<E: Error>(&mut self, v: &str) -> Result<Self::Value, E> {
        Ok(Value::String(v.into()))
    }

    fn visit_string<E: Error>(&mut self, v: String) -> Result<Self::Value, E> {
        Ok(Value::String(v))
    }

    fn visit_seq<V: de::SeqVisitor>(&mut self, visitor: V) -> Result<Value, V::Error> {
        let values = try!(de::impls::VecVisitor::new().visit_seq(visitor));
        Ok(Value::List(values))
    }

    fn visit_map<V: de::MapVisitor>(&mut self, visitor: V) -> Result<Value, V::Error> {
        let values = try!(de::impls::BTreeMapVisitor::new().visit_map(visitor));
        Ok(Value::Dict(values))
    }

}

impl Deserialize for Value {

    fn deserialize<D: Deserializer>(deserializer: &mut D) -> Result<Value, D::Error> {
        deserializer.visit(ValueVisitor)
    }

}
