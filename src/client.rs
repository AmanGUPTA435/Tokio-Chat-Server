use std::{fs, process};

use clap::Parser;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};
use tracing::{error, info};
use chat_stream_tokio::methods::{CliCommand, Cli, Command};

#[derive(Debug)]
enum ClientError {
    Io(std::io::Error),
    Serde(serde_json::Error),
    Server(String),
}

impl From<std::io::Error> for ClientError {
    fn from(e: std::io::Error) -> Self {
        ClientError::Io(e)
    }
}

impl From<serde_json::Error> for ClientError {
    fn from(e: serde_json::Error) -> Self {
        ClientError::Serde(e)
    }
}

async fn send_command(cmd: &Command) -> Result<TcpStream, ClientError> {
    let mut stream = TcpStream::connect("127.0.0.1:8081").await?;

    let json = serde_json::to_string(cmd)?;
    stream.write_all(format!("{}\n", json).as_bytes()).await?;

    Ok(stream)
}

async fn register(cmd: CliCommand) -> Result<(), ClientError> {
    let command = match cmd {
        CliCommand::Register { username, password } => {
            Command::Register { username, password }
        }
        _ => unreachable!(),
    };
    let stream = send_command(&command).await?;

    let mut reader = BufReader::new(stream);
    let mut response = String::new();

    reader.read_line(&mut response).await?;
    handle_server_response(&response)?;

    Ok(())
}

async fn login(cmd: CliCommand) -> Result<(), ClientError> {
    let command = match cmd {
        CliCommand::Login { username, password } => {
            Command::Login { username, password }
        }
        _ => unreachable!(),
    };
    let stream = send_command(&command).await?;
    
    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    
    reader.read_line(&mut response).await?;
    handle_server_response(&response)?;

    // Extract session
    if let Some(session_id) = response.strip_prefix("SESSION ") {
        let session_file = std::env::var("SESSION_FILE")
            .unwrap_or(".session".to_string());

        fs::write(&session_file, session_id.trim())?;
    }
    
    println!("{}", response.trim());

    Ok(())
}

async fn join_chat(cmd: CliCommand) -> Result<(), ClientError> {
    // Read session
    let session_file = std::env::var("SESSION_FILE")
        .unwrap_or(".session".to_string());
    println!("Using session file: {}", session_file);
    let session_id = match std::fs::read_to_string(&session_file) {
        Ok(s) => s.trim().to_string(),
        Err(_) => {
            error!("No session found. Please login first.");
            return Ok(());
        }
    };
    let command = match cmd {
        CliCommand::Join { group_id } => {
            Command::Join { session_id, group_id }
        }
        _ => unreachable!(),
    };

    let stream = send_command(&command).await?;
    let (reader, mut writer) = stream.into_split();

    let mut reader = BufReader::new(reader);
    let mut server_msg = String::new();

    println!("--- Joined chat ---");

    let stdin = tokio::io::stdin();
    let mut stdin_reader = BufReader::new(stdin);
    let mut input = String::new();

    loop {
        tokio::select! {
            Ok(n) = stdin_reader.read_line(&mut input) => {
                if n == 0 { break; }
                if let Err(e) = writer.write_all(input.as_bytes()).await {
                    error!("Connection error: {}", e);
                    break;
                }
                // writer.write_all(input.as_bytes()).await?;
                input.clear();
            }

            Ok(n) = reader.read_line(&mut server_msg) => {
                if n == 0 {
                    println!("Disconnected from server");
                    break;
                }
                if server_msg.starts_with("ERROR:") {
                    error!("{}", server_msg.trim());
                } else {
                    print!("{}", server_msg);
                }

                server_msg.clear();
            }
        }
    }

    Ok(())
}

fn handle_server_response(response: &str) -> Result<(), ClientError> {
    if response.starts_with("ERROR:") {
        return Err(ClientError::Server(
            response.replace("ERROR:", "").trim().to_string()
        ));
    }

    println!("{}", response.trim());
    Ok(())
}

