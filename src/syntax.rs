use std::string::FromUtf8Error;

use nom::{space, line_ending, not_line_ending, alphanumeric};

fn to_owned_string(bv: &[u8]) -> Result<String, FromUtf8Error> {
    String::from_utf8(bv.to_owned())
}

named!(comment<()>,
       chain!(tag!("#") ~
              not_line_ending?,
              || ()));

named!(endl<()>,
       chain!(space ~
              comment? ~
              line_ending,
              || ()));

named!(token,
       recognize!(many1!(alt!(alphanumeric | tag!("_")))));

named!(hex_digit<u8>,
       map!(one_of!(b"0123456789abcdefABCDEF"), |b| match b {
           '0' ... '9' => b as u8 - b'\0',
           'a' ... 'f' => b as u8 - b'a' + 10,
           'A' ... 'F' => b as u8 - b'A' + 10,
           _ => unreachable!()
       }));
named!(octal_digit<u8>, map!(one_of!("01234567"), |b| b as u8 - b'0'));
named!(quad_digit<u8>, map!(one_of!("0123"), |b| b as u8 - b'0'));

named!(octal_esc<u8>,
       chain!(tag!("\\") ~
              a: quad_digit ~
              b: octal_digit ~
              c: octal_digit,
              || { a << 6 | b << 3 | c }));
       
named!(hex_esc<u8>,
       chain!(tag!("\\x") ~
              a: hex_digit ~
              b: hex_digit,
              || { a << 4 | b }));

named!(quoted_string<String>,
       delimited!(tag!("\""),
                  map_res!(fold_many0!(alt!(map!(none_of!("\\\""), |b| b as u8) | hex_esc),
                                       Vec::new(), |mut a: Vec<_>, b| { a.push(b); a }),
                           String::from_utf8),
                  tag!("\"")));
                  
named!(multiline_octal<Vec<u8> >,
       fold_many0!(chain!(endl? ~ space? ~ o: octal_esc, || o),
                   Vec::new(), |mut a: Vec<_>, b| { a.push(b); a }));

named!(type_and_value<Value>,
       alt!(chain!(tag!("MULTILINE_OCTAL") ~
                   endl ~
                   bits: multiline_octal ~
                   tag!("END") ~
                   endl, 
                   || Value::Binary(bits))
            |
            chain!(type_tag: map_res!(alt!(tag!("ASCII") | tag!("UTF8")), to_owned_string) ~
                   space ~
                   value: quoted_string ~
                   endl,
                   || Value::String(type_tag, value))
            |
            chain!(type_tag: map_res!(token, to_owned_string) ~
                   space ~
                   value: map_res!(token, to_owned_string) ~
                   endl,
                   || Value::Token(type_tag, value))));

named!(key_value<Attr>,
       chain!(many0!(endl) ~
              space? ~
              key: map_res!(token, to_owned_string) ~
              space? ~
              value: type_and_value,
              || (key, value)));

named!(pub begindata<()>,
       chain!(many0!(endl) ~
              space? ~
              tag!("BEGINDATA") ~
              endl,
              || ()));

named!(pub next_attr<Option<Attr> >,
       chain!(many0!(endl) ~
              space? ~
              kv: key_value?,
              || kv));

pub type Token = String;
pub type Type = Token;
pub type Attr = (Token, Value);

#[derive(Clone, Debug)]
pub enum Value {
    Token(Type, Token),
    String(Type, String),
    Binary(Vec<u8>), // Type is always MULTILINE_OCTAL
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
