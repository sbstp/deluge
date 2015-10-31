use byteorder::{self, BigEndian, WriteBytesExt};
use serde::ser::{MapVisitor, SeqVisitor, Serialize, Serializer};
use std::{i8, i16, i32};
use std::io::{self, Write};

// type codes
const LIST: u8 = 59;
const DICT: u8 = 60;
//const INT: u8 = 61;
const I8: u8 = 62;
const I16: u8 = 63;
const I32: u8 = 64;
const I64: u8 = 65;
const F32: u8 = 66;
const F64: u8 = 44;
const TRUE: u8 = 67;
const FALSE: u8 = 68;
const NONE: u8 = 69;
const TERM: u8 = 127;

// i8 bounds
const I8_MIN: i64 = i8::MIN as i64;
const I8_MAX: i64 = i8::MAX as i64;

// i16 bounds
const I16_MIN: i64 = i16::MIN as i64;
const I16_MAX: i64 = i16::MAX as i64;

// i32 bounds
const I32_MIN: i64 = i32::MIN as i64;
const I32_MAX: i64 = i32::MAX as i64;

// positive integers with value embedded in typecode.
const INT_POS_FIXED_START: i64 = 0;
const INT_POS_FIXED_COUNT: i64 = 44;

// Negative integers with value embedded in typecode.
const INT_NEG_FIXED_START: i64 = 70;
const INT_NEG_FIXED_COUNT: i64 = -32;

// Strings with length embedded in typecode.
const STR_FIXED_START: usize = 128;
const STR_FIXED_COUNT: usize = 64;

// Lists with length embedded in typecode.
const LIST_FIXED_START: usize = STR_FIXED_START + STR_FIXED_COUNT;
const LIST_FIXED_COUNT: usize = 64;

// Dictionaries with length embedded in typecode.
const DICT_FIXED_START: usize = 102;
const DICT_FIXED_COUNT: usize = 25;

#[derive(Debug)]
pub enum Error {
    UnexpectedEOF,
    IoError(io::Error),
}

impl From<byteorder::Error> for Error {

    fn from(err: byteorder::Error) -> Error {
        match err {
            byteorder::Error::UnexpectedEOF => Error::UnexpectedEOF,
            byteorder::Error::Io(err) => Error::IoError(err),
        }
    }

}

impl From<io::Error> for Error {

    fn from(err: io::Error) -> Error {
        Error::IoError(err)
    }

}

struct Encoder<W: Write> {
    writer: W,
}

impl<W: Write> Serializer for Encoder<W> {

    type Error = Error;

    fn visit_bool(&mut self, v: bool) -> Result<(), Error> {
        let val = if v { TRUE } else { FALSE };
        try!(self.writer.write_u8(val));
        Ok(())
    }

    fn visit_i64(&mut self, v: i64) -> Result<(), Error> {
        match v {
            INT_NEG_FIXED_COUNT...-1 => {
                try!(self.writer.write_i8(INT_NEG_FIXED_START as i8 - 1 - v as i8));
            }
            0...INT_POS_FIXED_COUNT => {
                try!(self.writer.write_u8(INT_POS_FIXED_START as u8 + v as u8));
            }
            I8_MIN...I8_MAX => {
                try!(self.writer.write_u8(I8));
                try!(self.writer.write_i8(v as i8));
            }
            I16_MIN...I16_MAX => {
                try!(self.writer.write_u8(I16));
                try!(self.writer.write_i16::<BigEndian>(v as i16));
            }
            I32_MIN...I32_MAX => {
                try!(self.writer.write_u8(I32));
                try!(self.writer.write_i32::<BigEndian>(v as i32));
            }
            _ => {
                try!(self.writer.write_u8(I64));
                try!(self.writer.write_i64::<BigEndian>(v));
            }
        }
        Ok(())
    }

    fn visit_u64(&mut self, v: u64) -> Result<(), Error> {
        self.visit_i64(v as i64)
    }

    fn visit_f32(&mut self, v: f32) -> Result<(), Error> {
        try!(self.writer.write_u8(F32));
        try!(self.writer.write_f32::<BigEndian>(v));
        Ok(())
    }

    fn visit_f64(&mut self, v: f64) -> Result<(), Error> {
        try!(self.writer.write_u8(F64));
        try!(self.writer.write_f64::<BigEndian>(v));
        Ok(())
    }

