use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};

#[derive(Debug)]
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
    pub password_hash: String
}

pub async fn insert_user(
    tx: &mut Transaction<'_, Postgres>,
    user: User,
) -> Result<(), sqlx::Error> {
    if user.username.len() > 50 {
        return Err(sqlx::Error::Protocol("Username too long".into()));
    }
    if user.password_hash.len() == 0 {
        return Err(sqlx::Error::Protocol("Password not provided".into()));
    }

    sqlx::query!(
        "INSERT INTO users (username, created_at, password_hash) VALUES ($1, $2, $3)",
        user.username,
        user.created_at.naive_utc(),
        user.password_hash
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
    if join.group_id.parse::<i32>().map_err(|_| sqlx::Error::Protocol("Invalid group ID".into()))? <= 0 {
        return Err(sqlx::Error::Protocol("Invalid group ID".into()));
    }

    sqlx::query!(
        "INSERT INTO group_requests (user_name, group_id, request_type, status, created_at) VALUES ($1, $2, 'join', 'complete', $3)",
        join.username,
        join.group_id.parse::<i32>().map_err(|_| sqlx::Error::Protocol("Invalid group ID".into()))?,
        join.timestamp.naive_utc()
    )
    .execute(&mut **tx)
    .await?;

    sqlx::query!(
        "INSERT INTO group_members (user_name, group_id, joined_at) VALUES ($1, $2, $3)
        ON CONFLICT (user_name, group_id) DO NOTHING;",
        join.username,
        join.group_id.parse::<i32>().map_err(|_| sqlx::Error::Protocol("Invalid group ID".into()))?,
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
        "INSERT INTO group_requests (user_name, group_id, request_type, status, created_at) VALUES ($1, $2, 'leave', 'complete', $3)",
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
    if message.group_id.parse::<i32>().map_err(|_| sqlx::Error::Protocol("Invalid group ID".into()))? <= 0 {
        return Err(sqlx::Error::Protocol("Invalid group ID".into()));
    }

    sqlx::query!(
        "INSERT INTO group_chats (group_id, user_name, message, timestamp) VALUES ($1, $2, $3, $4)",
        message.group_id.parse::<i32>().map_err(|_| sqlx::Error::Protocol("Invalid group ID".into()))?,
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

pub async fn chat_history(
    pool: &sqlx::PgPool,
    group_id: i32,
) -> Vec<Message> {
    let mut rows = sqlx::query!(
        r#"
        SELECT gc.user_name, gc.message, gc.timestamp 
        FROM group_chats gc
        WHERE gc.group_id = $1
        ORDER BY gc.timestamp DESC
        LIMIT 20
        "#,
        group_id
    )
    .fetch_all(pool)
    .await
    .unwrap_or_else(|_| vec![]);

    rows.reverse(); // oldest → newest

    rows.into_iter()
        .map(|row| Message {
            group_id: group_id.to_string(),
            username: row.user_name,
            content: row.message,
            timestamp: DateTime::<Utc>::from_naive_utc_and_offset(row.timestamp.unwrap(), Utc),
        })
        .collect()
}

pub async fn get_password_hash(
    pool: &sqlx::PgPool,
    user_name: &str,
) -> Result<Option<String>, sqlx::Error> {
    let row = sqlx::query_scalar!(
        r#"
        SELECT password_hash
        FROM users
        WHERE username = $1
        "#,
        user_name
    )
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotenvy::from_filename;
    use sqlx::{PgPool, postgres::PgPoolOptions};
    use chrono::Utc;

    async fn setup_db() -> PgPool {
        from_filename(".env.test").ok();

        let db_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set");

        PgPoolOptions::new()
            .max_connections(2)
            .connect(&db_url)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_insert_user_and_fetch_hash() {
        let pool = setup_db().await;
        let mut tx = pool.begin().await.unwrap();

        let user = User {
            username: "test_user".into(),
            created_at: Utc::now(),
            password_hash: "hashed_pw".into(),
        };

        insert_user(&mut tx, user).await.unwrap();

        let hash = get_password_hash(&pool, "test_user")
            .await
            .unwrap();

        assert!(hash.is_some());
        assert_eq!(hash.unwrap(), "hashed_pw");

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_insert_user_invalid() {
        let pool = setup_db().await;
        let mut tx = pool.begin().await.unwrap();

        let user = User {
            username: "a".repeat(100),
            created_at: Utc::now(),
            password_hash: "".into(),
        };

        let res = insert_user(&mut tx, user).await;

        assert!(res.is_err());

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_join_and_leave_flow() {
        let pool = setup_db().await;
        let mut tx = pool.begin().await.unwrap();

        let join = Join {
            username: "user1".into(),
            group_id: "1".into(),
            timestamp: Utc::now(),
        };

        insert_join_event(&mut tx, join).await.unwrap();

        let leave = Leave {
            username: "user1".into(),
            group_id: 1,
            timestamp: Utc::now(),
        };

        insert_leave_event(&mut tx, leave).await.unwrap();

        // No panic = success

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_join_invalid_group() {
        let pool = setup_db().await;
        let mut tx = pool.begin().await.unwrap();

        let join = Join {
            username: "user1".into(),
            group_id: "-1".into(),
            timestamp: Utc::now(),
        };

        let res = insert_join_event(&mut tx, join).await;

        assert!(res.is_err());

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_insert_message_and_history() {
        let pool = setup_db().await;
        let mut tx = pool.begin().await.unwrap();

        let msg = Message {
            username: "user1".into(),
            group_id: "1".into(),
            content: "hello world".into(),
            timestamp: Utc::now(),
        };

        insert_message_event(&mut tx, msg).await.unwrap();

        tx.commit().await.unwrap(); // commit so history sees it

        let history = chat_history(&pool, 1).await;

        assert!(!history.is_empty());
        assert_eq!(history[0].content, "hello world");
    }

    #[tokio::test]
    async fn test_message_invalid() {
        let pool = setup_db().await;
        let mut tx = pool.begin().await.unwrap();

        let msg = Message {
            username: "user1".into(),
            group_id: "abc".into(),
            content: "hello".into(),
            timestamp: Utc::now(),
        };

        let res = insert_message_event(&mut tx, msg).await;

        assert!(res.is_err());

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_is_user_registered() {
        let pool = setup_db().await;
        let mut tx = pool.begin().await.unwrap();

        let user = User {
            username: "exists_user".into(),
            created_at: Utc::now(),
            password_hash: "pw".into(),
        };

        insert_user(&mut tx, user).await.unwrap();
        tx.commit().await.unwrap();

        let exists = is_user_registered(&pool, "exists_user").await;
        let not_exists = is_user_registered(&pool, "random_user").await;

        assert!(exists);
        assert!(!not_exists);
    }
}