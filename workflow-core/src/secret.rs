use serde::Deserialize;

use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

/// A wrapper for a secret string that can be printed to the console without revealing the secret.
#[derive(Clone, Default)]
pub struct Secret {
    secret: String,
}

impl Secret {
    /// Creates a new secret from the given string.
    ///
    /// # Arguments
    /// * `secret` - The secret string.
    pub fn new(secret: String) -> Self {
        Self { secret }
    }

    /// Returns the secret string without disguising it.
    pub fn secret(&self) -> &str {
        &self.secret
    }
}

impl Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let secret = disguise_secret(&self.secret);
        write!(f, "Secret: {}", secret)
    }
}

impl Display for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let secret = disguise_secret(&self.secret);
        write!(f, "{}", secret)
    }
}

impl FromStr for Secret {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(s.to_string()))
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D>(deserializer: D) -> Result<Secret, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let secret = String::deserialize(deserializer)?;
        Ok(Secret::new(secret))
    }
}

/// Returns a disguised version of the given secret by replacing all but the first and last two
/// characters with asterisks. If the secret is too short, all characters are replaced with
/// asterisks.
///
/// # Arguments
/// * `secret` - The secret to disguise.
pub fn disguise_secret(secret: &str) -> String {
    // The number of non-disguised characters in the beginning and end of the secret.
    const NUM_CLEAN_CHARS: usize = 2;

    // If the secret is too short, don't let any characters be visible.
    let num_clean_chars = if NUM_CLEAN_CHARS * 4 >= secret.len() {
        0
    } else {
        NUM_CLEAN_CHARS
    };

    // Disguise the secret.
    let mut disguised = String::new();
    disguised.push_str(&secret[..num_clean_chars]);
    disguised.extend(std::iter::repeat('*').take(secret.len() - 2 * num_clean_chars));
    disguised.push_str(&secret[secret.len() - num_clean_chars..]);

    disguised
}

#[cfg(test)]
mod test {
    use serde::Deserialize;

    use super::*;

    #[test]
    fn test_secret_display() {
        let secret = Secret::new("password".to_string());
        assert_eq!(format!("{}", secret), "********");
    }

    #[test]
    fn test_secret_debug() {
        let secret = Secret::new("password".to_string());
        assert_eq!(format!("{:?}", secret), "Secret: ********");
    }

    #[test]
    fn test_secret_from_str() {
        let secret: Secret = "password".parse().unwrap();
        assert_eq!(secret.secret(), "password");
    }

    #[test]
    fn test_deserialize_secret() {
        let source = "{\"secret\": \"password\"}";

        #[derive(Deserialize)]
        struct SecretConfig {
            secret: Secret,
        }

        let s = serde_json::from_str::<SecretConfig>(source).unwrap();
        assert_eq!(s.secret.secret(), "password");
    }

    #[test]
    fn test_disguise_secret() {
        let short_secret = "abc";
        assert_eq!(disguise_secret(short_secret), "***");

        let short_secret = "12345678";
        assert_eq!(disguise_secret(short_secret), "********");
    }
}
