use std::{env, net::SocketAddr, sync::Arc, time::Instant};
use tokio::{io::{AsyncBufReadExt, AsyncWriteExt}, net::TcpStream};
use dashmap::DashMap;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::{io::BufReader, net::TcpListener, sync::broadcast};
use tracing::{info, error};
use uuid::Uuid;
use chat_stream_tokio::db;
use chat_stream_tokio::methods::Command;

type GroupId = String;


#[derive(Debug)]
pub enum AppError {
    Db(sqlx::Error),
    Io(std::io::Error),
    Hash(bcrypt::BcryptError),
    Auth(String),
    InvalidInput(String),
    Internal(String),
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::Db(e)
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e)
    }
}

impl From<bcrypt::BcryptError> for AppError {
    fn from(e: bcrypt::BcryptError) -> Self {
        AppError::Hash(e)
    }
}

#[derive(Debug, Clone)]
enum Event {
    Message { user: String, msg: String },
    Join { user: String },
    Leave { user: String },
}

#[derive(Debug, Clone)]
pub struct AuthImpl {
    pub db: PgPool,
    pub groups: DashMap<GroupId, broadcast::Sender<(Event, std::net::SocketAddr)>>,
    pub sessions: DashMap<String, Session>,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub username: String,
    pub created_at: Instant,
}

#[derive(Debug, Clone)]
pub struct Group {
    pub group_id: GroupId,
    pub joined_at: Instant
}

async fn handle_connection(
    socket: TcpStream,
    server: Arc<AuthImpl>,
    addr: SocketAddr
) -> Result<(), AppError> {
    let (reader, mut writer) = socket.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    reader.read_line(&mut line).await?;

    let cmd: Command = match serde_json::from_str(&line) {
        Ok(c) => c,
        Err(_) => {
            writer.write_all(b"Invalid command\n").await?;
            return Ok(());
        }
    };

    let result = match cmd {
        Command::Register { username, password } =>
            server.handle_register(&mut writer, username, password).await,

        Command::Join { group_id, session_id } =>
            server.handle_join(reader, &mut writer, addr, group_id, session_id).await,

        Command::Login { username, password } =>
            server.handle_login(&mut writer, username, password).await,
        
        Command::Logout { session_id } => {
            if server.sessions.remove(&session_id).is_some() {
                writer.write_all(b"Logged out\n").await?;
            } else {
                return Err(AppError::Auth("Invalid session".into()));
            }
            writer.write_all(b"Logged out\n").await?;
            Ok(())
        }
    };

    if let Err(e) = result {
        let msg = match &e {
            AppError::Auth(m) | AppError::InvalidInput(m) => m.clone(),
            _ => {
                error!("Internal error: {:?}", e);
                "Internal server error".to_string()
            }
        };
        writer.write_all(format!("ERROR: {}\n", msg).as_bytes()).await?;
    }

    Ok(())
}

impl AuthImpl {
    pub async fn handle_register(
        &self,
        writer: &mut tokio::net::tcp::OwnedWriteHalf,
        username: String,
        password: String
    ) -> Result<(), AppError> {
        let username = username.trim().to_string();
        let password = password.trim().to_string();
        let password_hash = bcrypt::hash(password, 10)?;
        let mut tx = self.db.begin().await?;

        let user = db::User {
            username: username.clone(),
            created_at: chrono::Utc::now(),
            password_hash: password_hash
        };

        match db::insert_user(&mut tx, user).await {
            Ok(_) => {
                tx.commit().await?;
                writer.write_all(b"User registered\n").await?;
            }
            Err(e) => {
                error!("DB error: {:?}", e);
                return Err(AppError::Auth("User already exists".into()));
            }
        }

        Ok(())
    }

    pub async fn handle_login(
        &self,
        writer: &mut tokio::net::tcp::OwnedWriteHalf,
        username: String,
        password: String,
    ) -> Result<(), AppError> {

        let stored_hash = match db::get_password_hash(&self.db, &username).await? {
            Some(hash) => hash,
            None => {
                return Err(AppError::Auth("User not found".into()));
            }
        };

        if !bcrypt::verify(&password, &stored_hash)? {
            return Err(AppError::Auth("Invalid Password".into()));
        }   

        let session_id = Uuid::new_v4().to_string();
        self.sessions.insert(session_id.clone(), Session { username, created_at: Instant::now() });

        writer.write_all(format!("SESSION {}\n", session_id).as_bytes()).await?;

        Ok(())
    }