    fn visit_unit(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn visit_none(&mut self) -> Result<(), Error> {
        try!(self.writer.write_u8(NONE));
        Ok(())
    }

    fn visit_some<V: Serialize>(&mut self, v: V) -> Result<(), Error> {
        v.serialize(self)
    }

    fn visit_str(&mut self, v: &str) -> Result<(), Error> {
        if v.len() < STR_FIXED_COUNT {
            try!(self.writer.write_u8(STR_FIXED_START as u8 + v.len() as u8));
            try!(self.writer.write_all(v.as_bytes()));
        } else {
            try!(write!(self.writer, "{}:{}", v.len(), v));
        }
        Ok(())
    }

    fn visit_seq<V: SeqVisitor>(&mut self, mut v: V) -> Result<(), Error> {
        match v.len() {
            Some(len) if len < LIST_FIXED_COUNT => {
                try!(self.writer.write_u8(LIST_FIXED_START as u8 + len as u8));
                while let Some(_) = try!(v.visit(self)) {}
                return Ok(());
            }
            Some(_) | None => {
                try!(self.writer.write_u8(LIST));
                while let Some(_) = try!(v.visit(self)) {}
                try!(self.writer.write_u8(TERM));
                Ok(())
            }
        }
    }

    fn visit_seq_elt<V: Serialize>(&mut self, v: V) -> Result<(), Error> {
        v.serialize(self)
    }

    fn visit_map<V: MapVisitor>(&mut self, mut v: V) -> Result<(), Error> {
        match v.len() {
            Some(len) if len < DICT_FIXED_COUNT => {
                try!(self.writer.write_u8(DICT_FIXED_START as u8 + len as u8));
                while let Some(_) = try!(v.visit(self)) {}
                return Ok(());
            }
            Some(_) | None => {
                try!(self.writer.write_u8(DICT));
                while let Some(_) = try!(v.visit(self)) {}
                try!(self.writer.write_u8(TERM));
                Ok(())
            }
        }
    }

    fn visit_map_elt<K: Serialize, V: Serialize>(&mut self, k: K, v: V) -> Result<(), Error> {
        try!(k.serialize(self));
        try!(v.serialize(self));
        Ok(())
    }

}

pub fn encode<S: Serialize>(v: S) -> Result<Vec<u8>, Error> {
    let mut encoder = Encoder {
        writer: Vec::new(),
    };
    try!(v.serialize(&mut encoder));
    Ok((encoder.writer))
}

struct Decoder<R: Reader> {
    reader: R,
}

impl Deserializer for Decoder {

    type Error = Error;

    fn visit<V>(&mut self, v: V) -> Result<V::Value, Error> {
        
    }

}

#[cfg(test)]
mod tests {
    use super::encode;
    use super::{DICT, LIST, TERM};
    use std::collections::HashMap;
    use std::iter::repeat;

    #[test]
    fn test_encode() {
        // integers
        assert_eq!(encode(5).unwrap(), &[5]);
        assert_eq!(encode(-5).unwrap(), &[74]);
        assert_eq!(encode(100).unwrap(), &[62, 100]);
        assert_eq!(encode(-100).unwrap(), &[62, 156]);
        assert_eq!(encode(200).unwrap(), &[63, 0, 200]);
        assert_eq!(encode(-200).unwrap(), &[63, 255, 56]);
        assert_eq!(encode(100_000).unwrap(), &[64, 0, 1, 134, 160]);
        assert_eq!(encode(-100_000).unwrap(), &[64, 255, 254, 121, 96]);
        assert_eq!(encode(400_000_000_000_i64).unwrap(), &[65, 0, 0, 0, 93, 33, 219, 160, 0]);
        assert_eq!(encode(-400_000_000_000_i64).unwrap(), &[65, 255, 255, 255, 162, 222, 36, 96, 0]);
        // strings
        assert_eq!(encode("abc").unwrap(), &[131, 97, 98, 99]);
        assert_eq!(encode("ghkdgdfjgdfjgfdgjhkdfgjhdfgfdjgdfjkgdfjhghfdgdfhkgdfhkgfdhgdfhgdfhdfghdfghkdfhdk").unwrap(),
                   "80:ghkdgdfjgdfjgfdgjhkdfgjhdfgfdjgdfjkgdfjhghfdgdfhkgdfhkgfdhgdfhgdfhdfghdfghkdfhdk".as_bytes());
        // list
        {
            assert_eq!(encode(&[1, 2]).unwrap(), &[194, 1, 2]);
            let list = repeat(1).take(80).collect::<Vec<u8>>();
            let data = encode(list).unwrap();
            assert_eq!(data.len(), 82);
            assert_eq!(data[0], LIST);
            assert_eq!(data[81], TERM);
        }
        // map
        {
            let mut map = HashMap::new();
            map.insert(1, "a");
            assert_eq!(encode(map).unwrap(), &[103, 1, 129, 97]);

            let mut map = HashMap::new();
            for i in 0..80 {
                map.insert(i, i);
            }
            let data = encode(map).unwrap();
            assert_eq!(data[0], DICT);
            assert_eq!(data.last(), Some(TERM).as_ref());
        }
    }

    #[test]
    fn test_decode() {

    }
}
