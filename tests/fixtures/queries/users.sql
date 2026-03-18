-- name: GetUser :one
SELECT * FROM users WHERE id = $1;

-- name: ListUsers :many
SELECT id, name, email FROM users WHERE name ILIKE $1;

-- name: CreateUser :exec
INSERT INTO users (name, email, bio) VALUES ($1, $2, $3);

-- name: DeleteUser :execresult
DELETE FROM users WHERE id = $1;

-- name: ListUsersByDateRange :many
-- @param $1 start_date
-- @param $2 end_date
SELECT * FROM users WHERE created_at > $1 AND created_at < $2;
