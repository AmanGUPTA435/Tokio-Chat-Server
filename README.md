# Chat Server

This is a simple multi-client chat server implemented in Rust using the Tokio asynchronous runtime. Clients can connect to the server, set a username, and send messages to each other in real-time. The server broadcasts join, leave, and message events to all connected clients.

## Features

- Multiple clients can connect to the server simultaneously.
- Clients are prompted to enter a username when they connect.
- Messages sent by one client are broadcast to all other connected clients.
- Notifications are sent when a user joins or leaves the chat.
- Fully asynchronous implementation using Tokio.

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) installed on your system.
- Basic understanding of Rust and Tokio library.

## Getting Started

### Cloning the Repository

```bash
# Clone this repository
git clone https://github.com/AmanGUPTA435/Tokio-Chat-Server.git

# Navigate to the project directory
cd tokio-chat-server
```

### Running the Server

1. Compile and run the server using Cargo:

   ```bash
   cargo run
   ```

2. The server will start listening on `localhost:8081`.

### Connecting to the Server

1. Use a TCP client like `telnet` or `nc` (netcat) to connect to the server.

   Example with `telnet`:

   ```bash
   telnet localhost 8081
   ```

2. Enter your username when prompted.

3. Start sending messages, and you will see messages from other connected clients in real-time.

## Code Overview

### Key Components

- **`TcpListener`**: Listens for incoming client connections.
- **`broadcast`**: Used for sharing messages between clients efficiently.
- **`tokio::spawn`**: Creates a separate asynchronous task for each client.
- **`tokio::select!`**: Concurrently handles incoming client messages and broadcast events.

### Functionality

1. **Client Connection**:

   - Clients are prompted to enter their username upon connection.
   - A join message is broadcast to notify other clients.

2. **Message Handling**:

   - Messages sent by a client are broadcast to all other connected clients.

3. **Client Disconnection**:
   - A leave message is broadcast when a client disconnects.

### Sample Interaction

1. Client 1 connects and enters the username "Alice":

   ```
   Enter your username: Alice
   Alice has joined the chat.
   ```

2. Client 2 connects and enters the username "Bob":

   ```
   Enter your username: Bob
   Bob has joined the chat.
   ```

3. Client 1 sends a message:

   ```
   Alice: Hello, everyone!
   ```

4. Client 2 receives the message:

   ```
   Alice: Hello, everyone!
   ```

5. Client 2 sends a reply:

   ```
   Bob: Hi Alice!
   ```

6. Client 1 receives the reply:

   ```
   Bob: Hi Alice!
   ```

7. When a client disconnects, a leave message is broadcast:

   ```
   Bob has left the chat.
   ```

## Contributing

Feel free to fork this repository and submit pull requests for new features or bug fixes.
