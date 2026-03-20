-- name: GetUser :one
SELECT * FROM users WHERE id = $1;

-- name: ListUsers :many
SELECT * FROM users ORDER BY created_at DESC;

-- name: CreateUser :exec
INSERT INTO users (name, email) VALUES ($1, $2);

-- name: DeleteUser :execresult
DELETE FROM users WHERE id = $1;