async fn run_client() -> Result<(), ClientError> {
    let cli = Cli::parse();

    match cli.command {
        cmd @ CliCommand::Register { .. } => {
            register(cmd).await?;
        }
        cmd @ CliCommand::Join { .. } => {
            join_chat(cmd).await?;
        }
        cmd @ CliCommand::Login { .. } => {
            login(cmd).await?;
        }
        cmd @ CliCommand::Logout { .. } => {
            let session_file = std::env::var("SESSION_FILE")
                .unwrap_or(".session".to_string());

            let session_id = std::fs::read_to_string(&session_file)?.trim().to_string();

            let command = Command::Logout { session_id };

            let stream = send_command(&command).await?;
            let mut reader = BufReader::new(stream);

            let mut response = String::new();
            reader.read_line(&mut response).await?;

            handle_server_response(&response)?;

            std::fs::remove_file(&session_file).ok();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use chat_stream_tokio::methods::{Command, CliCommand};

    // ----------------------------------------
    // 🧪 HANDLE RESPONSE TESTS
    // ----------------------------------------

    #[test]
    fn test_handle_server_response_success() {
        let res = handle_server_response("User registered\n");
        assert!(res.is_ok());
    }

    #[test]
    fn test_handle_server_response_error() {
        let res = handle_server_response("ERROR: Invalid user\n");
        assert!(res.is_err());

        match res {
            Err(ClientError::Server(msg)) => {
                assert_eq!(msg, "Invalid user");
            }
            _ => panic!("Expected ClientError::Server"),
        }
    }

    // ----------------------------------------
    // 🧪 COMMAND SERIALIZATION
    // ----------------------------------------

    #[test]
    fn test_command_serialization() {
        let cmd = Command::Register {
            username: "user".into(),
            password: "pass".into(),
        };

        let json = serde_json::to_string(&cmd).unwrap();

        assert!(json.contains("Register"));
        assert!(json.contains("user"));
    }

    // ----------------------------------------
    // 🧪 CLI → COMMAND MAPPING
    // ----------------------------------------

    #[test]
    fn test_cli_to_command_mapping_register() {
        let cli = CliCommand::Register {
            username: "user".into(),
            password: "pass".into(),
        };

        let cmd = match cli {
            CliCommand::Register { username, password } => {
                Command::Register { username, password }
            }
            _ => unreachable!(),
        };

        match cmd {
            Command::Register { username, password } => {
                assert_eq!(username, "user");
                assert_eq!(password, "pass");
            }
            _ => panic!("Wrong mapping"),
        }
    }

    // ----------------------------------------
    // 🧪 SESSION FILE TESTS
    // ----------------------------------------

    #[test]
    fn test_session_file_write_and_read() {
        let file = ".test_session";

        fs::write(file, "session123").unwrap();

        let content = fs::read_to_string(file).unwrap();

        assert_eq!(content.trim(), "session123");

        fs::remove_file(file).unwrap();
    }

    #[test]
    fn test_session_file_missing() {
        let file = ".missing_session";

        let res = fs::read_to_string(file);

        assert!(res.is_err());
    }

    // ----------------------------------------
    // 🧪 LOGIN RESPONSE PARSING
    // ----------------------------------------

    #[test]
    fn test_session_extraction() {
        let response = "SESSION abc123\n";

        let session_id = response
            .strip_prefix("SESSION ")
            .unwrap()
            .trim();

        assert_eq!(session_id, "abc123");
    }

    // ----------------------------------------
    // 🧪 ERROR PROPAGATION
    // ----------------------------------------

    #[test]
    fn test_error_conversion_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::Other, "fail");

        let err: ClientError = io_err.into();

        match err {
            ClientError::Io(_) => {}
            _ => panic!("Expected Io error"),
        }
    }

    #[test]
    fn test_error_conversion_serde() {
        let err = serde_json::from_str::<Command>("invalid");

        assert!(err.is_err());

        let client_err: ClientError = err.unwrap_err().into();

        match client_err {
            ClientError::Serde(_) => {}
            _ => panic!("Expected Serde error"),
        }
    }
}