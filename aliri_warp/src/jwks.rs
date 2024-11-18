//! Filters for verifying JWTs using a JWK

use aliri::{
    jwt::{self, CoreHeaders, HasAlgorithm},
    Jwks, Jwt,
};
use thiserror::Error;
use warp::Filter;

/// An arbitrary error occurred while verifying the JWT
#[derive(Debug, Error)]
#[error("error verifying jwt")]
pub enum VerifyError {
    /// JWT verification error
    JwtVerifyError(#[from] aliri::error::JwtVerifyError),

    /// Unable to find an appropriate JWK in the set
    #[error("no JWK found to verify this token")]
    NoValidKeyFound,
}

impl warp::reject::Reject for VerifyError {}

async fn check_jwt<C: for<'de> serde::Deserialize<'de> + jwt::CoreClaims>(
    jwt: Jwt,
    jwks: &Jwks,
    validator: &jwt::CoreValidator,
) -> Result<C, VerifyError> {
    let decomposed: jwt::Decomposed = jwt.decompose()?;
    let jwk = jwks
        .get_key_by_opt(decomposed.kid(), decomposed.alg())
        .ok_or(VerifyError::NoValidKeyFound)?;
    let v: jwt::Validated<C> = decomposed.verify(jwk, validator)?;
    let (_, c) = v.extract();
    Ok(c)
}

/// Validates a JWT against an approved JWKS
pub fn no_claims<F, K, V>(
    jwt: F,
    jwks: K,
    validator: V,
) -> impl Filter<Extract = (), Error = warp::reject::Rejection> + Clone
where
    F: Filter<Extract = (Jwt,), Error = warp::reject::Rejection> + Clone,
    K: AsRef<Jwks> + Clone + Send + Sync + 'static,
    V: AsRef<jwt::CoreValidator> + Clone + Send + Sync + 'static,
{
    self::jwks(jwt, jwks, validator)
        .map(|_: jwt::BasicClaims| ())
        .untuple_one()
}

/// Validates a JWT against an approved JWKS, returning the payload claims if valid
pub fn jwks<C, F, K, V>(
    jwt: F,
    jwks: K,
    validator: V,
) -> impl Filter<Extract = (C,), Error = warp::reject::Rejection> + Clone
where
    C: for<'de> serde::Deserialize<'de> + jwt::CoreClaims,
    F: Filter<Extract = (Jwt,), Error = warp::reject::Rejection> + Clone,
    K: AsRef<Jwks> + Clone + Send + Sync + 'static,
    V: AsRef<jwt::CoreValidator> + Clone + Send + Sync + 'static,
{
    jwt.and_then(move |jwt: Jwt| {
        let jwks = jwks.clone();
        let validator = validator.clone();
        async move {
            check_jwt(jwt, jwks.as_ref(), validator.as_ref())
                .await
                .map_err(warp::reject::custom)
        }
    })
}
