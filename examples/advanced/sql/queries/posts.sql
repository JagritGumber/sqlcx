-- name: GetPost :one
SELECT * FROM posts WHERE id = $1;

-- name: GetPostBySlug :one
SELECT * FROM posts WHERE slug = $1;

-- name: ListPostsByUser :many
SELECT * FROM posts WHERE user_id = $1 ORDER BY created_at DESC;

-- name: ListPublishedPosts :many
SELECT * FROM posts WHERE status = 'published' ORDER BY published_at DESC;

-- name: CreatePost :one
INSERT INTO posts (user_id, title, slug, body, stats) VALUES ($1, $2, $3, $4, $5) RETURNING *;

-- name: PublishPost :exec
UPDATE posts SET status = 'published', published_at = NOW() WHERE id = $1;

-- name: DeletePost :execresult
DELETE FROM posts WHERE id = $1;

-- name: ListPostsByDateRange :many
-- @param $1 start_date
-- @param $2 end_date
SELECT * FROM posts WHERE published_at > $1 AND published_at < $2;
