use std::borrow::BorrowMut;

use crate::{
    error::Error,
    user::{AccountName, Admin, Key, ShuttleSubscriptionType, User},
};
use axum::{
    extract::{Path, State},
    Json,
};
use axum_sessions::extractors::{ReadableSession, WritableSession};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use shuttle_common::{
    claims::{AccountTier, Claim},
    models::user,
};
use stripe::CheckoutSession;
use tracing::instrument;

use super::{
    builder::{KeyManagerState, UserManagerState},
    RouterState,
};

#[instrument(skip_all, fields(account.name = %account_name))]
pub(crate) async fn get_user(
    _: Admin,
    State(user_manager): State<UserManagerState>,
    Path(account_name): Path<AccountName>,
) -> Result<Json<user::Response>, Error> {
    let user = user_manager.get_user(account_name).await?;

    Ok(Json(user.into()))
}

#[instrument(skip_all, fields(account.name = %account_name, account.tier = %account_tier))]
pub(crate) async fn post_user(
    _: Admin,
    State(user_manager): State<UserManagerState>,
    Path((account_name, account_tier)): Path<(AccountName, AccountTier)>,
) -> Result<Json<user::Response>, Error> {
    let user = user_manager.create_user(account_name, account_tier).await?;

    Ok(Json(user.into()))
}

#[instrument(skip(user_manager, account_name, account_tier), fields(account.name = %account_name, account.tier = %account_tier))]
pub(crate) async fn update_user_tier(
    _: Admin,
    State(user_manager): State<UserManagerState>,
    Path((account_name, account_tier)): Path<(AccountName, AccountTier)>,
    payload: Option<Json<CheckoutSession>>,
) -> Result<(), Error> {
    if account_tier == AccountTier::Pro {
        match payload {
            Some(Json(checkout_session)) => {
                user_manager
                    .upgrade_to_pro(&account_name, checkout_session)
                    .await?;
            }
            None => return Err(Error::MissingCheckoutSession),
        }
    } else {
        user_manager
            .update_tier(&account_name, account_tier)
            .await?;
    };

    Ok(())
}

#[derive(Deserialize, Debug)]
pub struct SubscriptionPayload {
    pub session: stripe::CheckoutSession,
    pub r#type: ShuttleSubscriptionType,
}

// Add a new non-pro subscription, or increment the quantity.
// TODO: take price ID to determine type of subscription? Or have console send
// type as well?
// What if the user subscribes to several price_ids for the rds product?
// What if we want to support different configs for rds, e.g. different instance sizes?
// If the checkout is completed in the console, why do we need to send the checkout session
// to the backend?
// Do we need to send a checkout session from the console?
#[instrument(skip(user_manager, account_name), fields(account.name = %account_name))]
pub(crate) async fn add_subscription(
    _: Admin,
    State(user_manager): State<UserManagerState>,
    Path(account_name): Path<AccountName>,
    Json(SubscriptionPayload { session, r#type }): Json<SubscriptionPayload>,
) -> Result<(), Error> {
    // fetch the users subscriptions
    let User { subscriptions, .. } = user_manager.get_user(account_name).await?;
    // check if subscription of given type (price id?) already exists
    let existing_subscription = subscriptions.iter().find(|sub| sub.r#type == r#type);

    if let Some(existing_subscription) = existing_subscription {
        // TODO: increase quantity of subscription in state.
    } else {
        // TODO: insert subscription into state.
    }

    Ok(())
}

// Find subscription of given ID and delete it. This should only be done when the subscription
// is fully cancelled.
// TODO: should rds subscription be cancelled immediately or at end of period?
#[instrument(skip(_user_manager, account_name), fields(account.name = %account_name))]
pub(crate) async fn delete_subscription(
    _: Admin,
    State(_user_manager): State<UserManagerState>,
    Path((account_name, subscription_id)): Path<(AccountName, String)>,
) -> Result<(), Error> {
    Ok(())
}

pub(crate) async fn put_user_reset_key(
    session: ReadableSession,
    State(user_manager): State<UserManagerState>,
    key: Option<Key>,
) -> Result<(), Error> {
    let account_name = match session.get::<String>("account_name") {
        Some(account_name) => account_name.into(),
        None => match key {
            Some(key) => user_manager.get_user_by_key(key.into()).await?.name,
            None => return Err(Error::Unauthorized),
        },
    };

    user_manager.reset_key(account_name).await
}

pub(crate) async fn logout(mut session: WritableSession) {
    session.destroy();
}

// Dummy health-check returning 200 if the auth server is up.
pub(crate) async fn health_check() -> Result<(), Error> {
    Ok(())
}

pub(crate) async fn convert_cookie(
    session: ReadableSession,
    State(key_manager): State<KeyManagerState>,
) -> Result<Json<shuttle_common::backends::auth::ConvertResponse>, StatusCode> {
    let account_name = session
        .get::<String>("account_name")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let account_tier = session
        .get::<AccountTier>("account_tier")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let claim = Claim::new(
        account_name,
        account_tier.into(),
        account_tier,
        account_tier,
    );

    let token = claim.into_token(key_manager.private_key())?;

    let response = shuttle_common::backends::auth::ConvertResponse { token };

    Ok(Json(response))
}

/// Convert a valid API-key bearer token to a JWT.
pub(crate) async fn convert_key(
    _: Admin,
    State(RouterState {
        key_manager,
        user_manager,
    }): State<RouterState>,
    key: Key,
) -> Result<Json<shuttle_common::backends::auth::ConvertResponse>, StatusCode> {
    let User {
        name, account_tier, ..
    } = user_manager
        .get_user_by_key(key.into())
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let claim = Claim::new(
        name.to_string(),
        account_tier.into(),
        account_tier,
        account_tier,
    );

    // TODO: check users subscriptions for RDS subs, set rds_quota on claim limits to sub quantity.
    let token = claim.into_token(key_manager.private_key())?;

    let response = shuttle_common::backends::auth::ConvertResponse { token };

    Ok(Json(response))
}

pub(crate) async fn refresh_token() {}

pub(crate) async fn get_public_key(State(key_manager): State<KeyManagerState>) -> Vec<u8> {
    key_manager.public_key().to_vec()
}

#[derive(Deserialize, Serialize)]
pub struct LoginRequest {
    account_name: AccountName,
}
