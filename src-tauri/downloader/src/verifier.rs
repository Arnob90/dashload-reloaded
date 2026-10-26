use crate::serialize_io_error::serialize_io_error;
use digest::DynDigest;
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use thiserror::Error;

/// Errors that can occur during file hash verification.
#[derive(Error, Debug, Serialize)]
pub enum VerifyError {
    #[serde(serialize_with = "serialize_io_error")]
    #[error("There has been error in IO")]
    IOError(#[from] std::io::Error),
}

/// Supported hash algorithms for integrity verification.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum HashAlgo {
    Sha256,
    Sha512,
    Md5,
}

/// Internal verifier holding the expected hash and algorithm.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Verifier {
    to_match_hash: String,
    algo: HashAlgo,
}

impl Verifier {
    /// Verifies the file at `filepath` against the expected hash.
    /// Reads the file in 8KB chunks to compute the hash incrementally.
    pub fn verify(&self, filepath: &Path) -> Result<bool, VerifyError> {
        let mut hasher: Box<dyn DynDigest> = match self.algo {
            HashAlgo::Sha256 => Box::new(sha2::Sha256::new()),
            HashAlgo::Sha512 => Box::new(sha2::Sha512::new()),
            HashAlgo::Md5 => Box::new(Md5::new()),
        };
        let mut buffer = [0u8; 8192];
        let mut file = File::open(filepath)?;
        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }
        let res = hasher.finalize_reset();
        let computed_hex = hex::encode(res);
        Ok(computed_hex == self.to_match_hash)
    }
}

/// Optional wrapper around `Verifier`, allowing downloads without checksum verification.
#[derive(Serialize, Deserialize, Clone)]
pub struct VerifierMaybe {
    verifier: Option<Verifier>,
}

impl VerifierMaybe {
    /// Creates a new verifier with the specified algorithm and expected hash.
    pub fn new(algo: HashAlgo, to_match_hash: String) -> Self {
        VerifierMaybe {
            verifier: Some(Verifier {
                algo,
                to_match_hash,
            }),
        }
    }

    /// Creates a `VerifierMaybe` with no verification (always passes).
    pub fn none() -> Self {
        VerifierMaybe { verifier: None }
    }

    /// Verifies the file if a verifier is present; otherwise returns `Ok(true)`.
    pub fn verify(&self, filepath: &Path) -> Result<bool, VerifyError> {
        if let Some(verifier) = &self.verifier {
            verifier.verify(filepath)
        } else {
            Ok(true)
        }
    }

    /// Returns the hash algorithm if a verifier is present.
    pub fn algorithm(&self) -> Option<HashAlgo> {
        self.verifier.as_ref().map(|v| v.algo)
    }

    /// Returns the expected hash string if a verifier is present.
    pub fn expected_hash(&self) -> Option<&str> {
        self.verifier.as_ref().map(|v| v.to_match_hash.as_str())
    }

    /// Returns true if a verifier is configured.
    pub fn is_some(&self) -> bool {
        self.verifier.is_some()
    }
}
