extern crate nss_certdata_parser;

use std::fs::File;
use std::env::args;
use std::io::BufReader;

use nss_certdata_parser::{ObjectIter, Usage};
use nss_certdata_parser::embed::CertData;

fn main() {
    for path in args().skip(1) {
        let objs = ObjectIter::new(BufReader::new(File::open(path).unwrap()));
        let stuff = CertData::from_iter(objs).unwrap();
        println!("pub const ALL_CERTS: &'static [Certificate<'static>] = &{:#?};",
                 stuff.certs());
        println!("pub const TLS_SERVER_TRUST_ROOTS: &'static [Certificate<'static>] = &{:#?};",
                 stuff.trusted_certs(Usage::TlsServer));
        println!("pub const TLS_SERVER_DISTRUSTS: &'static [Trust<'static>] = &{:#?};",
                 stuff.distrusts(Usage::TlsServer));
    }
}
