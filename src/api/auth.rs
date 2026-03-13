/// Placeholder for PAM authentication + JWT (implemented in Milestone 10).

use jsonwebtoken::{encode, decode, Header, Algorithm, Validation, EncodingKey, DecodingKey};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

/// Issue a JWT token for the given username with the given secret.
pub fn issue_token(username: &str, secret: &[u8], expires_in_secs: i64) -> anyhow::Result<String> {
    let now = Utc::now();
    let exp = (now + Duration::seconds(expires_in_secs)).timestamp() as usize;
    let iat = now.timestamp() as usize;

    let claims = Claims {
        sub: username.to_string(),
        exp,
        iat,
    };

    let token = encode(&Header::default(), &claims, &EncodingKey::from_secret(secret))?;
    Ok(token)
}

/// Validate a JWT token and return the claims.
pub fn validate_token(token: &str, secret: &[u8]) -> anyhow::Result<Claims> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    let data = decode::<Claims>(token, &DecodingKey::from_secret(secret), &validation)?;
    Ok(data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_issue_and_validate() {
        let secret = b"test_secret_key_for_jwt_testing";
        let token = issue_token("testuser", secret, 3600).unwrap();
        assert!(!token.is_empty());

        let claims = validate_token(&token, secret).unwrap();
        assert_eq!(claims.sub, "testuser");
    }

    #[test]
    fn test_jwt_invalid_secret_rejected() {
        let secret = b"correct_secret";
        let token = issue_token("user", secret, 3600).unwrap();
        let result = validate_token(&token, b"wrong_secret");
        assert!(result.is_err());
    }

    #[test]
    fn test_jwt_expired_token_rejected() {
        let secret = b"secret";
        // Issue a token that expired 1 hour ago (well beyond leeway)
        let token = issue_token("user", secret, -3600).unwrap();
        let result = validate_token(&token, secret);
        assert!(result.is_err(), "Expired token should be rejected");
    }
}
