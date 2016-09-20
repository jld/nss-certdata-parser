extern crate nss_certdata_parser;

use std::fs::File;
use std::env::args;
use std::io::BufReader;

use nss_certdata_parser::reader::AttrIter;
use nss_certdata_parser::syntax::Value;

// cf. perl -C0 -ne 's/^([^"]*)#.*/$1/;s/\\x(..)/chr(hex($1))/eg;print if /\S/'
// (Note that the certdata.txt has both hex-escaped and unescaped non-ASCII chars.)

fn main() {
    for path in args().skip(1) {
        println!("BEGINDATA");
        for res_attr in AttrIter::new(BufReader::new(File::open(path).unwrap())) {
            // This might be useful as part of the actual library....
            match res_attr.unwrap() {
                (k, Value::Token(t, v)) => println!("{} {} {}", k, t, v),
                // This isn't quite right -- it `\u{...}` escapes non-ASCII.
                // (Hint: perl -C7 -pe 's/\\u\{(.+?)\}/chr(hex($1))/eg')
                (k, Value::String(v)) => println!("{} UTF8 {:?}", k, v),
                (k, Value::Binary(v)) => {
                    print!("{} MULTILINE_OCTAL", k);
                    for (i, b) in v.into_iter().enumerate() {
                        if i % 16 == 0 {
                            println!("");
                        }
                        print!("\\{:03o}", b);
                    }
                    println!("");
                    println!("END");
                }
            }
        }
    }
}
