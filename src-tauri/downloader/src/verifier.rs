use crate::serialize_io_error::serialize_io_error;
use digest::DynDigest;
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use thiserror::Error;
#[derive(Error, Debug, Serialize)]
pub enum VerifyError {
    #[serde(serialize_with = "serialize_io_error")]
    #[error("There has been error in IO")]
    IOError(#[from] std::io::Error),
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum HashAlgo {
    Sha256,
    Sha512,
    Md5,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Verifier {
    to_match_hash: String,
    algo: HashAlgo,
}
impl Verifier {
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
#[derive(Serialize, Deserialize, Clone)]
pub struct VerifierMaybe {
    verifier: Option<Verifier>,
}
impl VerifierMaybe {
    pub fn new(algo: HashAlgo, to_match_hash: String) -> Self {
        VerifierMaybe {
            verifier: Some(Verifier {
                algo,
                to_match_hash,
            }),
        }
    }
    pub fn none() -> Self {
        VerifierMaybe { verifier: None }
    }
    pub fn verify(&self, filepath: &Path) -> Result<bool, VerifyError> {
        if let Some(verifier) = &self.verifier {
            verifier.verify(filepath)
        } else {
            Ok(true)
        }
    }
}