    pub async fn handle_join(
        &self,
        mut reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
        writer: &mut tokio::net::tcp::OwnedWriteHalf,
        addr: SocketAddr,
        group_id: GroupId,
        session_id: String
    ) -> Result<(), AppError> {
        let mut to_write = String::new();

        let session = self.sessions.get(&session_id)
            .ok_or(AppError::Auth("Invalid session".into()))?;

        let max_age = std::time::Duration::from_secs(60 * 30); // 30 min

        if session.created_at.elapsed() > max_age {
            self.sessions.remove(&session_id);
            return Err(AppError::Auth("Session expired".into()));
        }

        let username = session.username.clone();
        let group_id = group_id.trim().to_string();

        // Get or create group channel
        let tx = self.groups
            .entry(group_id.clone())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(100);
                tx
            })
            .clone();

        let mut rx = tx.subscribe();

        let mut db_tx = self.db.begin().await?;
        if !db::is_user_registered(&self.db, &username).await {
            return Err(AppError::Auth("User not registered".into()));
        }
        let join_event = db::Join {
            group_id: group_id.clone(),
            username: username.clone(),
            timestamp: chrono::Utc::now(),
        };

        db::insert_join_event(&mut db_tx, join_event).await?;

        if let Err(e) = tx.send((Event::Join { user: username.clone() }, addr)) {
            error!("Broadcast error: {}", e);
        }
        db_tx.commit().await?;

        let group_id_int = group_id
            .parse::<i32>()
            .map_err(|_| AppError::InvalidInput("Invalid group_id".into()))?;

        let chat_history = db::chat_history(&self.db, group_id_int).await;
       
        for message in chat_history {
            writer.write_all(format!("{}: {}\n", message.username, message.content).as_bytes()).await?;
        }
        loop {
            tokio::select! {
                // Read incoming message
                result = reader.read_line(&mut to_write) => {
                    match result {
                        Ok(0) => {
                            break;
                        },
                        Ok(_) => {
                            let trimmed_message = to_write.trim_end();
                            if trimmed_message.is_empty() {
                                to_write.clear();
                                continue;
                            }
                            if trimmed_message.len() > 512 {
                                if let Err(e) = writer.write_all(b"Message too long. Please limit to 512 characters.\n").await {
                                    error!("Error: {}", e);
                                    break;
                                }
                                to_write.clear();
                                continue;
                            }
                           
                            let mut db_tx = self.db.begin().await?;
                            let message_event = db::Message {
                                group_id: group_id.clone(),
                                username: username.clone(),
                                content: trimmed_message.to_string(),
                                timestamp: chrono::Utc::now(),
                            };

                            db::insert_message_event(&mut db_tx, message_event).await?;
                            db_tx.commit().await?;

                            // THEN broadcast
                            if let Err(e) = tx.send((Event::Message {
                                user: username.clone(),
                                msg: trimmed_message.to_string()
                            }, addr)) {
                                error!("Broadcast error: {}", e);
                            }
                            to_write.clear();
                        },
                        Err(e) => {
                            error!("Error: {}", e);
                            break;
                        }
                    }
                }

                // Receive and forward messages
                result = rx.recv() => {
                    match result {
                        Ok((event, other_addr)) => {
                            if other_addr == addr {
                                continue;
                            }
                            match event {
                                Event::Message{user, msg} => {
                                    if msg.is_empty() {
                                        continue;
                                    }
                                    // Write the received message to the client's terminal
                                    if let Err(e) = writer.write_all(format!("{}: {}\n", user, msg).as_bytes()).await {
                                        error!("Error: {}", e);
                                        break;
                                    }
                                },
                                Event::Join{user} => {
                                    if let Err(e) = writer.write_all(format!("{} has joined the chat.\n", user).as_bytes()).await {
                                        error!("Error: {}", e);
                                        break;
                                    }
                                },
                                Event::Leave{user} => {
                                    if let Err(e) = writer.write_all(format!("{} has left the chat.\n", user).as_bytes()).await {
                                        error!("Error: {}", e);

                                        break;
                                    }
                                },
                            }
                        },
                        Err(broadcast::error::RecvError::Lagged(count)) => {
                            error!("{} lagged {} messages", addr, count);
                        },
                        Err(broadcast::error::RecvError::Closed) => {
                            error!("Broadcast channel closed");
                            break;
                        }
                    }
                }
            }
        }

        // Notify all clients that the user has left
        let mut db_tx = self.db.begin().await?;
        let leave_event = db::Leave {
            group_id: group_id.clone().parse::<i32>()
                    .map_err(|_| AppError::InvalidInput("Invalid group ID".into()))?,
            username: username.clone(),
            timestamp: chrono::Utc::now(),
        };

        if let Err(e) = db::insert_leave_event(&mut db_tx, leave_event).await {
            error!("Insert message error: {}", e);
        }
        db_tx.commit().await?;
        if let Err(e) = tx.send((
            Event::Leave {
                user: username.clone(),
            },
            addr,
        )) {
            error!("Error: {}", e);
        }
        if tx.receiver_count() == 0 {
            self.groups.remove(&group_id);
        }
        Ok(())
    }
}

