use byteorder::{self, BigEndian, ReadBytesExt};
use serde::de::{Deserialize, Deserializer, Error as DeserializerError, MapVisitor, SeqVisitor, Visitor};
use std::io::{self, Read};
use std::num::ParseIntError;
use std::string::FromUtf8Error;

use super::consts::*;

// type code ranges 0..43, 70..101, 102..127, 128..191, 192..255
//                   int    -int     dict      string    list

// positive integers with value embedded in typecode.
const INT_POS_FIXED_START: u8 = 0;
const INT_POS_FIXED_COUNT: u8 = 44;
const INT_POS_FIXED_END: u8 = INT_POS_FIXED_START + INT_POS_FIXED_COUNT - 1;

// Negative integers with value embedded in typecode.
const INT_NEG_FIXED_START: u8 = 70;
const INT_NEG_FIXED_COUNT: u8 = 32;
const INT_NEG_FIXED_END: u8 = INT_NEG_FIXED_START + INT_NEG_FIXED_COUNT - 1;

// Strings with length embedded in typecode.
const STR_FIXED_START: u8 = 128;
const STR_FIXED_COUNT: u8 = 64;
const STR_FIXED_END: u8 = STR_FIXED_START + STR_FIXED_COUNT - 1;

// Lists with length embedded in typecode.
const LIST_FIXED_START: u8 = STR_FIXED_START + STR_FIXED_COUNT;
const LIST_FIXED_COUNT: u8 = 64;
const LIST_FIXED_END: u8 = LIST_FIXED_START - 1 + LIST_FIXED_COUNT;

// Dictionaries with length embedded in typecode.
const DICT_FIXED_START: u8 = 102;
const DICT_FIXED_COUNT: u8 = 25;
const DICT_FIXED_END: u8 = DICT_FIXED_START + DICT_FIXED_COUNT - 1;

#[derive(Debug)]
pub enum Error {
    EndOfStream,
    EndOfStruct,
    FromUtf8Error(FromUtf8Error),
    IoError(io::Error),
    MissingField(&'static str),
    ParseIntError(ParseIntError),
    Syntax(String),
    UnexpectedEOF,
    UnknownField(String),
}

impl From<byteorder::Error> for Error {

    fn from(err: byteorder::Error) -> Error {
        match err {
            byteorder::Error::Io(err) => Error::IoError(err),
            byteorder::Error::UnexpectedEOF => Error::UnexpectedEOF,
        }
    }

}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::IoError(err)
    }
}

impl From<FromUtf8Error> for Error {
    fn from(err: FromUtf8Error) -> Error {
        Error::FromUtf8Error(err)
    }
}

impl From<ParseIntError> for Error {
    fn from(err: ParseIntError) -> Error {
        Error::ParseIntError(err)
    }
}

impl DeserializerError for Error {

    fn syntax(msg: &str) -> Error {
        Error::Syntax(msg.into())
    }

    fn end_of_stream() -> Error {
        Error::EndOfStream
    }

    fn unknown_field(field: &str) -> Error {
        Error::UnknownField(field.into())
    }

    fn missing_field(field: &'static str) -> Error {
        Error::MissingField(field)
    }

}

struct Decoder<R: Read> {
    reader: R,
    peek: Option<u8>,
}

impl<R: Read> Decoder<R> {

    fn next(&mut self) -> Result<u8, Error> {
        match self.peek.take() {
            Some(byte) => Ok(byte),
            None => self.reader.read_u8().map_err(From::from),
        }
    }

    fn peek(&mut self) -> Result<u8, Error> {
        // make sure the peak is empty so that next doesn't use it
        self.peek.take();

        match self.next() {
            Ok(byte) => {
                self.peek = Some(byte);
                Ok(byte)
            },
            Err(err) => Err(From::from(err)),
        }
    }

    fn take_while<P: FnMut(u8) -> bool>(&mut self, mut pred: P) -> Result<Vec<u8>, Error> {
        let mut buff = Vec::new();
        loop {
            match self.next() {
                Ok(byte) => {
                    if pred(byte) {
                        buff.push(byte);
                    } else {
                        return Ok(buff);
                    }
                }
                Err(err) => return Err(From::from(err)),
            }
        }
    }

    fn take(&mut self, mut n: usize) -> Result<Vec<u8>, Error> {
        let mut buff = Vec::new();

        if n == 0 {
            return Ok(buff);
        }

        while n > 0 {
            match self.next() {
                Ok(byte) => buff.push(byte),
                Err(err) => return Err(From::from(err)),
            }
            n -= 1;
        }

        Ok(buff)
    }

