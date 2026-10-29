//! Optional bearer token for `/api/*`, enabled by setting `API_TOKEN`.

use std::{fmt, str::FromStr, sync::Arc};

use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::{IntoResponse, Response},
};

use super::error::ApiError;

/// The configured token. Its `Debug` output never shows the value.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiToken(Arc<str>);

impl fmt::Debug for ApiToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiToken(***)")
    }
}

impl FromStr for ApiToken {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim();
        if value.is_empty() {
            return Err("the token must not be empty".to_owned());
        }
        if value.contains(char::is_whitespace) {
            return Err("the token must not contain spaces".to_owned());
        }
        Ok(Self(value.into()))
    }
}

impl ApiToken {
    /// Compares in constant time for tokens of the same length, so response times do not reveal
    /// how much of a guess was right.
    fn matches(&self, candidate: &str) -> bool {
        let (expected, candidate) = (self.0.as_bytes(), candidate.as_bytes());
        expected.len() == candidate.len()
            && expected
                .iter()
                .zip(candidate)
                .fold(0u8, |diff, (a, b)| diff | (a ^ b))
                == 0
    }
}

/// Middleware: lets the request through only with `Authorization: Bearer <token>`.
pub async fn require_token(
    State(token): State<ApiToken>,
    request: Request,
    next: Next,
) -> Response {
    let authorized = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split_once(' '))
        .is_some_and(|(scheme, candidate)| {
            scheme.eq_ignore_ascii_case("bearer") && token.matches(candidate.trim())
        });

    if authorized {
        next.run(request).await
    } else {
        ApiError::Unauthorized.into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tokens() {
        assert!("".parse::<ApiToken>().is_err());
        assert!("  ".parse::<ApiToken>().is_err());
        assert!("two words".parse::<ApiToken>().is_err());
        assert_eq!(
            " secret ".parse::<ApiToken>().unwrap(),
            "secret".parse().unwrap()
        );
    }

    #[test]
    fn matches_only_the_exact_token() {
        let token: ApiToken = "s3cret".parse().unwrap();
        assert!(token.matches("s3cret"));
        assert!(!token.matches("s3creT"));
        assert!(!token.matches("s3cre"));
        assert!(!token.matches("s3crets"));
        assert!(!token.matches(""));
    }

    #[test]
    fn debug_hides_the_value() {
        let token: ApiToken = "s3cret".parse().unwrap();
        assert_eq!(format!("{token:?}"), "ApiToken(***)");
    }
}
