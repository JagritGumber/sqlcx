-- name: GetUser :one
SELECT * FROM users WHERE id = ?;

-- name: ListUsers :many
SELECT id, name, email FROM users WHERE name LIKE ?;

-- name: CreateUser :exec
INSERT INTO users (name, email, bio) VALUES (?, ?, ?);

-- name: DeleteUser :execresult
DELETE FROM users WHERE id = ?;

-- name: ListUsersByDateRange :many
-- @param $1 start_date
-- @param $2 end_date
SELECT * FROM users WHERE created_at > ? AND created_at < ?;
