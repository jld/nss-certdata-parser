# NSS `certdata.txt` Parser

This crate parses `certdata.txt` file from [NSS][], which represents
the [Mozilla CA Certificate Store][castore].

[NSS]: https://nss-crypto.org/
[castore]: https://www.mozilla.org/en-US/about/governance/policies/security-group/certs/

## The Data

The `certdata.txt` file describes a collection of PKCS\#11 objects,
which are collections of attributes, which are more or less
key/type/value tuples.  This includes certificates and trust declarations.

A certificate in this file could be a trusted CA, **but** it could
also be certificate that's known to be compromised or fraudulent; use
with caution.

The trust declarations can apply either to certificates in the file
itself or to certificates that might be presented by an entity trying
to prove its identity (e.g., a TLS server); they can indicate either
trust or distrust, possibly with different trust levels for different
uses.

The most important thing about the trust levels is that an explicitly
distrusted certificate (`CKT_NSS_NOT_TRUSTED`) is untrusted *even if
it has an otherwise valid signature* (directly or indirectly) from a
trusted delegator.  In contrast, `CKT_NSS_MUST_VERIFY_TRUST` is more
or less equivalent to not having a trust entry at all.  (These trust
levels were originally a Netscape extension that never made it back
into the upstream standards, and it's not clear that there's any
documentation other than what former Netscape employees happen to
remember, but this seems to be how they're used.)

## Bugs

* Needs documentation.

* The low-level `nom` parser doesn't really need to be public; hiding
  it would allow changing the implementation without (further)
  breaking the API.

* Needs tests for the higher layers of the library, not just the syntax.

* `nom` was not the best choice here, in hindsight.  A hand-written
  parser would probably be simpler overall, wouldn't need delicate
  hacks to adapt it to streaming use, and would be much easier to get
  useful error messages (e.g., with a line number) from.  However, the
  current parser works, and the file format is unlikely to change
  substantially.

