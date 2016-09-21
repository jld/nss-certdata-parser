use std::cmp::{Ord, Ordering};
use std::io;
use std::io::Write;

use structured::{Certificate, Trust, Object, Usage};
use structured::TrustLevel::*;

#[allow(dead_code)]
pub mod types {
    pub use structured::TrustLevel;
    pub type Blob = [u8];
    pub type Asn1 = Blob;
    pub struct Certificate<'a> {
        pub label: &'a str,
        pub value: &'a Asn1,
        pub issuer: &'a Asn1,
        pub serial: &'a Asn1,
        pub subject: &'a Asn1,
    }
    pub struct Trust<'a> {
        pub label: &'a str,
        pub issuer: &'a Asn1,
        pub serial: &'a Asn1,
        pub tls_server_trust: TrustLevel,
        pub email_trust: TrustLevel,
        pub code_signing_trust: TrustLevel,
    } 
    pub struct Distrust<'a> {
        pub issuer: &'a Asn1,
        pub serial: &'a Asn1,
        pub label: &'a str,
    }
}

pub fn print_cert<W: Write>(mut out: W, cert: &Certificate) -> io::Result<()> {
    write!(out, concat!("Certificate {{\n",
                        "    label: {label:?},\n",
                        "    value: &{value:?},\n",
                        "    issuer: &{issuer:?}\n",
                        "    serial: &{serial:?}\n",
                        "    subject: &{subject:?}\n",
                        "}}"),
           label = cert.label,
           value = cert.value,
           issuer = cert.issuer,
           serial = cert.serial,
           subject = cert.subject)
}

fn cert_cmp(ca: &Certificate, cb: &Certificate) -> Ordering {
    ca.subject.cmp(&cb.subject)
}
fn trust_cmp_with(t: &Trust, issuer: &[u8], serial: &[u8]) -> Ordering {
    (&t.issuer[..], &t.serial[..]).cmp(&(issuer, serial))
}
fn trust_cmp(ta: &Trust, tb: &Trust) -> Ordering {
    trust_cmp_with(ta, &ta.issuer, &tb.serial)
}

pub struct CertData {
    certs: Box<[Certificate]>,
    trusts: Box<[Trust]>,
}

impl CertData {
    pub fn from_iter<E, I>(iter: I) -> Result<Self, E>
        where I: IntoIterator<Item = Result<Object, E>>
    {
        let mut certbuf = Vec::new();
        let mut trustbuf = Vec::new();
        for thing in iter {
            match try!(thing) {
                Object::Certificate(cert) => certbuf.push(cert),
                Object::Trust(trust) => trustbuf.push(trust),
            }
        }            
        let mut certs = certbuf.into_boxed_slice();
        let mut trusts = trustbuf.into_boxed_slice();
        certs.sort_by(cert_cmp);
        trusts.sort_by(trust_cmp);
        Ok(CertData {
            certs: certs,
            trusts: trusts,
        })
    }

    pub fn certs(&self) -> &[Certificate] {
        &self.certs
    }
    pub fn trusts(&self) -> &[Trust] {
        &self.trusts
    }

    pub fn trust_for(&self, issuer: &[u8], serial: &[u8]) -> Option<&Trust> {
        if let Ok(i) = self.trusts.binary_search_by(|t| trust_cmp_with(t, issuer, serial)) {
            Some(&self.trusts[i])
        } else {
            None
        }
    }

    pub fn trust_for_cert(&self, cert: &Certificate) -> Option<&Trust> {
        self.trust_for(&cert.issuer, &cert.serial)
    }

    pub fn trusted_certs(&self, usage: Usage) -> Vec<&Certificate> {
        self.certs.iter()
            .filter(|cert| {
                self.trust_for_cert(cert)
                    .map_or(MustVerify, |trust| trust.trust_level(usage))
                    == TrustedDelegator
            }).collect()
    }
    pub fn distrusts(&self, usage: Usage) -> Vec<&Trust> {
        self.trusts.iter()
            .filter(|trust| trust.trust_level(usage) == Distrust)
            .collect()
    }
}
