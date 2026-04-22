// Package typecheckgo anchors the go.mod pgx requirement so `go mod tidy`
// retains it + writes a complete go.sum covering the transitive graph.
// The actual compile-check workload lives in tests/typecheck-go/generated/,
// populated at runtime by crates/sqlcx-core/tests/typecheck_go.rs.
package typecheckgo

import (
	_ "github.com/jackc/pgx/v5"
	_ "github.com/jackc/pgx/v5/pgconn"
)
