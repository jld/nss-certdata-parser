use nom::{space, not_line_ending, alphanumeric, ErrorKind};

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

named!(token<Token>,
       map_res!(recognize!(many1!(alt!(alphanumeric | tag!("_")))),
                |bv: &[_]| String::from_utf8(bv.to_owned())));

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
                  map_res!(many0!(alt!(map!(none_of!("\\\""), |b| b as u8) | hex_esc)),
                           String::from_utf8),
                  tag!("\"")));

named!(multiline_octal<Vec<u8> >,
       many0!(preceded!(leading_junk, octal_esc)));

named!(type_and_value<Value>,
       alt!(preceded!(tag!("MULTILINE_OCTAL"),
                      error!(ErrorKind::Alt,
                             chain!(endl ~
                                    bits: multiline_octal ~
                                    endl ~
                                    tag!("END") ~
                                    endl,
                                    || Value::Binary(bits)))) |
            preceded!(tag!("UTF8"),
                      // ASCII7 is also attested but not actually used in certdata.txt
                      error!(ErrorKind::Alt,
                             chain!(space ~
                                    value: quoted_string ~
                                    endl,
                                    || Value::String(value)))) |
            chain!(type_tag: token ~
                   space ~
                   value: token ~
                   endl,
                   || Value::Token(type_tag, value))));

named!(pub attribute<Attr>,
       chain!(leading_junk ~
              key: token ~
              space ~
              value: type_and_value,
              || (key, value)));

named!(pub leading_junk<()>,
       chain!(fold_many0!(endl, (), |(), ()| ()) ~ space?, || ()));

named!(pub begindata<()>,
       chain!(leading_junk ~
              tag!("BEGINDATA") ~
              endl,
              || ()));

pub type Token = String;
pub type Type = Token;
pub type Attr = (Token, Value);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Token(Type, Token),
    String(String),
    Binary(Vec<u8>), // Type is always MULTILINE_OCTAL
}

