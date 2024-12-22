use tokio::{io::AsyncBufReadExt, spawn, sync::broadcast};
use tokio::{io::{AsyncWriteExt, BufReader}, net::TcpListener};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("localhost:8081").await.unwrap();
    let (tx, _) = broadcast::channel(10);
    loop {
        let (socket, addr) = listener.accept().await.unwrap();
        let tx = tx.clone();
        let mut rx = tx.subscribe();
        spawn(async move {
            let (reader, mut writer) = socket.into_split();
            // let mut data = vec![0;1024];
            let mut reader = BufReader::new(reader);
            let mut to_write = String::new();
            loop{
                tokio::select! {
                    result = reader.read_line(&mut to_write) => {
                        if result.unwrap_or(0) == 0 {
                            println!("Server disconnected {}", addr);
                            break;
                        }
                        // println!("Received from {}: {}", addr, to_write.trim());
                        if let Err(err) = tx.send((to_write.clone(), addr)) {
                            eprintln!("Broadcast Error: {}", err);
                        }
                        
                        to_write.clear();
                    }

                    result = rx.recv() => {
                        match result {
                            Ok((mssg, other_addr)) => {
                                if other_addr != addr {
                                    if let Err(err) = writer.write_all(mssg.as_bytes()).await {
                                        eprintln!("Can't write {err}");
                                        break;
                                    }
                                }
                            } 
                            Err(err) => {
                                eprintln!("Receive error: {}", err);
                                break;
                            }
                        }
                    }
                }
            }            
        });        
    }
    
}
