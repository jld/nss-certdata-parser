use std::string::FromUtf8Error;

use nom::{space, not_line_ending, alphanumeric};

fn to_owned_string(bv: &[u8]) -> Result<String, FromUtf8Error> {
    String::from_utf8(bv.to_owned())
}

named!(comment<()>,
       chain!(tag!("#") ~
              not_line_ending,
              || ()));

named!(endl<()>,
       chain!(space? ~
              comment? ~
              char!('\r')? ~
              char!('\n'),
              || ()));

named!(token,
       recognize!(many1!(alt!(alphanumeric | tag!("_")))));

named!(hex_digit<u8>,
       map!(one_of!(b"0123456789abcdefABCDEF"), |b| match b {
           '0' ... '9' => b as u8 - b'0',
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
    use super::{comment, endl, token, quad_digit, octal_digit, hex_digit, octal_esc, hex_esc};
    use nom::IResult::*;
    use nom::{Needed, ErrorKind, Err};

    #[test]
    fn test_comment() {
        assert_eq!(comment(b"# thing"), Done(&b""[..], ()));
        assert_eq!(comment(b"#thing"), Done(&b""[..], ()));
        assert_eq!(comment("# ýŷỳ".as_bytes()), Done(&b""[..], ()));
        assert_eq!(comment(b"#"), Done(&b""[..], ()));
        assert_eq!(comment(b""), Incomplete(Needed::Size(1)));
        assert_eq!(comment(b"# thing\n stuff"), Done(&b"\n stuff"[..], ()));
        assert_eq!(comment(b"bees"), Error(Err::Position(ErrorKind::Tag, &b"bees"[..])));
        assert_eq!(comment(b" #bees"), Error(Err::Position(ErrorKind::Tag, &b" #bees"[..])));
    }

    #[test]
    fn test_endl() {
        assert_eq!(endl(b"\n"), Done(&b""[..], ()));
        assert_eq!(endl(b"\r\n"), Done(&b""[..], ()));
        assert_eq!(endl(b"\n\n"), Done(&b"\n"[..], ()));
        assert_eq!(endl(b"\n "), Done(&b" "[..], ()));
        assert_eq!(endl(b" \n"), Done(&b""[..], ()));
        assert_eq!(endl(b"    \n"), Done(&b""[..], ()));
        assert_eq!(endl(b"\t\n"), Done(&b""[..], ()));
        assert_eq!(endl(b"# bees \n"), Done(&b""[..], ()));
        assert_eq!(endl(b"   # bees \n"), Done(&b""[..], ()));
        assert_eq!(endl(b"    bees \n"), Error(Err::Position(ErrorKind::Char, &b"bees \n"[..])));
        assert_eq!(endl(b"# bees"), Incomplete(Needed::Size("# bees".len() + 1)));
    } 

    #[test]
    fn test_token() {
        assert_eq!(token(b"CK_TRUE"), Done(&b""[..], &b"CK_TRUE"[..]));
        assert_eq!(token(b"UTF8"), Done(&b""[..], &b"UTF8"[..]));
        assert_eq!(token(b"CKA_CERT_MD5_HASH"), Done(&b""[..], &b"CKA_CERT_MD5_HASH"[..]));
        assert_eq!(token(b"UTF8 "), Done(&b" "[..], &b"UTF8"[..]));
        assert_eq!(token(b"UTF8\n"), Done(&b"\n"[..], &b"UTF8"[..]));
        assert_eq!(token(b" UTF8"), Error(Err::Position(ErrorKind::Many1, &b" UTF8"[..])));
        assert_eq!(token(b"\"UTF8\""), Error(Err::Position(ErrorKind::Many1, &b"\"UTF8\""[..])));
        assert_eq!(token(b"\\x41"), Error(Err::Position(ErrorKind::Many1, &b"\\x41"[..])));
        assert_eq!(token(b"\\101"), Error(Err::Position(ErrorKind::Many1, &b"\\101"[..])));
        assert_eq!(token(b""), Incomplete(Needed::Size(1)))
    }

    #[test]
    fn test_digits() {
        assert_eq!(quad_digit(b"0"), Done(&b""[..], 0));
        assert_eq!(octal_digit(b"0"), Done(&b""[..], 0));
        assert_eq!(hex_digit(b"0"), Done(&b""[..], 0));

        assert_eq!(quad_digit(b"00"), Done(&b"0"[..], 0));
        assert_eq!(octal_digit(b"00"), Done(&b"0"[..], 0));
        assert_eq!(hex_digit(b"00"), Done(&b"0"[..], 0));

        assert_eq!(quad_digit(b"32"), Done(&b"2"[..], 3));
        assert_eq!(octal_digit(b"76"), Done(&b"6"[..], 7));
        assert_eq!(hex_digit(b"98"), Done(&b"8"[..], 9));

        assert_eq!(quad_digit(b"4"), Error(Err::Position(ErrorKind::OneOf, &b"4"[..])));
        assert_eq!(octal_digit(b"8"), Error(Err::Position(ErrorKind::OneOf, &b"8"[..])));
        assert_eq!(hex_digit(b"g"), Error(Err::Position(ErrorKind::OneOf, &b"g"[..])));
        assert_eq!(hex_digit(b"G"), Error(Err::Position(ErrorKind::OneOf, &b"G"[..])));
        assert_eq!(hex_digit(b":"), Error(Err::Position(ErrorKind::OneOf, &b":"[..])));
        assert_eq!(hex_digit(b"@"), Error(Err::Position(ErrorKind::OneOf, &b"@"[..])));

        assert_eq!(hex_digit(b"a"), Done(&b""[..], 10));
        assert_eq!(hex_digit(b"A"), Done(&b""[..], 10));
        assert_eq!(hex_digit(b"f"), Done(&b""[..], 15));
        assert_eq!(hex_digit(b"F"), Done(&b""[..], 15));
    }

    #[test]
    fn test_octal_esc() {
        assert_eq!(octal_esc(b"\\000"), Done(&b""[..], 0o000));
        assert_eq!(octal_esc(b"\\007"), Done(&b""[..], 0o007));
        assert_eq!(octal_esc(b"\\077"), Done(&b""[..], 0o077));
        assert_eq!(octal_esc(b"\\377"), Done(&b""[..], 0o377));

        assert_eq!(octal_esc(b"\\"), Incomplete(Needed::Size(2)));
        assert_eq!(octal_esc(b"\\0"), Incomplete(Needed::Size(3)));
        assert_eq!(octal_esc(b"\\00"), Incomplete(Needed::Size(4)));
        assert_eq!(octal_esc(b"\\0000"), Done(&b"0"[..], 0));
        assert_eq!(octal_esc(b"\\3765"), Done(&b"5"[..], 0o376));

        assert_eq!(octal_esc(b"\\080"), Error(Err::Position(ErrorKind::OneOf, &b"80"[..])));
        assert_eq!(octal_esc(b"\\400"), Error(Err::Position(ErrorKind::OneOf, &b"400"[..])));

        assert_eq!(octal_esc(b"\\x00"), Error(Err::Position(ErrorKind::OneOf, &b"x00"[..])));
        assert_eq!(octal_esc(b"A"), Error(Err::Position(ErrorKind::Tag, &b"A"[..])));
        assert_eq!(octal_esc(b" \\000"), Error(Err::Position(ErrorKind::Tag, &b" \\000"[..])));
    }

    #[test]
    fn test_hex_esc() {
        assert_eq!(hex_esc(b"\\x00"), Done(&b""[..], 0x00));
        assert_eq!(hex_esc(b"\\x0f"), Done(&b""[..], 0x0f));
        assert_eq!(hex_esc(b"\\xf0"), Done(&b""[..], 0xf0));

        assert_eq!(hex_esc(b"\\"), Incomplete(Needed::Size(2)));
        assert_eq!(hex_esc(b"\\x"), Incomplete(Needed::Size(3)));
        assert_eq!(hex_esc(b"\\x0"), Incomplete(Needed::Size(4)));
        assert_eq!(hex_esc(b"\\x000"), Done(&b"0"[..], 0x00));
        assert_eq!(hex_esc(b"\\xba9"), Done(&b"9"[..], 0xba));

        assert_eq!(hex_esc(b"0x41"), Error(Err::Position(ErrorKind::Tag, &b"0x41"[..])));
        assert_eq!(hex_esc(b"\\000"), Error(Err::Position(ErrorKind::Tag, &b"\\000"[..])));
        assert_eq!(hex_esc(b"\\x0g"), Error(Err::Position(ErrorKind::OneOf, &b"g"[..])));
    }
}
