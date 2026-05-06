CREATE TABLE chat (
    id SERIAL PRIMARY KEY,
    user_name VARCHAR(255) NOT NULL,
    message TEXT NOT NULL,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_chat_user_name ON chat(user_name);

CREATE TABLE group_requests (
    user_name VARCHAR(255) NOT NULL,
    group_id INTEGER NOT NULL,
    request_type VARCHAR(10) NOT NULL CHECK (request_type IN ('join', 'leave')),
    status VARCHAR(20) DEFAULT 'pending',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);


CREATE TABLE group_members (
    user_name VARCHAR(255) NOT NULL,
    group_id INTEGER NOT NULL,
    is_active BOOLEAN DEFAULT TRUE,
    joined_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (user_name, group_id)
);

CREATE TABLE group_chats (
    group_id INTEGER NOT NULL,
    user_name VARCHAR(255) NOT NULL,
    message TEXT NOT NULL,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (user_name, group_id) REFERENCES group_members(user_name, group_id) ON DELETE CASCADE
);

CREATE INDEX idx_group_chats_group_id ON group_chats(group_id);
CREATE INDEX idx_group_chats_user_id ON group_chats(user_name);

CREATE TABLE users (
    username VARCHAR(255) UNIQUE NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    password_hash TEXT NOT NULL
);