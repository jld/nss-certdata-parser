#[macro_use]
extern crate nom;
#[macro_use]
extern crate quick_error;

pub mod reader;
pub mod structured;
pub mod syntax;

pub use reader::ParseError;
pub use structured::{StructureError, TypeError, ValueError, Object, Certificate, Trust};

use std::io;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        IOError(err: io::Error) {
            from()
            description(err.description())
        }
        ParseError(err: ParseError) {
            from()
            description("parse error")
        }
        StructureError(err: StructureError) {
            from()
            description(err.description())
        }
    }
}
