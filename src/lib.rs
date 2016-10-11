/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#[macro_use]
extern crate nom;
#[macro_use]
extern crate quick_error;

pub mod collect;
pub mod reader;
pub mod structured;
pub mod syntax;

pub use collect::CertData;
pub use reader::{ParseError, ObjectIter};
pub use structured::{StructureError, TypeError, ValueError,
                     Object, Certificate, Trust, TrustLevel, Usage};

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