    fn parse_string(&mut self) -> Result<String, Error> {
        let numstr = try!(String::from_utf8(try!(self.take_while(|b| b != b':'))));
        let num: usize = try!(numstr.parse());
        let newstr = try!(String::from_utf8(try!(self.take(num))));
        Ok(newstr)
    }

    fn parse_embed_string(&mut self, byte: u8) -> Result<String, Error> {
        let len = byte - STR_FIXED_START;
        self.peek.take();
        let newstr = try!(String::from_utf8(try!(self.take(len as usize))));
        Ok(newstr)
    }

    fn parse_i8(&mut self) -> Result<i8, Error> {
        self.reader.read_i8().map_err(From::from)
    }

    fn parse_i16(&mut self) -> Result<i16, Error> {
        self.reader.read_i16::<BigEndian>().map_err(From::from)
    }

    fn parse_i32(&mut self) -> Result<i32, Error> {
        self.reader.read_i32::<BigEndian>().map_err(From::from)
    }

    fn parse_i64(&mut self) -> Result<i64, Error> {
        self.reader.read_i64::<BigEndian>().map_err(From::from)
    }

    fn parse_f32(&mut self) -> Result<f32, Error> {
        self.reader.read_f32::<BigEndian>().map_err(From::from)
    }

    fn parse_f64(&mut self) -> Result<f64, Error> {
        self.reader.read_f64::<BigEndian>().map_err(From::from)
    }

    fn parse_embed_pos(&mut self, byte: u8) -> Result<i8, Error> {
        Ok((byte - INT_POS_FIXED_START) as i8)
    }

    fn parse_embed_neg(&mut self, byte: u8) -> Result<i8, Error> {
        Ok(-((byte - INT_NEG_FIXED_START + 1) as i8))
    }

    fn build_fixed_visitor<'a>(&'a mut self, len: u8) -> FixedVisitor<'a, R> {
        self.peek.take();
        FixedVisitor {
            decoder: self,
            count: 0,
            len: len,
        }
    }

}

impl<R: Read> Deserializer for Decoder<R> {

    type Error = Error;

    fn visit<V: Visitor>(&mut self, mut visitor: V) -> Result<V::Value, Error> {
        match self.peek() {
            Ok(byte) => {
                match byte {
                    b'0'...b'9' => visitor.visit_string(try!(self.parse_string())),
                    STR_FIXED_START...STR_FIXED_END => {
                        visitor.visit_string(try!(self.parse_embed_string(byte)))
                    }
                    I8 => visitor.visit_i8(try!(self.parse_i8())),
                    I16 => visitor.visit_i16(try!(self.parse_i16())),
                    I32 => visitor.visit_i32(try!(self.parse_i32())),
                    I64 => visitor.visit_i64(try!(self.parse_i64())),
                    F32 => visitor.visit_f32(try!(self.parse_f32())),
                    F64 => visitor.visit_f64(try!(self.parse_f64())),
                    INT_POS_FIXED_START...INT_POS_FIXED_END => {
                        visitor.visit_i8(try!(self.parse_embed_pos(byte)))
                    }
                    INT_NEG_FIXED_START...INT_NEG_FIXED_END => {
                        visitor.visit_i8(try!(self.parse_embed_neg(byte)))
                    }
                    TRUE => visitor.visit_bool(true),
                    FALSE => visitor.visit_bool(false),
                    NONE => visitor.visit_none(),
                    LIST => visitor.visit_seq(self),
                    LIST_FIXED_START...LIST_FIXED_END => {
                        let len = byte - LIST_FIXED_START;
                        visitor.visit_seq(self.build_fixed_visitor(len))
                    }
                    DICT => visitor.visit_map(self),
                    DICT_FIXED_START...DICT_FIXED_END => {
                        let len = byte - DICT_FIXED_START;
                        visitor.visit_map(self.build_fixed_visitor(len))
                    }
                    TERM => Err(Error::EndOfStruct),
                    _ => Err(Error::syntax("unexpected byte")),
                }
            }
            Err(err) => Err(err),
        }
    }

}

impl<R: Read> SeqVisitor for Decoder<R> {

    type Error = Error;

    fn visit<T: Deserialize>(&mut self) -> Result<Option<T>, Self::Error> {
        match Deserialize::deserialize(self) {
            Ok(val) => Ok(Some(val)),
            Err(err) => {
                match err {
                    Error::EndOfStruct => Ok(None),
                    _ => Err(err),
                }
            }
        }
    }

