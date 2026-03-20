CREATE TYPE post_status AS ENUM ('draft', 'published', 'archived');

CREATE TABLE users (
  id SERIAL PRIMARY KEY,
  username TEXT NOT NULL UNIQUE,
  email TEXT NOT NULL,
  bio TEXT,
  -- @enum("admin", "editor", "viewer")
  role TEXT NOT NULL DEFAULT 'viewer',
  -- @json({ theme: string, language: string, notifications: boolean })
  preferences JSONB,
  tags TEXT[],
  created_at TIMESTAMP NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE posts (
  id SERIAL PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users(id),
  title TEXT NOT NULL,
  slug TEXT NOT NULL UNIQUE,
  body TEXT NOT NULL,
  status post_status NOT NULL DEFAULT 'draft',
  -- @json({ views: number, likes: number, shares: number })
  stats JSONB NOT NULL,
  published_at TIMESTAMP,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE comments (
  id SERIAL PRIMARY KEY,
  post_id INTEGER NOT NULL REFERENCES posts(id),
  user_id INTEGER NOT NULL REFERENCES users(id),
  body TEXT NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
