use std::error::Error as StdError;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use serde::{ser::SerializeMap, Serialize};
use shuttle_common::models::error::ApiError;
use stripe::StripeError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("User could not be found")]
    UserNotFound,
    #[error("API key is missing.")]
    KeyMissing,
    #[error("Unauthorized.")]
    Unauthorized,
    #[error("Forbidden.")]
    Forbidden,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
    #[error("Missing checkout session.")]
    MissingCheckoutSession,
    #[error("Incomplete checkout session.")]
    IncompleteCheckoutSession,
    #[error("Interacting with stripe resulted in error: {0}.")]
    Stripe(#[from] StripeError),
    // NOTE: this string is matched in the provisioner when requesting subscription item deletion.
    // If this is changed here it needs to be changed there as well.
    #[error("Missing subscription ID.")]
    MissingSubscriptionId,
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("type", &format!("{:?}", self))?;
        // use the error source if available, if not use display implementation
        map.serialize_entry("msg", &self.source().unwrap_or(self).to_string())?;
        map.end()
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let code = match self {
            Error::Forbidden => StatusCode::FORBIDDEN,
            Error::Unauthorized | Error::KeyMissing => StatusCode::UNAUTHORIZED,
            Error::Database(_) | Error::UserNotFound => StatusCode::NOT_FOUND,
            Error::MissingCheckoutSession
            // NOTE: the provisioner expects a MissingSubscriptionId to return 400.
            | Error::MissingSubscriptionId
            | Error::IncompleteCheckoutSession => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        ApiError {
            message: self.to_string(),
            status_code: code.as_u16(),
        }
        .into_response()
    }
}
