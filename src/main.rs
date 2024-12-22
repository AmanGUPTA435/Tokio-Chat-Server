// use tokio::{io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader}, net::TcpListener, sync::broadcast};

use std::borrow::BorrowMut;

use tokio::{io::AsyncBufReadExt, spawn, sync::broadcast};
// #[tokio::main]
// async fn main() {
//     let listener = TcpListener::bind("localhost:8180").await.unwrap();
//     let (tx, mut _rx) = broadcast::channel(10);
//     loop {
//         let (mut socket, addr) = listener.accept().await.unwrap();
//         let tx = tx.clone(); 
//         let mut rx = tx.subscribe();   
//         tokio::spawn(async move {
//             let (reader, mut writer) = socket.split();
//             let mut reader = BufReader::new(reader);
//             let mut line = String::new();
//             loop{
//                 tokio::select! {
//                     result = reader.read_line(&mut line) => {
//                         if result.unwrap() == 0{
//                             break;
//                         }
//                         tx.send((line.clone(), addr)).unwrap();
//                         line.clear();
//                     }
//                     result = rx.recv() => {
//                         let (msg, other_addr) = result.unwrap();
//                         if addr != other_addr {
//                             writer.write_all(msg.as_bytes()).await.unwrap();
//                         } 
//                     }
//                 }
//                 // let bytes_read = reader.read_line(&mut line).await.unwrap();
//                 // if bytes_read == 0{
//                 //     break;
//                 // }
//                 // let msg = rx.recv().await.unwrap();
//                 // writer.write_all(msg.as_bytes()).await.unwrap();
//                 // tx.send(line.clone()).unwrap();
//                 // line.clear();
//             }
//         });
//     }
// }
// #[cfg_attr(forbid(unused_imports), )]
// #[expect(unused_attributes)]
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
                        // if let Err(err) = writer.write_all(to_write.as_bytes()).await {
                        //     eprintln!("Failed to write to client: {}", err);
                        //     break;
                        // }
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
            // std::io::stdin().read_line(&mut to_write).expect("Failed to read input");
            // let write = writer.write(to_write.as_bytes()).await;
            // let read = reader.try_read(&mut data);
            // tx.send(to_write.clone()).unwrap();
            // if write.ok() != read.ok() {
            //     println!("works");
            //     // break;
            // }
            // println!("{:?}",rx.recv().await);
            
            
        });
        // handle.await.unwrap();
        // let buffer:BufReader<tokio::net::TcpStream> = BufReader::new(&socket);
    }
    // socket.write().await;
    // Ok(())
}