    fn end(&mut self) -> Result<(), Self::Error> {
        match try!(self.next()) {
            TERM => Ok(()),
            _ => Err(Error::syntax("expected TERM")),
        }
    }

}

impl<R: Read> MapVisitor for Decoder<R> {

    type Error = Error;

    fn visit_key<K: Deserialize>(&mut self) -> Result<Option<K>, Self::Error> {
        match Deserialize::deserialize(self) {
            Ok(val) => Ok(Some(val)),
            Err(err) => {
                match err {
                    Error::EndOfStruct => Ok(None),
                    _ => Err(err),
                }
            }
        }
    }

    fn visit_value<V: Deserialize>(&mut self) -> Result<V, Self::Error> {
        Deserialize::deserialize(self)
    }

    fn end(&mut self) -> Result<(), Self::Error> {
        match try!(self.next()) {
            TERM => Ok(()),
            _ => Err(Error::syntax("expected TERM")),
        }
    }

}

struct FixedVisitor<'a, R: Read + 'a> {
    decoder: &'a mut Decoder<R>,
    count: u8,
    len: u8,
}

impl<'a, R: Read> SeqVisitor for FixedVisitor<'a, R> {

    type Error = Error;

    fn visit<T: Deserialize>(&mut self) -> Result<Option<T>, Self::Error> {
        if self.count >= self.len {
            Ok(None)
        } else {
            match Deserialize::deserialize(self.decoder) {
                Ok(val) => {
                    self.count += 1;
                    Ok(Some(val))
                },
                Err(err) => Err(err),
            }
        }
    }

    fn end(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

}

impl<'a, R: Read> MapVisitor for FixedVisitor<'a, R> {

    type Error = Error;

    fn visit_key<K: Deserialize>(&mut self) -> Result<Option<K>, Self::Error> {
        if self.count >= self.len {
            Ok(None)
        } else {
            match Deserialize::deserialize(self.decoder) {
                Ok(val) => {
                    self.count += 1;
                    Ok(Some(val))
                },
                Err(err) => Err(err),
            }
        }
    }

    fn visit_value<V: Deserialize>(&mut self) -> Result<V, Self::Error> {
        Deserialize::deserialize(self.decoder)
    }

    fn end(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

}

pub fn decode<T: Deserialize, R: Read>(reader: R) -> Result<T, Error> {
    let mut decoder = Decoder {
        reader: reader,
        peek: None,
    };
    Deserialize::deserialize(&mut decoder)
}

#[cfg(test)]
mod tests {
    use super::decode;
    use super::super::consts::*;
    use std::collections::HashMap;

    #[test]
    fn test_decode_string() {
        // embed
        let s: String = decode(&[131, b'a', b'b', b'c'][..]).unwrap();
        assert_eq!(s, "abc");
        // not embed
        let s: String = decode("8:rustlang".as_bytes()).unwrap();
        assert_eq!(s, "rustlang");
    }

    #[test]
    fn test_decode_int() {
        // embed pos
        let n: i8 = decode(&[43u8][..]).unwrap();
        assert_eq!(n, 43);
        let n: i8 = decode(&[I8, 44][..]).unwrap();
        assert_eq!(n, 44);
        // embed neg
        let n: i8 = decode(&[101u8][..]).unwrap();
        assert_eq!(n, -32);
        let n: i8 = decode(&[I8, 223][..]).unwrap();
        assert_eq!(n, -33);
        // i8
        let n: i8 = decode(&[I8, 100][..]).unwrap();
        assert_eq!(n, 100);
        let n: i8 = decode(&[I8, 156][..]).unwrap();
        assert_eq!(n, -100);
    }

    #[test]
    fn test_decode_seq() {
        // embed
        let a: Vec<i8> = decode(&[195u8, 1, 2, 3][..]).unwrap();
        assert_eq!(a, [1i8, 2, 3]);
        // normal
        let a: Vec<i8> = decode(&[LIST, I8, 1, I8, 2, I8, 3, TERM][..]).unwrap();
        assert_eq!(a, [1i8, 2, 3]);
    }

    #[test]
    fn test_decode_map() {
        let mut b = HashMap::new();
        b.insert(1, 2);
        b.insert(3, 4);
        // embed
        let a: HashMap<i8, i8> = decode(&[104u8, 1, 2, 3, 4][..]).unwrap();
        assert_eq!(a, b);
        // normal
        let a: HashMap<i8, i8> = decode(&[DICT, I8, 1, I8, 2, I8, 3, I8, 4, TERM][..]).unwrap();
        assert_eq!(a, b);
    }
}
