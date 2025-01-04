use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::broadcast,
};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("localhost:8081").await.unwrap();
    let (tx, _) = broadcast::channel(10); // Broadcast channel for message sharing
    println!("Server started at localhost:8081");

    loop {
        let (mut socket, addr) = listener.accept().await.unwrap();
        let tx = tx.clone();
        let mut rx = tx.subscribe();

        tokio::spawn(async move {
            let mut username = String::new();
            let mut to_write = String::new();
            let (reader, mut writer) = socket.split();
            let mut reader = BufReader::new(reader);

            // Ask for username
            writer.write_all(b"Enter your username: ").await.unwrap();
            reader.read_line(&mut username).await.unwrap();
            username = username.trim().to_string(); // Trim whitespace/newlines

            // Notify all clients that a new user has joined
            let join_message = format!("{} has joined the chat.\n", username);
            tx.send((join_message.clone(), addr)).unwrap();
            writer.write_all(join_message.as_bytes()).await.unwrap();

            loop {
                tokio::select! {
                    // Read incoming message
                    result = reader.read_line(&mut to_write) => {
                        if result.unwrap() == 0 {
                            break;
                        }
                        let trimmed_message = to_write.trim_end();
                        if let Err(e) = tx.send((format!("{}: {}", username, trimmed_message), addr)) {
                            eprint!("Error: {}", e);
                        }
                        to_write.clear();
                    }

                    // Receive and forward messages
                    result = rx.recv() => {
                        match result {
                            Ok((mssg, other_addr)) => {
                                if other_addr != addr {
                                    // Write the received message to the client's terminal
                                    writer.write_all(format!("{}\n", mssg).as_bytes()).await.unwrap();
                                }
                            },
                            Err(e) => {
                                eprintln!("received error: {}", e);
                                break;
                            }
                        }
                    }
                }
            }

            // Notify all clients that the user has left
            let leave_message = format!("{} has left the chat.\n", username);
            tx.send((leave_message.clone(), addr)).unwrap();
            println!("{}", leave_message);
        });
    }
}

