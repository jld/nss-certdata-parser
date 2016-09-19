use syntax;

use nom::{slice_to_offsets, Err, IResult};
use std::convert::From;
use std::io;
use std::io::BufRead;

pub type Offset = u64;

#[derive(Debug)]
pub struct ParseError {
    pub byte_offset: Offset,
}

fn nom_error_loc<'a, E>(err: &'a Err<&'a [u8], E>) -> Option<&'a [u8]> {
    match *err {
        Err::Code(_) => None,
        Err::Node(_, ref err_box) => nom_error_loc(err_box),
        Err::Position(_, loc) => Some(loc),
        Err::NodePosition(_, _, ref err_box) => nom_error_loc(err_box),
    }
}

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        ParseError(err: ParseError) {
            from()
            description("parse error")
        }
        IOError(err: io::Error) {
            from()
            description(err.description())
        }
    }
}

// The code duplication makes me sad, and this *really* need to be properly tested...
fn bufferize<I, O, E, F>(mut src: I, mut f: F) -> Result<Option<(usize, O)>, E>
    where I: BufRead,
          E: From<io::Error>,
          F: for<'a> FnMut(&'a [u8]) -> Result<Option<(usize, O)>, E>
{
    let mut big_buf = Vec::new();
    // Non-lexical lifetimes would make this code cleaner.
    match {
        let buf = try!(src.fill_buf());
        if buf.len() == 0 {
            return Ok(None);
        }
        match try!(f(buf)) {
            None => { big_buf.extend_from_slice(buf); None },
            some @ Some(_) => some
        }
    } {
        Some((used, res)) => {
            src.consume(used);
            return Ok(Some((used, res)));
        },
        None => {
            src.consume(big_buf.len());
        }
    };
    loop {
        let old_len = big_buf.len();
        big_buf.extend_from_slice({
            let buf = try!(src.fill_buf());
            if buf.len() == 0 {
                return Ok(None);
            } 
            buf
        });
        match try!(f(&big_buf)) {
            Some((used, res)) => {
                src.consume(used - old_len);
                return Ok(Some((used, res)));
            },
            None => src.consume(big_buf.len() - old_len),
        }
    }
}

fn apply_nom<I, O, P>(mut parser: P, off: Offset, src: I)
                   -> Result<Option<(Offset, O)>, Error>
    where I: BufRead,
          P: for<'a> FnMut(&'a [u8]) -> IResult<&'a [u8], O>
{
    if let Some((used, res)) = try!(bufferize(src, |buf| {
        match parser(buf) {
            IResult::Done(rest, res) => {
                let (used, _) = slice_to_offsets(buf, rest);
                Ok(Some((used, res)))
            }
            IResult::Error(err) => {
                let rest = nom_error_loc(&err).expect("expected location for parse error");
                let (seen, _) = slice_to_offsets(buf, rest);
                Err(Error::ParseError(ParseError{ byte_offset: off + (seen as Offset) }.into()))
            }
            IResult::Incomplete(_) => {
                Ok(None)
            }
        }
    })) {
        Ok(Some((off + (used as Offset), res)))
    } else {
        Ok(None)
    }
}

pub struct AttrIter<I: BufRead> {
    src: I,
    offset: Offset,
    had_error: bool,
}

impl<I: BufRead> AttrIter<I> {
    pub fn new(src: I) -> Self {
        AttrIter {
            src: src,
            offset: 0,
            had_error: false
        }
    }
}
impl<I: BufRead> Iterator for AttrIter<I> {
    type Item = Result<syntax::Attr, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.had_error {
            return None;
        }
        if self.offset == 0 {
            match apply_nom(syntax::begindata, self.offset, &mut self.src) {
                Err(err) => {
                    self.had_error = true;
                    return Some(Err(err));
                }
                Ok(None) => {
                    return None;
                }
                Ok(Some((offset, ()))) => {
                    assert!(offset != 0);
                    self.offset = offset;
                }
            }
        }
        match apply_nom(syntax::attribute, self.offset, &mut self.src) {
            Err(err) => {
                self.had_error = true;
                Some(Err(err))
            }
            Ok(None) => None,
            Ok(Some((offset, attr))) => {
                assert!(offset > self.offset);
                self.offset = offset;
                Some(Ok(attr))
            }
        }
    }
}
