# 🔥 Low-Latency Async Chat Server (Rust + Tokio + PostgreSQL)

A **real-time, multi-client chat system** built in Rust using Tokio, designed to demonstrate **async concurrency, low-latency networking, and backend system design**.

## 🚀 Overview

This project implements a **stateful TCP-based chat server** capable of handling multiple concurrent clients with non-blocking I/O.

Clients can:

- Register with a username
- Join chat groups
- Exchange messages in real time
- Receive join/leave events

## ✨ Key Features

- ⚡ Low-latency async networking using Tokio
- 🔁 Concurrent client handling via task-based execution
- 📡 Real-time message broadcasting
- 🧠 Stateful session and group management
- 🗄️ PostgreSQL-backed persistence (SQLx)

## 🧠 Core Concepts Demonstrated

- Async Rust (Tokio runtime)
- Concurrent system design (task scheduling, synchronization)
- TCP protocol design and request handling
- Event-driven architecture
- Database-backed state management
- Backpressure-safe message distribution

## 🏗️ Architecture

```
CLI Client
│
▼
TCP Connection
│
▼
Async Server (Tokio)
│
▼
PostgreSQL (SQLx)
```

## ⚙️ System Components

### Client

- CLI-based interface using `clap`
- Sends structured commands (`register`, `join`)
- Maintains persistent TCP connection for real-time messaging

### Server

- Handles each client in an independent async task (`tokio::spawn`)
- Parses and dispatches commands
- Maintains shared state across clients (groups, sessions)
- Broadcasts messages using `tokio::sync::broadcast`

### Database

- Stores users, group membership, and messages
- Accessed via SQLx with compile-time query validation
- Ensures transactional consistency for concurrent operations

## 🔄 System Flow

### 1. Registration

```
Client → register
Client → username
Server → insert user into DB
```

### 2. Join Group

```
Client → join
Client → username + group_id
Server → validate user
Server → register membership
Server → enter chat loop
```

### 3. Messaging

- Messages are:
  - persisted to PostgreSQL
  - broadcast to all clients in the same group

### 4. Disconnection

- Client disconnect triggers:
  - membership update
  - broadcast of leave event

## 🧵 Concurrency Model

Each client connection runs in its own async task:

```rust
tokio::spawn(async move {
    // handle client connection
});
```

Message distribution uses:

```rust
tokio::sync::broadcast
```

### Properties:

- Non-blocking I/O
- Efficient fan-out to multiple clients
- Minimal contention using concurrent data structures

## 🗄️ Data Model

- users → registered users
- group_members → active memberships
- group_requests → join/leave history
- group_chats → message storage

## 📡 Protocol Design

A simple command-based TCP protocol:

```bash
register
<username>

join
<username>
<group_id>
```

After joining, the connection transitions into a real-time message stream.

## ⚙️ Tech Stack

- Rust
- Tokio (async runtime)
- SQLx (PostgreSQL with compile-time checks)
- PostgreSQL
- Clap (CLI interface)

## 🚀 Running the Project

1. Setup Database

```bash
createdb chat_server
export DATABASE_URL=postgres://postgres:password@localhost:5432/chat_server
cargo sqlx migrate run
```

2. Run Server

```bash
cargo run --bin server
```

3. Run Clients

```bash
cargo run --bin client -- register alice
cargo run --bin client -- join alice 1
```

## 🧪 Example

Terminal 1

```bash
cargo run --bin server
```

Terminal 2

```bash
cargo run --bin client -- join alice 1
```

Terminal 3

```bash
cargo run --bin client -- join bob 1
```

👉 Real-time communication starts immediately.

## ⚠️ Limitations

- Text-based protocol (no structured encoding like JSON/protobuf)
- No authentication or authorization
- In-memory broadcast (not horizontally scalable)
- Single-node deployment

## 🔮 Future Improvements

- Structured protocol (JSON / Protobuf)
- WebSocket support for browser clients
- Distributed pub/sub (Redis / NATS)
- Authentication and access control
- Horizontal scaling with shared state layer

## 💡 Why This Project Matters

This project demonstrates:

- Building low-latency networked systems in Rust
- Handling concurrency with async task scheduling
- Designing stateful backend services
- Managing real-time data flow and persistence

## 🏁 Summary

A practical backend system combining:

- async Rust
- networking
- concurrency primitives
- database systems

to deliver a real-time, scalable chat service foundation.
