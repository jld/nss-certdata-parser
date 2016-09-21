extern crate nss_certdata_parser;

use std::fs::File;
use std::env::args;
use std::io::{BufReader,stdout};

use nss_certdata_parser::{ObjectIter, CertData, print_cert};

fn main() {
    for path in args().skip(1) {
        let objs = ObjectIter::new(BufReader::new(File::open(path).unwrap()));
        let stuff = CertData::from_iter(objs).unwrap();
        println!("pub const ALL_CERTS: &'static [Certificate<'static>] = [");
        for cert in stuff.certs() {
            print_cert(stdout(), cert).unwrap();
            println!(",");
        }
    }
}
