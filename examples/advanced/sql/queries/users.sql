-- name: GetUserById :one
SELECT * FROM users WHERE id = $1;

-- name: GetUserByUsername :one
SELECT * FROM users WHERE username = $1;

-- name: ListUsers :many
SELECT id, username, email, role FROM users ORDER BY created_at DESC;

-- name: CreateUser :one
INSERT INTO users (username, email, role) VALUES ($1, $2, $3) RETURNING *;

-- name: UpdateUserBio :exec
UPDATE users SET bio = $1, updated_at = NOW() WHERE id = $2;

-- name: SearchUsers :many
SELECT * FROM users WHERE username ILIKE $1 OR email ILIKE $1;
