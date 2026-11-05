//! Recovery-secret resolution.
//!
//! Precedence is fixed: `backupPassphraseRef`, then `backupPassphraseEnvVar`,
//! then `MATRIX_RECOVERY_KEY_FILE`. A named source that cannot be read is an
//! error. A missing default recovery-key file means no secret is configured,
//! which is how a fresh account starts. Secret bytes are never formatted.

use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use crate::error::{Result, SecretResolutionError, TrustPolicyNotSatisfiedError};

const MAX_SECRET_BYTES: u64 = 4096;

/// Where a resolved recovery secret came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackupSource {
    /// No secret was configured.
    None,
    /// `backupPassphraseEnvVar` named an environment variable that held a value.
    EnvVar,
    /// `MATRIX_RECOVERY_KEY_FILE` existed and was readable.
    RecoveryKeyFile,
}

impl BackupSource {
    /// Wire value used by `bootstrap-e2ee` (`resolvedBackupSource`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::EnvVar => "env_var",
            Self::RecoveryKeyFile => "recovery_key_file",
        }
    }
}

/// A recovery secret plus the source that produced it.
///
/// `Debug` redacts the secret.
pub struct ResolvedRecovery {
    source: BackupSource,
    secret: Option<String>,
}

impl ResolvedRecovery {
    /// Which source produced this value.
    pub fn source(&self) -> BackupSource {
        self.source
    }

    /// Borrow the secret. `None` when no source was configured.
    #[allow(dead_code)]
    pub fn secret(&self) -> Option<&str> {
        self.secret.as_deref()
    }

    /// Take the secret out of the resolver result.
    #[cfg_attr(not(feature = "e2ee"), allow(dead_code))]
    pub fn into_secret(self) -> Option<String> {
        self.secret
    }
}

impl fmt::Debug for ResolvedRecovery {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResolvedRecovery")
            .field("source", &self.source)
            .field("secret", &self.secret.as_ref().map(|_| "[redacted]"))
            .finish()
    }
}

/// Inputs for one resolution attempt.
///
/// Empty strings are treated as unset. The recovery-key path is the
/// configured file, not a raw secret.
pub struct RecoverySecretRequest<'a> {
    /// Secret-manager reference. No backend is configured in this feature.
    pub backup_passphrase_ref: Option<&'a str>,
    /// Name of an environment variable. The value is the secret.
    pub backup_passphrase_env_var: Option<&'a str>,
    /// Path of `MATRIX_RECOVERY_KEY_FILE`, when configured.
    pub recovery_key_file: Option<&'a Path>,
}

/// Reject any trust policy other than `cross-signed`.
///
/// An omitted policy is `cross-signed`. The supplied text is not copied into
/// the error, so a caller cannot smuggle a secret out through this field.
pub fn require_cross_signed_policy(policy: Option<&str>) -> Result<()> {
    match nonempty(policy) {
        None | Some("cross-signed") => Ok(()),
        Some(_) => {
            Err(TrustPolicyNotSatisfiedError::new("trustPolicy must be \"cross-signed\"").into())
        }
    }
}

/// Resolve a recovery secret in data-model precedence order.
///
/// A `backupPassphraseRef` does not fall through: this build has no
/// secret-manager backend. A named env var does not fall through to the
/// file. A missing file is "no secret". A file that exists but cannot be
/// read, is empty, is larger than 4 KiB, or is not mode `0600` is
/// `SecretResolutionError`.
pub fn resolve_recovery_secret(request: RecoverySecretRequest<'_>) -> Result<ResolvedRecovery> {
    if nonempty(request.backup_passphrase_ref).is_some() {
        return Err(SecretResolutionError::new(
            "backupPassphraseRef requires a secret-manager backend, which is not configured",
        )
        .into());
    }

    if let Some(name) = nonempty(request.backup_passphrase_env_var) {
        let value = read_named_env(name)?;
        return Ok(ResolvedRecovery {
            source: BackupSource::EnvVar,
            secret: Some(value),
        });
    }

    let Some(path) = request.recovery_key_file else {
        return Ok(ResolvedRecovery {
            source: BackupSource::None,
            secret: None,
        });
    };

    if !path.exists() {
        return Ok(ResolvedRecovery {
            source: BackupSource::None,
            secret: None,
        });
    }

    let value = read_secret_file(path)?;
    Ok(ResolvedRecovery {
        source: BackupSource::RecoveryKeyFile,
        secret: Some(value),
    })
}

