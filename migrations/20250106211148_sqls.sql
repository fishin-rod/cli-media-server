-- Add migration script here
-- Table for storing users
CREATE TABLE users (
    id TEXT PRIMARY KEY,            -- Corresponds to `id: String`
    name TEXT NOT NULL,             -- Corresponds to `name: String`
    password TEXT NOT NULL          -- Corresponds to `password: String`
);

CREATE TABLE friends (
    user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
    friend_id TEXT REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, friend_id)
);
-- Table for storing posts
CREATE TABLE posts (
    id TEXT PRIMARY KEY,            -- Corresponds to `id: String`
    user_id TEXT NOT NULL,          -- Corresponds to `user_id: String`
    title TEXT NOT NULL,            -- Corresponds to `title: String`
    body TEXT NOT NULL,             -- Corresponds to `body: String`
    date TEXT NOT NULL,        -- Corresponds to `date: String` with timezone support
    likes INT DEFAULT 0,            -- Corresponds to `likes: i32`
    dislikes INT DEFAULT 0,         -- Corresponds to `dislikes: i32`
    FOREIGN KEY (user_id) REFERENCES users(id) -- Enforce referential integrity
);

-- Table for storing comments
CREATE TABLE comments (
    id TEXT PRIMARY KEY,            -- Corresponds to `id: String`
    user_id TEXT NOT NULL,          -- Corresponds to `user_id: String`
    post_id TEXT NOT NULL,          -- Corresponds to `post_id: String`
    body TEXT NOT NULL,             -- Corresponds to `body: String`
    date TEXT NOT NULL,        -- Corresponds to `date: String`
    likes INT DEFAULT 0,            -- Corresponds to `likes: i32`
    dislikes INT DEFAULT 0,         -- Corresponds to `dislikes: i32`
    FOREIGN KEY (user_id) REFERENCES users(id), -- Enforce referential integrity
    FOREIGN KEY (post_id) REFERENCES posts(id)  -- Enforce referential integrity
);