impl Value {
    pub fn get_type(&self) -> &str {
        match *self {
            Value::Token(ref ttype, _) => ttype,
            Value::String(_) => "UTF8",
            Value::Binary(_) => "MULTILINE_OCTAL",
        }
    }
    pub fn into_type(self) -> String {
        match self {
            Value::Token(ttype, _) => ttype,
            _ => self.get_type().to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{comment, endl, token, quad_digit, octal_digit, hex_digit, octal_esc, hex_esc,
                quoted_string, multiline_octal, type_and_value, attribute, Value};
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
        assert_eq!(token(b"CK_TRUE"), Done(&b""[..], "CK_TRUE".to_owned()));
        assert_eq!(token(b"UTF8"), Done(&b""[..], "UTF8".to_owned()));
        assert_eq!(token(b"CKA_CERT_MD5_HASH"), Done(&b""[..], "CKA_CERT_MD5_HASH".to_owned()));
        assert_eq!(token(b"UTF8 "), Done(&b" "[..], "UTF8".to_owned()));
        assert_eq!(token(b"UTF8\n"), Done(&b"\n"[..], "UTF8".to_owned()));
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

    #[test]
    fn test_quoted_string() {
        assert_eq!(quoted_string(b"\"Stuff\""), Done(&b""[..], "Stuff".to_owned()));
        assert_eq!(quoted_string("\"Stũff\"".as_bytes()), Done(&b""[..], "Stũff".to_owned()));
        assert_eq!(quoted_string(b"\"a\"\"b\""), Done(&b"\"b\""[..], "a".to_owned()));

        assert_eq!(quoted_string(b"\"A\\x42\""), Done(&b""[..], "AB".to_owned()));

        assert_eq!(quoted_string(b"UTF8"), Error(Err::Position(ErrorKind::Tag, &b"UTF8"[..])));

        assert_eq!(quoted_string(b"\"A\\x82\""),
                   Error(Err::Position(ErrorKind::MapRes, &b"A\\x82\""[..])));
        assert_eq!(quoted_string(b"\"A\\xce\""),
                   Error(Err::Position(ErrorKind::MapRes, &b"A\\xce\""[..])));
        assert_eq!(quoted_string(b"\"A\\xce\\xbb\""),
                   Done(&b""[..], "Aλ".to_owned()));

        assert_eq!(quoted_string(b"\"A\\\\B\""),
                   Error(Err::Position(ErrorKind::Tag, &b"\\\\B\""[..])));
        assert_eq!(quoted_string(b"\"A\\\"B\""),
                   Error(Err::Position(ErrorKind::Tag, &b"\\\"B\""[..])));
        assert_eq!(quoted_string(b"\"A\\102\""),
                   Error(Err::Position(ErrorKind::Tag, &b"\\102\""[..])));

        assert_eq!(quoted_string(b"\"AC Ra\\xC3\\xADz\""),
                   Done(&b""[..], "AC Raíz".to_owned()));
        assert_eq!(quoted_string("\"Főtanúsítvány\"".as_bytes()),
                   Done(&b""[..], "Főtanúsítvány".to_owned()));
    }

    #[test]
    fn test_multiline_octal() {
        assert_eq!(multiline_octal(b"\\101"), Done(&b""[..], vec![65]));
        assert_eq!(multiline_octal(b"\\101\\033"), Done(&b""[..], vec![65, 27]));
        assert_eq!(multiline_octal(b"\\101\n\\033"), Done(&b""[..], vec![65, 27]));
        assert_eq!(multiline_octal(b"\\101\\033\n"),
                   Incomplete(Needed::Size("\\101\\033\n".len() + 1)));
        assert_eq!(multiline_octal(b"\n\\101\\033"), Done(&b""[..], vec![65, 27]));
        assert_eq!(multiline_octal(b"\\101\n\n\n\n\n\n\\033"), Done(&b""[..], vec![65, 27]));
        assert_eq!(multiline_octal(b"\\101\r\n\r\n\r\n\\033"), Done(&b""[..], vec![65, 27]));
        assert_eq!(multiline_octal(b"\\101 \\033"), Done(&b""[..], vec![65, 27]));
        assert_eq!(multiline_octal(b"\\101 # Sixty-five \n\t\\033"), Done(&b""[..], vec![65, 27]));
        assert_eq!(multiline_octal(b"\\101\\033\nEND"), Done(&b"\nEND"[..], vec![65, 27]));
        assert_eq!(multiline_octal(b"\\101\\033\n   END"), Done(&b"\n   END"[..], vec![65, 27]));
    }

    #[test]
    fn test_token_value() {
        assert_eq!(type_and_value(b"CK_BBOOL CK_TRUE\n"),
                   Done(&b""[..], Value::Token("CK_BBOOL".to_owned(), "CK_TRUE".to_owned())));
        assert_eq!(type_and_value(b"CK_BBOOL   \t   CK_TRUE\n"),
                   Done(&b""[..], Value::Token("CK_BBOOL".to_owned(), "CK_TRUE".to_owned())));
        assert_eq!(type_and_value(b"CK_BBOOL CK_TRUE\n\n"),
                   Done(&b"\n"[..], Value::Token("CK_BBOOL".to_owned(), "CK_TRUE".to_owned())));
        assert_eq!(type_and_value(b"CK_BBOOL CK_TRUE \n "),
                   Done(&b" "[..], Value::Token("CK_BBOOL".to_owned(), "CK_TRUE".to_owned())));
        assert_eq!(type_and_value(b"CK_BBOOL CK_TRUE # Very true. Wow. \n"),
                   Done(&b""[..], Value::Token("CK_BBOOL".to_owned(), "CK_TRUE".to_owned())));
        assert_eq!(type_and_value(b"CK_BBOOL CK_TRUE"),
                   Incomplete(Needed::Size("CK_BBOOL CK_TRUE".len() + 1)));
        assert!(type_and_value(b"CK_BBOOL\nCK_TRUE\n").is_err());

        let bad_rhs = |ek: ErrorKind| {
            let inner = Box::new(Err::Position(ek, &b"CK_TRUE\n"[..]));
            Error(Err::NodePosition(ErrorKind::Alt, &b" CK_TRUE\n"[..], inner))
        };
        assert_eq!(type_and_value(b"UTF8 CK_TRUE\n"), bad_rhs(ErrorKind::Tag));
        assert_eq!(type_and_value(b"MULTILINE_OCTAL CK_TRUE\n"), bad_rhs(ErrorKind::Char));
    }

    #[test]
    fn test_string_value() {
        assert_eq!(type_and_value(b"UTF8 \"0\"\n"),
                   Done(&b""[..], Value::String("0".to_owned())));
        assert_eq!(type_and_value(b"UTF8 \"Bogus Mozilla Addons\"\n"),
                   Done(&b""[..], Value::String("Bogus Mozilla Addons".to_owned())));

        assert!(type_and_value(b"UTF8\n\"0\"\n").is_err());
        assert!(type_and_value(b"CK_OBJECT_CLASS \"0\"\n").is_err());
        assert!(type_and_value(b"MULTILINE_OCTAL \"0\"\n").is_err());

        assert_eq!(type_and_value(b"UTF8   "), Incomplete(Needed::Size("UTF8   ".len() + 1)));
        assert_eq!(type_and_value(b"UTF8 \""), Incomplete(Needed::Size("UTF8 \"".len() + 1)));
        assert_eq!(type_and_value(b"UTF8 \"x\""), Incomplete(Needed::Size("UTF8 \"x\"".len() + 1)));
    }

    #[test]
    fn test_octal_value() {
        assert_eq!(type_and_value(b"MULTILINE_OCTAL\n\
                                    \\000\\001\\002\n\
                                    \\010\\011\\012\n\
                                    END\n"),
                   Done(&b""[..], Value::Binary(vec![0, 1, 2, 8, 9, 10])))

        // TODO: more cases?
    }

    #[test]
    fn test_attr() {
        assert_eq!(attribute(b"# This is a thing.\n\
                               CKA_CLASS CK_OBJECT_CLASS CKO_CERTIFICATE\n"),
                   Done(&b""[..], ("CKA_CLASS".to_owned(),
                                   Value::Token("CK_OBJECT_CLASS".to_owned(),
                                                "CKO_CERTIFICATE".to_owned()))));

        assert_eq!(attribute(b"CKA_SERIAL_NUMBER MULTILINE_OCTAL\n\
                               \\002\\004\\011\\023\\310\\251\n\
                               END\n"),
                   Done(&b""[..], ("CKA_SERIAL_NUMBER".to_owned(),
                                   Value::Binary(vec![0x02, 4, 0x9, 0x13, 0xc8, 0xa9]))));

        // TODO: more cases?
    }
}
