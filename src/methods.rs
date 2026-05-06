use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Subcommand, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CliCommand {
    Register {
        username: String,
        password: String
    },
    Login {
        username: String,
        password: String
    },
    Join {
        group_id: String
    },
    Logout {
        session_id: String
    },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum Command {
    Register { username: String, password: String },
    Login { username: String, password: String },
    // internal use (network layer)
    Join {
        session_id: String,
        group_id: String,
    },
    Logout { session_id: String },
}