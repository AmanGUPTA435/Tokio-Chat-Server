use clap::{Parser, Subcommand};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Register {
        username: String,
    },
    Join {
        username: String,
        group_id: String,
    },
}

async fn register(username: String) -> Result<(), Box<dyn std::error::Error>> {
    let stream = TcpStream::connect("127.0.0.1:8081").await?;
    let (reader, mut writer) = stream.into_split();

    let mut reader = BufReader::new(reader);
    let mut response = String::new();

    // Send command
    writer.write_all(b"register\n").await?;

    // Send username
    writer.write_all(format!("{}\n", username).as_bytes()).await?;

    // Read response
    reader.read_line(&mut response).await?;
    println!("{}", response);

    Ok(())
}

async fn join_chat(
    username: String,
    group_id: String,
) -> Result<(), Box<dyn std::error::Error>> {

    let stream = TcpStream::connect("127.0.0.1:8081").await?;
    let (reader, mut writer) = stream.into_split();

    let mut reader = BufReader::new(reader);
    let mut server_msg = String::new();

    // Send command
    writer.write_all(b"join\n").await?;

    // Send username
    writer.write_all(format!("{}\n", username).as_bytes()).await?;

    // Send group id
    writer.write_all(format!("{}\n", group_id).as_bytes()).await?;

    println!("--- Joined chat ---");

    let stdin = tokio::io::stdin();
    let mut stdin_reader = BufReader::new(stdin);
    let mut input = String::new();

    loop {
        tokio::select! {
            // User input
            Ok(n) = stdin_reader.read_line(&mut input) => {
                if n == 0 {
                    break; // Ctrl+D
                }
                writer.write_all(input.as_bytes()).await?;
                input.clear();
            }

            // Server messages
            Ok(n) = reader.read_line(&mut server_msg) => {
                if n == 0 {
                    println!("Disconnected from server");
                    break;
                }
                print!("{}", server_msg);
                server_msg.clear();
            }
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Register { username } => {
            register(username).await?;
        }
        Commands::Join { username, group_id } => {
            join_chat(username, group_id).await?;
        }
    }

    Ok(())
}