/// Write a newly minted recovery key. The file is created mode `0600` and
/// is not overwritten. The key is not logged.
#[cfg_attr(not(feature = "e2ee"), allow(dead_code))]
pub fn write_recovery_key_file(path: &Path, key: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                std::fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(parent)?;
            }
            #[cfg(not(unix))]
            {
                std::fs::create_dir_all(parent)?;
            }
        }
    }

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    writeln!(file, "{key}")?;
    Ok(())
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|text| !text.is_empty())
}

fn read_named_env(name: &str) -> Result<String> {
    if name.chars().any(|ch| ch == '\0' || ch == '=') {
        return Err(SecretResolutionError::new(
            "backupPassphraseEnvVar is not an environment variable name",
        )
        .into());
    }
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(value.trim().to_string()),
        _ => Err(SecretResolutionError::new(
            "named recovery environment variable is unset or empty",
        )
        .into()),
    }
}

fn read_secret_file(path: &Path) -> Result<String> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > MAX_SECRET_BYTES {
        return Err(
            SecretResolutionError::new("recovery key file is larger than a recovery key").into(),
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode() & 0o777;
        if mode != 0o600 {
            return Err(SecretResolutionError::new("recovery key file must be mode 0600").into());
        }
    }
    let text = std::fs::read_to_string(path)?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(SecretResolutionError::new("recovery key file is empty").into());
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::os::unix::fs::OpenOptionsExt;

    const PLANTED: &str = "planted-recovery-secret";

    fn write_mode(path: &Path, mode: u32, body: &str) {
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true).mode(mode);
        let mut file = options.open(path).expect("create");
        writeln!(file, "{body}").expect("write");
    }

    #[test]
    fn omitted_policy_and_cross_signed_are_accepted() {
        require_cross_signed_policy(None).unwrap();
        require_cross_signed_policy(Some("")).unwrap();
        require_cross_signed_policy(Some("cross-signed")).unwrap();
    }

    #[test]
    fn other_policies_are_rejected_without_echoing_the_value() {
        let error = require_cross_signed_policy(Some("tofu")).unwrap_err();
        let rendered = error.to_string();
        assert!(rendered.contains("TrustPolicyNotSatisfiedError"));
        assert!(!rendered.contains("tofu"));
        let smuggled = require_cross_signed_policy(Some(PLANTED)).unwrap_err();
        assert!(!smuggled.to_string().contains(PLANTED));
    }

    #[test]
    #[serial]
    fn passphrase_ref_does_not_fall_through_and_does_not_echo_secrets() {
        let dir =
            std::env::temp_dir().join(format!("matrix-mcp-secret-ref-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("recovery-key");
        write_mode(&file, 0o600, PLANTED);
        std::env::set_var("MATRIX_MCP_TEST_RECOVERY_SECRET", PLANTED);

        let error = resolve_recovery_secret(RecoverySecretRequest {
            backup_passphrase_ref: Some("vault://planted-recovery-secret"),
            backup_passphrase_env_var: Some("MATRIX_MCP_TEST_RECOVERY_SECRET"),
            recovery_key_file: Some(&file),
        })
        .unwrap_err();
        let rendered = error.to_string();
        assert!(rendered.contains("SecretResolutionError"));
        assert!(rendered.contains("secret-manager"));
        assert!(!rendered.contains(PLANTED));

        std::env::remove_var("MATRIX_MCP_TEST_RECOVERY_SECRET");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[serial]
    fn env_var_beats_the_file_and_debug_redacts_the_value() {
        let dir =
            std::env::temp_dir().join(format!("matrix-mcp-secret-env-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("recovery-key");
        write_mode(&file, 0o600, "file-secret-should-not-win");
        std::env::set_var("MATRIX_MCP_TEST_RECOVERY_SECRET", PLANTED);

        let resolved = resolve_recovery_secret(RecoverySecretRequest {
            backup_passphrase_ref: None,
            backup_passphrase_env_var: Some("MATRIX_MCP_TEST_RECOVERY_SECRET"),
            recovery_key_file: Some(&file),
        })
        .unwrap();
        assert_eq!(resolved.source(), BackupSource::EnvVar);
        assert_eq!(resolved.secret(), Some(PLANTED));
        assert!(!format!("{resolved:?}").contains(PLANTED));

        std::env::remove_var("MATRIX_MCP_TEST_RECOVERY_SECRET");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[serial]
    fn missing_named_env_var_does_not_fall_through_to_the_file() {
        std::env::remove_var("MATRIX_MCP_TEST_RECOVERY_SECRET");
        let dir = std::env::temp_dir().join(format!(
            "matrix-mcp-secret-missing-env-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("recovery-key");
        write_mode(&file, 0o600, PLANTED);

        let error = resolve_recovery_secret(RecoverySecretRequest {
            backup_passphrase_ref: None,
            backup_passphrase_env_var: Some("MATRIX_MCP_TEST_RECOVERY_SECRET"),
            recovery_key_file: Some(&file),
        })
        .unwrap_err();
        assert!(error.to_string().contains("unset or empty"));
        assert!(!error.to_string().contains(PLANTED));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_means_no_secret() {
        let path = std::env::temp_dir().join(format!(
            "matrix-mcp-secret-absent-{}-{}",
            std::process::id(),
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let resolved = resolve_recovery_secret(RecoverySecretRequest {
            backup_passphrase_ref: None,
            backup_passphrase_env_var: None,
            recovery_key_file: Some(&path),
        })
        .unwrap();
        assert_eq!(resolved.source(), BackupSource::None);
        assert!(resolved.secret().is_none());
    }

    #[test]
    fn loose_or_empty_file_is_an_error_and_contents_stay_out_of_the_message() {
        let dir =
            std::env::temp_dir().join(format!("matrix-mcp-secret-mode-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let loose = dir.join("loose");
        write_mode(&loose, 0o644, PLANTED);
        let error = resolve_recovery_secret(RecoverySecretRequest {
            backup_passphrase_ref: None,
            backup_passphrase_env_var: None,
            recovery_key_file: Some(&loose),
        })
        .unwrap_err();
        assert!(error.to_string().contains("0600"));
        assert!(!error.to_string().contains(PLANTED));

        let empty = dir.join("empty");
        write_mode(&empty, 0o600, "   ");
        let error = resolve_recovery_secret(RecoverySecretRequest {
            backup_passphrase_ref: None,
            backup_passphrase_env_var: None,
            recovery_key_file: Some(&empty),
        })
        .unwrap_err();
        assert!(error.to_string().contains("empty"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn readable_file_is_the_third_source() {
        let dir =
            std::env::temp_dir().join(format!("matrix-mcp-secret-file-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("recovery-key");
        write_mode(&file, 0o600, PLANTED);
        let resolved = resolve_recovery_secret(RecoverySecretRequest {
            backup_passphrase_ref: None,
            backup_passphrase_env_var: Some("  "),
            recovery_key_file: Some(&file),
        })
        .unwrap();
        assert_eq!(resolved.source(), BackupSource::RecoveryKeyFile);
        assert_eq!(resolved.source().as_str(), "recovery_key_file");
        assert_eq!(resolved.secret(), Some(PLANTED));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
