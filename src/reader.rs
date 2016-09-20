use super::Error;
use syntax::{Token, Value, Attr, attribute, begindata};

use nom::{slice_to_offsets, Err, IResult};
use std::collections::HashMap;
use std::convert::From;
use std::io;
use std::io::BufRead;
use std::mem;

pub type Offset = u64;

#[derive(Debug)]
pub struct ParseError {
    // TODO: more information would be good.  Like the line number.
    // (Or at least one or more ErrorKinds.)
    pub byte_offset: Offset,
}

fn nom_error_loc<'a, E>(err: &'a Err<&'a [u8], E>) -> &'a [u8] {
    match *err {
        Err::Code(_) => unimplemented!(),
        Err::Node(_, ref err_box) => nom_error_loc(err_box),
        Err::Position(_, loc) => loc,
        Err::NodePosition(_, _, ref err_box) => nom_error_loc(err_box),
    }
}

// The code duplication makes me sad, and this ought to be properly tested....
fn bufferize<I, O, E, F>(mut src: I, mut f: F) -> Result<Option<(usize, O)>, E>
    where I: BufRead,
          E: From<io::Error>,
          F: for<'a> FnMut(&'a [u8]) -> Result<Option<(usize, O)>, E>
{
    let mut big_buf = Vec::new();
    // Non-lexical lifetimes would make this code cleaner.
    match {
        let buf = try!(src.fill_buf());
        if buf.is_empty() {
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
            if buf.is_empty() {
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

// Applies a `nom` parser to a `BufRead`, trying to parse from the
// input's own buffer if possible, or else allocating a larger
// temporary buffer if needed.
//
// WARNING: this needs a parser where any proper prefix of a valid
// input is *not* a complete input, and specifically where that
// results in either Incomplete or an Error with innermost location
// (see `nom_error_loc`) at the end of the input.  Otherwise there's
// no way to tell when more input is needed without reading the entire
// file into memory.
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
                let rest = nom_error_loc(&err);
                if rest.is_empty() {
                    // Treat an error at the end of the buffer as if Incomplete.
                    Ok(None)
                } else {
                    let (seen, _) = slice_to_offsets(buf, rest);
                    Err(Error::ParseError(ParseError{ byte_offset: off + (seen as Offset) }.into()))
                }
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
    type Item = Result<Attr, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.had_error {
            return None;
        }
        if self.offset == 0 {
            match apply_nom(begindata, self.offset, &mut self.src) {
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
        match apply_nom(attribute, self.offset, &mut self.src) {
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

pub type RawObject = HashMap<Token, Value>;

pub struct RawObjectIter<I: BufRead> {
    inner: AttrIter<I>,
    acc: RawObject,
    done: bool,
}

impl<I: BufRead> RawObjectIter<I> {
    pub fn new(src: I) -> Self {
        RawObjectIter {
            inner: AttrIter::new(src),
            acc: HashMap::new(),
            done: false,
        }
    }
}

impl<I: BufRead> Iterator for RawObjectIter<I> {
    type Item = Result<RawObject, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        loop {
            assert!(!self.done);
            match self.inner.next() {
                Some(Err(err)) => {
                    self.done = true;
                    return Some(Err(err))
                }
                Some(Ok((key, value))) => {
                    if key == "CKA_CLASS" && !self.acc.is_empty() {
                        let mut next_obj = HashMap::new();
                        next_obj.insert(key, value);
                        return Some(Ok(mem::replace(&mut self.acc, next_obj)))
                    } else {
                        self.acc.insert(key, value);
                    }
                },
                None => {
                    self.done = true;
                    if !self.acc.is_empty() {
                        return Some(Ok(mem::replace(&mut self.acc, HashMap::new())));
                    } else {
                        return None;
                    }
                }
            }
        }
    }
}
