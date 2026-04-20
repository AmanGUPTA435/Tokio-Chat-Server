use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};

pub struct Message {
    pub group_id: String,
    pub username: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

pub struct Join {
    pub group_id: String,
    pub username: String,
    pub timestamp: DateTime<Utc>,
}

pub struct Leave {
    pub group_id: i32,
    pub username: String,
    pub timestamp: DateTime<Utc>,
}

pub struct User {
    pub username: String,
    pub created_at: DateTime<Utc>,
}

pub async fn insert_user(
    tx: &mut Transaction<'_, Postgres>,
    user: User,
) -> Result<(), sqlx::Error> {
    if user.username.len() > 50 {
        return Err(sqlx::Error::Protocol("Username too long".into()));
    }

    sqlx::query!(
        "INSERT INTO users (username, created_at) VALUES ($1, $2)",
        user.username,
        user.created_at.naive_utc()
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn insert_join_event(
    tx: &mut Transaction<'_, Postgres>,
    join: Join,
) -> Result<(), sqlx::Error> {
    if join.username.len() > 50 {
        return Err(sqlx::Error::Protocol("Username too long".into()));
    }
    if join.group_id.parse::<i32>().unwrap_or(0) <= 0 {
        return Err(sqlx::Error::Protocol("Invalid group ID".into()));
    }

    sqlx::query!(
        "INSERT INTO group_requests (user_name, group_id, request_type, status, created_at) VALUES ((SELECT id FROM users WHERE username = $1), $2, 'join', 'complete', $3)",
        join.username,
        join.group_id.parse::<i32>().unwrap_or(0),
        join.timestamp.naive_utc()
    )
    .execute(&mut **tx)
    .await?;

    sqlx::query!(
        "INSERT INTO group_members (user_name, group_id, joined_at) VALUES ((SELECT id FROM users WHERE username = $1), $2, $3)",
        join.username,
        join.group_id.parse::<i32>().unwrap_or(0),
        join.timestamp.naive_utc()
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn insert_leave_event(
    tx: &mut Transaction<'_, Postgres>,
    leave: Leave,
) -> Result<(), sqlx::Error> {
    if leave.username.len() > 50 {
        return Err(sqlx::Error::Protocol("Username too long".into()));
    }
    if leave.group_id <= 0 {
        return Err(sqlx::Error::Protocol("Invalid group ID".into()));
    }

    sqlx::query!(
        "INSERT INTO group_requests (user_name, group_id, request_type, status, created_at) VALUES ((SELECT id FROM users WHERE username = $1), $2, 'leave', 'complete', $3)",
        leave.username,
        leave.group_id,
        leave.timestamp.naive_utc()
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!(
        "DELETE FROM group_members 
        WHERE user_name = $1 AND group_id = $2",
        leave.username,
        leave.group_id
    )
    .execute(&mut **tx)
    .await?;
   
    Ok(())
}

pub async fn insert_message_event(
    tx: &mut Transaction<'_, Postgres>,
    message: Message,
) -> Result<(), sqlx::Error> {
    if message.username.len() > 50 {
        return Err(sqlx::Error::Protocol("Username too long".into()));
    }
    if message.content.len() > 512 {
        return Err(sqlx::Error::Protocol("Message content too long".into()));
    }
    if message.group_id.parse::<i32>().unwrap_or(0) <= 0 {
        return Err(sqlx::Error::Protocol("Invalid group ID".into()));
    }

    sqlx::query!(
        "INSERT INTO group_chats (group_id, user_name, message, timestamp) VALUES ($1, $2, $3, $4)",
        message.group_id.parse::<i32>().unwrap_or(0),
        message.username,
        message.content,
        message.timestamp.naive_utc()
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn is_user_registered(pool: &sqlx::PgPool, user_name: &str) -> bool {
    let user = sqlx::query!(
        "SELECT username FROM users WHERE username = $1",
        user_name
    )
    .fetch_optional(pool)
    .await
    .unwrap_or_else(|_| None);

    user.is_some()
}