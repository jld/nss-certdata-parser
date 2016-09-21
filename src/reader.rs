use super::Error;
use syntax::{Token, Value, Attr, attribute, begindata};
use structured::Object;

use nom::{slice_to_offsets, Err, ErrorKind, IResult};
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
    pub buf_left: usize,
    pub what: ErrorKind,
}

fn nom_error_info<'a, E: Clone>(err: &'a Err<&'a [u8], E>) -> (&'a [u8], ErrorKind<E>) {
    match *err {
        Err::Code(_) => unimplemented!(),
        Err::Node(_, ref err_box) => nom_error_info(err_box),
        Err::Position(ref ek, loc) => (loc, ek.clone()),
        Err::NodePosition(_, _, ref err_box) => nom_error_info(err_box),
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
// FIXME: this is fundamentally broken; nom expects the entire input,
// and in some cases an unexpected "end of file" (which in this usage
// is really just the end of an arbitrary buffer) can cause Error
// results rather than Incomplete.  Worse, the reported location for
// that error might *not* be the end of the buffer, depending on how
// the grammar.  This should be replaced by either reading the entire
// file into memory or using a handwritten parser.
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
                let (rest, what) = nom_error_info(&err);
                // This is a hack to try to recognize spurious errors.
                // Might cause false negatives.
                if !rest.contains(&b'\n') {
                    Ok(None)
                } else {
                    let (seen, _) = slice_to_offsets(buf, rest);
                    Err(Error::ParseError(ParseError{
                        byte_offset: off + (seen as Offset),
                        buf_left: rest.len(),
                        what: what,
                    }.into()))
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

// Does this really belong in this module (vs. structured)?  Does it matter?
pub struct ObjectIter<I: BufRead> {
    inner: RawObjectIter<I>,
}

impl<I: BufRead> From<ObjectIter<I>> for RawObjectIter<I> {
    fn from(outer: ObjectIter<I>) -> Self {
        outer.inner
    }
}
impl<I: BufRead> From<RawObjectIter<I>> for ObjectIter<I> {
    fn from(inner: RawObjectIter<I>) -> Self {
        ObjectIter { inner: inner }
    }
}

impl<I: BufRead> ObjectIter<I> {
    pub fn new(src: I) -> Self {
        RawObjectIter::new(src).into()
    }
    pub fn into_inner(self) -> RawObjectIter<I> {
        self.into()
    }
}

impl<I: BufRead> Iterator for ObjectIter<I> {
    type Item = Result<Object, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                None => return None,
                Some(Err(err)) => return Some(Err(err)),
                Some(Ok(obj)) => match Object::from_raw(obj) {
                    Err(err) => return Some(Err(err.into())),
                    Ok(Some(obj)) => return Some(Ok(obj)),
                    Ok(None) => ()
                }
            };
        }
    }
}
