//! RSA public-key model and input parsing.
//!
//! Supports hex `n:e`, PKCS#1 `RSAPublicKey`, and X.509 `SubjectPublicKeyInfo`
//! in PEM or raw DER. Parsing is intentionally self-contained (a tiny DER reader)
//! so the whole tool depends on one big-integer stack and the parse path is auditable.

use num_bigint::BigUint;
use num_traits::Num;
use std::fmt;

/// An RSA public key under audit. We only ever hold public material here.
#[derive(Clone, Debug)]
pub struct RsaPublicKey {
    pub n: BigUint,
    pub e: BigUint,
    /// Free-form provenance label (file name, inventory id). Never a secret.
    pub source: String,
}

impl RsaPublicKey {
    pub fn new(n: BigUint, e: BigUint, source: impl Into<String>) -> Self {
        Self { n, e, source: source.into() }
    }

    /// Modulus size in bits.
    pub fn modulus_bits(&self) -> u64 {
        self.n.bits()
    }

    /// Short, non-sensitive fingerprint of the modulus (first/last hex nibbles)
    /// for referring to a key in reports without printing the whole modulus.
    pub fn short_id(&self) -> String {
        let hex = self.n.to_str_radix(16);
        if hex.len() <= 16 {
            hex
        } else {
            format!("{}…{}", &hex[..8], &hex[hex.len() - 8..])
        }
    }
}

impl fmt::Display for RsaPublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RSA-{} key {} (e={})", self.modulus_bits(), self.short_id(), self.e)
    }
}

#[derive(Debug)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "key parse error: {}", self.0)
    }
}
impl std::error::Error for ParseError {}

fn err<T>(m: impl Into<String>) -> Result<T, ParseError> {
    Err(ParseError(m.into()))
}

/// Parse a key from a string: either `n:e` / `n,e` in hex, or a PEM block.
pub fn parse_any(input: &str, source: impl Into<String>) -> Result<RsaPublicKey, ParseError> {
    let source = source.into();
    let trimmed = input.trim();
    if trimmed.contains("-----BEGIN") {
        let der = pem_to_der(trimmed)?;
        return parse_der(&der, source);
    }
    // hex n:e form
    let sep = trimmed.find([':', ',']);
    if let Some(i) = sep {
        let (ns, es) = trimmed.split_at(i);
        let es = &es[1..];
        let n = BigUint::from_str_radix(clean_hex(ns), 16)
            .map_err(|_| ParseError("modulus is not valid hex".into()))?;
        let e = BigUint::from_str_radix(clean_hex(es), 16)
            .map_err(|_| ParseError("exponent is not valid hex".into()))?;
        return Ok(RsaPublicKey::new(n, e, source));
    }
    err("input is neither a PEM block nor a hex `modulus:exponent` pair")
}

fn clean_hex(s: &str) -> &str {
    let s = s.trim();
    s.strip_prefix("0x").unwrap_or(s)
}

/// Parse text holding many keys: PEM blocks and/or one hex `n:e` pair per line.
/// Each record yields its own result; lines that are neither are ignored.
pub fn parse_many(text: &str, source: &str) -> Vec<Result<RsaPublicKey, ParseError>> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut inside = false;
    for line in text.lines() {
        if line.contains("-----BEGIN") {
            inside = true;
            buf.clear();
        }
        if inside {
            buf.push_str(line);
            buf.push('\n');
            if line.contains("-----END") {
                inside = false;
                let record = format!("{source}#{}", out.len());
                out.push(parse_any(&buf, record));
            }
            continue;
        }
        let l = line.trim();
        if l.contains(':') || l.contains(',') {
            let record = format!("{source}#{}", out.len());
            out.push(parse_any(l, record));
        }
    }
    out
}

