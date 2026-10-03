//! The Windows Hello gate: an RSA key that Windows Hello keeps in the TPM
//! and uses only after PIN, fingerprint or face.

use crate::{Gate, VaultError};
use windows::Security::Credentials::{
    KeyCredential, KeyCredentialAttestationStatus, KeyCredentialCreationOption,
    KeyCredentialManager, KeyCredentialStatus,
};
use windows::Security::Cryptography::Core::{
    AsymmetricAlgorithmNames, AsymmetricKeyAlgorithmProvider, CryptographicEngine,
    CryptographicPublicKeyBlobType,
};
use windows::Security::Cryptography::CryptographicBuffer;
use windows::Storage::Streams::IBuffer;
use windows::core::{Array, HSTRING};
use zeroize::{Zeroize, Zeroizing};

/// Windows Hello, for the current user.
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowsHello;

impl WindowsHello {
    /// Whether Windows Hello is set up and can hold keys. Asks nothing.
    pub fn is_supported() -> Result<bool, VaultError> {
        KeyCredentialManager::IsSupportedAsync()
            .and_then(|op| op.join())
            .map_err(win)
    }
}

impl Gate for WindowsHello {
    fn kind(&self) -> &'static str {
        "windows-hello"
    }

    fn create(&self, name: &str) -> Result<Vec<u8>, VaultError> {
        if !WindowsHello::is_supported()? {
            return Err(VaultError::Unavailable(
                "Windows Hello isn't set up on this PC: add a PIN in Settings > Accounts > Sign-in options"
                    .into(),
            ));
        }
        let result = KeyCredentialManager::RequestCreateAsync(
            &HSTRING::from(name),
            KeyCredentialCreationOption::FailIfExists,
        )
        .and_then(|op| op.join())
        .map_err(win)?;
        status(result.Status().map_err(win)?)?;
        public_key(&result.Credential().map_err(win)?)
    }

    fn sign(
        &self,
        name: &str,
        challenge: &[u8],
        expected_key: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, VaultError> {
        let credential = open(name)?;
        // A key with the vault's name but another public key isn't the one
        // the vault was sealed with.
        if public_key(&credential)? != expected_key {
            return Err(VaultError::GateMissing);
        }
        let data = CryptographicBuffer::CreateFromByteArray(challenge).map_err(win)?;
        let result = credential
            .RequestSignAsync(&data)
            .and_then(|op| op.join())
            .map_err(win)?;
        status(result.Status().map_err(win)?)?;
        let signature = result.Result().map_err(win)?;
        // RSASSA-PKCS1-v1_5 is deterministic, so this check also guarantees
        // the same signature, and the same vault key, every time.
        let key = AsymmetricKeyAlgorithmProvider::OpenAlgorithm(
            &AsymmetricAlgorithmNames::RsaSignPkcs1Sha256().map_err(win)?,
        )
        .and_then(|p| {
            p.ImportPublicKeyWithBlobType(
                &CryptographicBuffer::CreateFromByteArray(expected_key)?,
                CryptographicPublicKeyBlobType::X509SubjectPublicKeyInfo,
            )
        })
        .map_err(win)?;
        if !CryptographicEngine::VerifySignature(&key, &data, &signature).map_err(win)? {
            return Err(VaultError::Other(
                "Windows Hello's signature doesn't verify as RSA PKCS#1 v1.5; refusing to use it"
                    .into(),
            ));
        }
        Ok(Zeroizing::new(bytes(&signature)?))
    }

    fn delete(&self, name: &str) -> Result<(), VaultError> {
        KeyCredentialManager::DeleteAsync(&HSTRING::from(name))
            .and_then(|op| op.join())
            .map_err(win)
    }

    /// Windows attests a key the TPM holds. "Not supported" means it can't:
    /// a key kept in software, on a PC without a TPM, or a TPM too old to
    /// attest. A temporary failure says nothing either way.
    fn attested(&self, name: &str) -> Result<Option<bool>, VaultError> {
        let result = open(name)?
            .GetAttestationAsync()
            .and_then(|op| op.join())
            .map_err(win)?;
        Ok(match result.Status().map_err(win)? {
            KeyCredentialAttestationStatus::Success => Some(true),
            KeyCredentialAttestationStatus::NotSupported => Some(false),
            _ => None,
        })
    }
}

fn open(name: &str) -> Result<KeyCredential, VaultError> {
    let result = KeyCredentialManager::OpenAsync(&HSTRING::from(name))
        .and_then(|op| op.join())
        .map_err(win)?;
    status(result.Status().map_err(win)?)?;
    result.Credential().map_err(win)
}

fn public_key(credential: &KeyCredential) -> Result<Vec<u8>, VaultError> {
    bytes(
        &credential
            .RetrievePublicKeyWithBlobType(CryptographicPublicKeyBlobType::X509SubjectPublicKeyInfo)
            .map_err(win)?,
    )
}

/// Copies a buffer out, wiping the copy WinRT handed over.
fn bytes(buffer: &IBuffer) -> Result<Vec<u8>, VaultError> {
    let mut array = Array::<u8>::new();
    CryptographicBuffer::CopyToByteArray(buffer, &mut array).map_err(win)?;
    let out = array.to_vec();
    array.zeroize();
    Ok(out)
}

fn status(s: KeyCredentialStatus) -> Result<(), VaultError> {
    match s {
        KeyCredentialStatus::Success => Ok(()),
        KeyCredentialStatus::NotFound => Err(VaultError::GateMissing),
        KeyCredentialStatus::UserCanceled | KeyCredentialStatus::UserPrefersPassword => {
            Err(VaultError::Canceled)
        }
        KeyCredentialStatus::SecurityDeviceLocked => Err(VaultError::Unavailable(
            "the PC's security device is locked after too many attempts; try again later".into(),
        )),
        KeyCredentialStatus::CredentialAlreadyExists => Err(VaultError::Other(
            "a Windows Hello key with this name already exists".into(),
        )),
        other => Err(VaultError::Other(format!(
            "Windows Hello failed (status {})",
            other.0
        ))),
    }
}

fn win(e: windows::core::Error) -> VaultError {
    VaultError::Other(format!("Windows Hello: {}", e.message()))
}
