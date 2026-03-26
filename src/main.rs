use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener},
    sync::broadcast,
};
use tokio::sync::Semaphore;
use std::sync::Arc;

#[derive(Debug, Clone)]
enum Event {
    Message{user: String, msg: String},
    Join{user: String},
    Leave{user: String},
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("localhost:8081").await?;
    let (tx, _) = broadcast::channel(10); // Broadcast channel for message sharing
    let limit = Arc::new(Semaphore::new(100)); // Limit to 100 concurrent clients
    println!("Server started at localhost:8081");

    loop {
        let (socket, addr) = listener.accept().await?;
        let tx = tx.clone();
        let mut rx = tx.subscribe();
        let permit = limit.clone().acquire_owned().await?;

        tokio::spawn(async move {
            let _permit = permit; // Hold the permit for the duration of the connection
            if let Err(e) = handle_client(socket, tx, addr, rx).await {
                eprintln!("Error handling client {}: {}", addr, e);
            }
        });
    }

    Ok(())
}

async fn handle_client(socket: tokio::net::TcpStream, tx: broadcast::Sender<(Event, std::net::SocketAddr)>, addr: std::net::SocketAddr, mut rx: broadcast::Receiver<(Event, std::net::SocketAddr)>) -> Result<(), Box<dyn std::error::Error>> {
    let mut username = String::new();
    let mut to_write = String::new();
    let (reader, mut writer) = socket.into_split();
    let mut reader = BufReader::new(reader);
    // Ask for username
    writer.write_all(b"Enter your username: ").await?;
        
    reader.read_line(&mut username).await?;
    username = username.trim().to_string(); // Trim whitespace/newlines

    // Notify all clients that a new user has joined
    if let Err(e) = tx.send((Event::Join{user: username.clone()}, addr)) {
        eprintln!("Error: {}", e);
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
                            if let Err(e) = writer.write_all(b"Message too long. Please limit to 200 characters.\n").await {
                                eprintln!("Error: {}", e);
                                break;
                            }
                            to_write.clear();
                            continue;
                        }
                        if let Err(e) = tx.send((Event::Message{user: username.clone(), msg: trimmed_message.to_string()}, addr)) {
                            eprint!("Error: {}", e);
                        }
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
    if let Err(e) = tx.send((Event::Leave{user: username.clone()}, addr)) {
        eprintln!("Error: {}", e);
    }
    Ok(())
}