/// Decode a PEM block to its DER bytes.
pub fn pem_to_der(pem: &str) -> Result<Vec<u8>, ParseError> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let mut body = String::new();
    let mut inside = false;
    for line in pem.lines() {
        let l = line.trim();
        if l.starts_with("-----BEGIN") {
            inside = true;
            continue;
        }
        if l.starts_with("-----END") {
            inside = false;
            continue;
        }
        if inside {
            body.push_str(l);
        }
    }
    if body.is_empty() {
        return err("PEM block contained no base64 body");
    }
    STANDARD
        .decode(body.as_bytes())
        .map_err(|e| ParseError(format!("PEM base64 decode failed: {e}")))
}

// ---- minimal DER reader ------------------------------------------------------

struct Der<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Der<'a> {
    fn new(b: &'a [u8]) -> Self {
        Der { b, pos: 0 }
    }
    fn remaining(&self) -> usize {
        self.b.len() - self.pos
    }
    fn byte(&mut self) -> Result<u8, ParseError> {
        if self.pos >= self.b.len() {
            return err("unexpected end of DER");
        }
        let v = self.b[self.pos];
        self.pos += 1;
        Ok(v)
    }
    /// Read a TLV, returning (tag, value_slice).
    fn tlv(&mut self) -> Result<(u8, &'a [u8]), ParseError> {
        let tag = self.byte()?;
        let first = self.byte()?;
        let len = if first & 0x80 == 0 {
            first as usize
        } else {
            let n = (first & 0x7f) as usize;
            if n == 0 || n > 4 {
                return err("unsupported DER length encoding");
            }
            let mut l = 0usize;
            for _ in 0..n {
                l = (l << 8) | self.byte()? as usize;
            }
            l
        };
        if len > self.remaining() {
            return err("DER length exceeds buffer");
        }
        let start = self.pos;
        self.pos += len;
        Ok((tag, &self.b[start..self.pos]))
    }
}

const TAG_INTEGER: u8 = 0x02;
const TAG_BIT_STRING: u8 = 0x03;
const TAG_SEQUENCE: u8 = 0x30;

fn read_uint(value: &[u8]) -> BigUint {
    // DER INTEGER may carry a leading 0x00 sign byte; from_bytes_be ignores it.
    BigUint::from_bytes_be(value)
}

/// Parse DER that is either PKCS#1 RSAPublicKey or X.509 SubjectPublicKeyInfo.
pub fn parse_der(der: &[u8], source: impl Into<String>) -> Result<RsaPublicKey, ParseError> {
    let source = source.into();
    let mut top = Der::new(der);
    let (tag, seq) = top.tlv()?;
    if tag != TAG_SEQUENCE {
        return err("top-level DER object is not a SEQUENCE");
    }
    let mut inner = Der::new(seq);
    let (first_tag, _) = {
        // peek the first element's tag without consuming the whole structure
        let save = inner.pos;
        let (t, _v) = inner.tlv()?;
        inner.pos = save;
        (t, 0u8)
    };

    if first_tag == TAG_INTEGER {
        // PKCS#1: SEQUENCE { modulus INTEGER, publicExponent INTEGER }
        let (tn, nv) = inner.tlv()?;
        let (te, ev) = inner.tlv()?;
        if tn != TAG_INTEGER || te != TAG_INTEGER {
            return err("PKCS#1 RSAPublicKey fields are not INTEGERs");
        }
        return Ok(RsaPublicKey::new(read_uint(nv), read_uint(ev), source));
    }

    if first_tag == TAG_SEQUENCE {
        // SPKI: SEQUENCE { AlgorithmIdentifier SEQUENCE, subjectPublicKey BIT STRING }
        let (_alg_tag, _alg) = inner.tlv()?; // skip AlgorithmIdentifier
        let (bs_tag, bs) = inner.tlv()?;
        if bs_tag != TAG_BIT_STRING {
            return err("SPKI subjectPublicKey is not a BIT STRING");
        }
        if bs.is_empty() {
            return err("SPKI BIT STRING is empty");
        }
        // first byte of a BIT STRING is the count of unused bits (expect 0)
        let pkcs1 = &bs[1..];
        return parse_der(pkcs1, source);
    }

    err("unrecognised public-key DER structure")
}
