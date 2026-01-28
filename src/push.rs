use serde::Deserialize;
use sqlx::SqlitePool;
use web_push::{
    ContentEncoding, IsahcWebPushClient, SubscriptionInfo, VapidSignatureBuilder, WebPushClient,
    WebPushError, WebPushMessageBuilder, URL_SAFE_NO_PAD,
};

use crate::{
    auth::new_id,
    state::{AppState, PushConfig},
};

#[derive(Debug, Deserialize)]
pub struct PushSubscriptionInput {
    pub endpoint: String,
    pub keys: PushKeys,
}

#[derive(Debug, Deserialize)]
pub struct PushKeys {
    pub p256dh: String,
    pub auth: String,
}

#[derive(Debug, sqlx::FromRow)]
struct PushSubscriptionRow {
    endpoint: String,
    p256dh: String,
    auth: String,
}

pub async fn store_subscription(
    pool: &SqlitePool,
    appointment_id: &str,
    raw_subscription: &str,
) -> Result<(), sqlx::Error> {
    let subscription: PushSubscriptionInput = match serde_json::from_str(raw_subscription) {
        Ok(value) => value,
        Err(err) => {
            log::warn!("Invalid push subscription payload: {err}");
            return Ok(());
        }
    };

    sqlx::query(
        r#"INSERT INTO push_subscriptions (id, appointment_id, endpoint, p256dh, auth, created_at)
           VALUES (?, ?, ?, ?, ?, ?)
           ON CONFLICT(appointment_id, endpoint) DO UPDATE SET
             p256dh = excluded.p256dh,
             auth = excluded.auth"#,
    )
    .bind(new_id())
    .bind(appointment_id)
    .bind(subscription.endpoint)
    .bind(subscription.keys.p256dh)
    .bind(subscription.keys.auth)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn notify_appointment(
    state: &AppState,
    appointment_id: &str,
    title: &str,
    body: &str,
    url: Option<&str>,
) {
    if !state.push.enabled() {
        return;
    }

    let rows = sqlx::query_as::<_, PushSubscriptionRow>(
        "SELECT endpoint, p256dh, auth FROM push_subscriptions WHERE appointment_id = ?",
    )
    .bind(appointment_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    if rows.is_empty() {
        return;
    }

    let payload = serde_json::json!({
        "title": title,
        "body": body,
        "url": url.unwrap_or("/")
    })
    .to_string();

    for row in rows {
        if let Err(err) = send_push(&state.push, row, &payload).await {
            log::warn!("Push send failed: {err}");
        }
    }
}

async fn send_push(
    config: &PushConfig,
    row: PushSubscriptionRow,
    payload: &str,
) -> Result<(), WebPushError> {
    let subscription = SubscriptionInfo::new(row.endpoint, row.p256dh, row.auth);
    let mut builder = WebPushMessageBuilder::new(&subscription);
    builder.set_payload(ContentEncoding::Aes128Gcm, payload.as_bytes());

    let mut vapid_builder =
        VapidSignatureBuilder::from_base64(&config.private_key, URL_SAFE_NO_PAD, &subscription)?;
    vapid_builder.add_claim("sub", config.subject.clone());

    builder.set_vapid_signature(vapid_builder.build()?);

    let client = IsahcWebPushClient::new()?;
    client.send(builder.build()?).await?;
    Ok(())
}
