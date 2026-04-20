# 🔥 Async Chat Server (Rust + Tokio + PostgreSQL)

A **multi-client, real-time chat system** built in Rust using Tokio, designed to demonstrate **async concurrency, protocol design, and backend system architecture**.

---

## 🚀 Overview

This project implements a **stateful chat server** where multiple clients can:

- Register with a username
- Join chat groups
- Exchange messages in real-time
- Receive join/leave notifications

### ✨ Key Features

- Concurrent connection handling
- Event broadcasting
- Database-backed persistence
- Command-based client-server protocol

---

## 🧠 Key Concepts Demonstrated

- Asynchronous Rust (Tokio runtime)
- Concurrent system design
- TCP-based protocol handling
- Database integration with SQLx
- Event-driven architecture
- State management across clients

---

## 🏗️ Architecture

```text
CLI Client
    ↓ TCP
Server (Tokio)
    ↓
PostgreSQL
```

## ⚙️ Components

### Client

- CLI-based interface using `clap`
- Sends structured commands (`register`, `join`)
- Maintains persistent connection for chat

### Server

- Handles multiple clients concurrently (`tokio::spawn`)
- Dispatches commands (`register`, `join`)
- Broadcasts events using `tokio::sync::broadcast`
- Maintains group-based chat state

### Database

- Stores users, group membership, and messages
- Accessed via SQLx with compile-time query validation

---

## ⚙️ Tech Stack

- Rust
- Tokio (async runtime)
- SQLx (PostgreSQL with compile-time checks)
- Clap (CLI parsing)
- PostgreSQL

---

## 🔄 System Flow

### 1. Registration

```text
Client → register
Client → username
Server → inserts user into DB
```

### 2. Join Chat

```text
Client → join
Client → username
Client → group_id
Server → validates user
Server → registers membership
Server → enters chat loop
```

### 3. Messaging

- Messages are:
  - written to DB
  - broadcast to all connected clients in the same group

---

### 4. Disconnection

- When a client disconnects:
  - a leave event is recorded
  - broadcast sent to group

---

## 🧵 Concurrency Model

Each client runs in its own async task:

```rust
tokio::spawn(async move {
    // handle connection
});
```

Message distribution uses:

```rust
tokio::sync::broadcast
```

## ✅ Benefits

1.  Non-blocking communication
2.  Scalable fan-out to multiple clients

## 🗄️ Database Design

### Tables

```text
users → registered users
group_members → active group membership
group_requests → join/leave history
group_chats → message storage
```

## 📡 Protocol Design

The system uses a simple command-based TCP protocol:

```text
Register
register
username
Join
join
username
group_id
```

After joining, the connection becomes a real-time message stream.

## 🚀 Running the Project

1. Setup Database
   createdb chat_server
   export DATABASE_URL=postgres://postgres:password@localhost:5432/chat_server
   cargo sqlx migrate run
2. Run Server
   cargo run --bin server
3. Run Client
   cargo run --bin client -- register <username>
   cargo run --bin client -- join <username> <group_id>
   🧪 Example

# Terminal 1

```bash
cargo run --bin server
```

# Terminal 2

```bash
cargo run --bin client -- join alice 1
```

# Terminal 3

```bash
cargo run --bin client -- join bob 1
```

👉 Real-time chat begins immediately.

## ⚠️ Limitations

Uses a simple text-based protocol (not JSON/protobuf)
No authentication or access control
In-memory group broadcast (not distributed)

## 🔮 Future Improvements

Structured protocol using serde (JSON)
WebSocket support for browser clients
Redis-backed pub/sub for scaling
Authentication and private groups
Message pagination and history APIs

## 💡 Why This Project Stands Out

This project demonstrates:

Real-world async Rust patterns
Understanding of concurrent systems
Clean separation of client/server responsibilities
Database-backed state management
Practical networking beyond simple examples

## 🏁 Summary

A production-style foundation for a chat system, showcasing how to combine:

async Rust
networking
database systems
concurrency primitives

into a cohesive backend service.

```

```