async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt().init();

    let db_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    let db_pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&db_url)
        .await?;

    let server = Arc::new(AuthImpl {
        db: db_pool,
        groups: DashMap::new(),
        sessions: DashMap::new()
    });

    let listener = TcpListener::bind("127.0.0.1:8081").await?;
    info!("Server running on 127.0.0.1:8081");

    let server_clone = Arc::clone(&server);

    tokio::spawn(async move {
        let interval = std::time::Duration::from_secs(600); // 10 min
        let mut ticker = tokio::time::interval(interval);

        loop {
            ticker.tick().await;
            let before = server_clone.sessions.len();
            info!("Running session cleanup...");
            let now = Instant::now();
            let max_age = std::time::Duration::from_secs(60 * 30);

            server_clone.sessions.retain(|_, session| {
                now.duration_since(session.created_at) < max_age
            });
            let after = server_clone.sessions.len();
            info!("Session cleanup complete. {} sessions removed, {} remain.", before - after, after);
        }
    });

    loop {
        let (socket, addr) = listener.accept().await?;

        let server_clone = Arc::clone(&server);

        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket, server_clone, addr).await {
                error!("Error handling client {}: {:?}", addr, e);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{net::TcpStream, io::{AsyncWriteExt, AsyncBufReadExt, BufReader}};
    use dotenvy::from_filename;
    use sqlx::postgres::PgPoolOptions;

    async fn spawn_test_server() -> String {
        from_filename(".env.test").ok();

        let db_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set");

        let db_pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(&db_url)
            .await
            .unwrap();

        let server = Arc::new(AuthImpl {
            db: db_pool,
            groups: DashMap::new(),
            sessions: DashMap::new(),
        });

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            loop {
                let (socket, addr) = listener.accept().await.unwrap();
                let server_clone = Arc::clone(&server);

                tokio::spawn(async move {
                    let _ = handle_connection(socket, server_clone, addr).await;
                });
            }
        });

        format!("{}", addr)
    }

    async fn send_command(addr: &str, cmd: Command) -> String {
        let mut stream = TcpStream::connect(addr).await.unwrap();

        let json = serde_json::to_string(&cmd).unwrap();
        stream.write_all(format!("{}\n", json).as_bytes()).await.unwrap();

        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        reader.read_line(&mut response).await.unwrap();

        response
    }

    // ----------------------------------------
    // 🧪 REGISTER TESTS
    // ----------------------------------------

    #[tokio::test]
    async fn test_register_success() {
        let addr = spawn_test_server().await;

        let res = send_command(
            &addr,
            Command::Register {
                username: "test_user".into(),
                password: "pass".into(),
            },
        )
        .await;

        assert!(res.contains("User registered"));
    }

    #[tokio::test]
    async fn test_register_duplicate() {
        let addr = spawn_test_server().await;

        let _ = send_command(
            &addr,
            Command::Register {
                username: "dup_user".into(),
                password: "pass".into(),
            },
        )
        .await;

        let res = send_command(
            &addr,
            Command::Register {
                username: "dup_user".into(),
                password: "pass".into(),
            },
        )
        .await;

        assert!(res.contains("ERROR"));
    }

    // ----------------------------------------
    // 🧪 LOGIN TESTS
    // ----------------------------------------

    #[tokio::test]
    async fn test_login_success() {
        let addr = spawn_test_server().await;

        let _ = send_command(
            &addr,
            Command::Register {
                username: "login_user".into(),
                password: "pass".into(),
            },
        )
        .await;

        let res = send_command(
            &addr,
            Command::Login {
                username: "login_user".into(),
                password: "pass".into(),
            },
        )
        .await;

        assert!(res.contains("SESSION"));
    }

    #[tokio::test]
    async fn test_login_wrong_password() {
        let addr = spawn_test_server().await;

        let _ = send_command(
            &addr,
            Command::Register {
                username: "user1".into(),
                password: "correct".into(),
            },
        )
        .await;

        let res = send_command(
            &addr,
            Command::Login {
                username: "user1".into(),
                password: "wrong".into(),
            },
        )
        .await;

        assert!(res.contains("ERROR"));
    }

    // ----------------------------------------
    // 🧪 SESSION TESTS
    // ----------------------------------------

    #[tokio::test]
    async fn test_logout() {
        let addr = spawn_test_server().await;

        let _ = send_command(
            &addr,
            Command::Register {
                username: "logout_user".into(),
                password: "pass".into(),
            },
        )
        .await;

        let res = send_command(
            &addr,
            Command::Login {
                username: "logout_user".into(),
                password: "pass".into(),
            },
        )
        .await;

        let session_id = res.replace("SESSION ", "").trim().to_string();

        let res = send_command(
            &addr,
            Command::Logout { session_id },
        )
        .await;

        assert!(res.contains("Logged out"));
    }

    // ----------------------------------------
    // 🧪 JOIN TESTS
    // ----------------------------------------

    #[tokio::test]
    async fn test_join_invalid_session() {
        let addr = spawn_test_server().await;

        let res = send_command(
            &addr,
            Command::Join {
                group_id: "1".into(),
                session_id: "invalid".into(),
            },
        )
        .await;

        assert!(res.contains("ERROR"));
    }

    // ----------------------------------------
    // 🧪 INVALID COMMAND
    // ----------------------------------------

    #[tokio::test]
    async fn test_invalid_json() {
        let addr = spawn_test_server().await;

        let mut stream = TcpStream::connect(&addr).await.unwrap();
        stream.write_all(b"invalid_json\n").await.unwrap();

        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        reader.read_line(&mut response).await.unwrap();

        assert!(response.contains("Invalid command"));
    }
}