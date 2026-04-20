use std::{env, error::Error, net::SocketAddr, sync::Arc, time::Instant};
use tokio::{io::{AsyncBufReadExt, AsyncWriteExt}, net::TcpStream};
use chrono::DateTime;
use dashmap::DashMap;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::{io::BufReader, net::TcpListener, sync::broadcast};
use tracing::info;
use chat_stream_tokio::db;

type GroupId = String;

#[derive(Debug, Clone)]
enum Event {
    Message { user: String, msg: String },
    Join { user: String },
    Leave { user: String },
}

#[derive(Debug, Clone)]
pub struct AuthImpl {
    pub db: PgPool,
    pub groups: DashMap<GroupId, broadcast::Sender<(Event, std::net::SocketAddr)>>
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
) -> Result<(), Box<dyn Error>> {

    let (reader, writer) = socket.into_split();
    let mut reader = BufReader::new(reader);

    let mut command = String::new();
    reader.read_line(&mut command).await?;
    let command = command.trim();

    match command {
        "register" => server.handle_register(reader, writer).await?,
        "join" => server.handle_join(reader, writer, addr).await?,
        _ => {
            let mut writer = writer;
            writer.write_all(b"Unknown command\n").await?;
        }
    }

    Ok(())
}

impl AuthImpl {
    pub async fn handle_register(
        &self,
        mut reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
        mut writer: tokio::net::tcp::OwnedWriteHalf,
    ) -> Result<(), Box<dyn Error>> {

        let mut username = String::new();
        reader.read_line(&mut username).await?;
        let username = username.trim().to_string();

        let mut tx = self.db.begin().await?;

        let user = db::User {
            username: username.clone(),
            created_at: chrono::Utc::now(),
        };

        match db::insert_user(&mut tx, user).await {
            Ok(_) => {
                tx.commit().await?;
                writer.write_all(b"User registered\n").await?;
            }
            Err(_) => {
                writer.write_all(b"User exists or DB error\n").await?;
            }
        }

        Ok(())
    }

    pub async fn handle_join(
        &self,
        mut reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
        mut writer: tokio::net::tcp::OwnedWriteHalf,
        addr: SocketAddr,
    ) -> Result<(), Box<dyn Error>> {
        let mut to_write = String::new();
        let mut username = String::new();
        reader.read_line(&mut username).await?;
        let username = username.trim().to_string();
        
        let mut group_input = String::new();
        reader.read_line(&mut group_input).await?;
        let group_id = group_input.trim().to_string();
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
            writer.write_all(b"User not registered\n").await?;
            return Ok(());
        }
        let join_event = db::Join {
            group_id: group_id.clone(),
            username: username.clone(),
            timestamp: chrono::Utc::now(),
        };
        tx.send((Event::Join { user: username.clone() }, addr))?;
        if let Err(e) = db::insert_join_event(&mut db_tx, join_event).await {
            eprintln!("Database error: {}", e);
        }
        db_tx.commit().await?;

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
                                if let Err(e) = writer.write_all(b"Message too long. Please limit to 200 characters.\n").await {
                                    eprintln!("Error: {}", e);
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

                            let _ = db::insert_message_event(&mut db_tx, message_event).await;
                            db_tx.commit().await?;
                            // THEN broadcast
                            tx.send((Event::Message {
                                user: username.clone(),
                                msg: trimmed_message.to_string()
                            }, addr))?;
                            to_write.clear();
                        },
                        Err(e) => {
                            eprintln!("Error: {}", e);
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
                                        eprintln!("Error: {}", e);
                                        break;
                                    }
                                },
                                Event::Join{user} => {
                                    if let Err(e) = writer.write_all(format!("{} has joined the chat.\n", user).as_bytes()).await {
                                        eprintln!("Error: {}", e);
                                        break;
                                    }
                                },
                                Event::Leave{user} => {
                                    if let Err(e) = writer.write_all(format!("{} has left the chat.\n", user).as_bytes()).await {
                                        eprintln!("Error: {}", e);

                                        break;
                                    }
                                },
                            }
                        },
                        Err(broadcast::error::RecvError::Lagged(count)) => {
                            eprintln!("{} lagged {} messages", addr, count);
                        },
                        Err(broadcast::error::RecvError::Closed) => {
                            eprintln!("Broadcast channel closed");
                            break;
                        }
                    }
                }
            }
        }

        // Notify all clients that the user has left
        let mut db_tx = self.db.begin().await?;
        let leave_event = db::Leave {
            group_id: group_id.clone().parse::<i32>().unwrap_or(0),
            username: username.clone(),
            timestamp: chrono::Utc::now(),
        };

        let _ = db::insert_leave_event(&mut db_tx, leave_event).await;
        db_tx.commit().await?;
        if let Err(e) = tx.send((
            Event::Leave {
                user: username.clone(),
            },
            addr,
        )) {
            eprintln!("Error: {}", e);
        }
        if tx.receiver_count() == 0 {
            self.groups.remove(&group_id);
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    });

    let listener = TcpListener::bind("127.0.0.1:8081").await?;
    info!("Server running on 127.0.0.1:8081");

    loop {
        let (socket, addr) = listener.accept().await?;

        let server_clone = Arc::clone(&server);

        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket, server_clone, addr).await {
                eprintln!("Error handling client {}: {}", addr, e);
            }
        });
    }
}