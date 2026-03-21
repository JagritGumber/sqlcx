use sqlcx_core::{
    config::TargetConfig,
    generator::resolve_language,
    ir::SqlcxIR,
    parser::resolve_parser,
};

const SCHEMA_SQL: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/schema.sql"));
const USERS_SQL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/queries/users.sql"
));

#[test]
fn full_pipeline_fixture_snapshot() {
    let parser = resolve_parser("postgres").unwrap();
    let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
    let queries = parser
        .parse_queries(USERS_SQL, &tables, &enums, "queries/users.sql")
        .unwrap();
    let ir = SqlcxIR {
        tables,
        queries,
        enums,
    };

    let plugin = resolve_language("typescript", "typebox", "bun-sql").unwrap();
    let config = TargetConfig {
        language: "typescript".to_string(),
        out: "./src/db".to_string(),
        schema: "typebox".to_string(),
        driver: "bun-sql".to_string(),
        overrides: std::collections::HashMap::new(),
    };
    let files = plugin.generate(&ir, &config).unwrap();

    assert!(
        files.len() >= 3,
        "Expected at least 3 files, got {}",
        files.len()
    );

    for file in &files {
        let snapshot_name = file.path.replace("./", "").replace('/', "__");
        insta::assert_snapshot!(format!("e2e_{}", snapshot_name), &file.content);
    }
}

#[test]
fn ir_json_output_matches_ts_shape() {
    let parser = resolve_parser("postgres").unwrap();
    let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
    let ir = SqlcxIR {
        tables,
        queries: vec![],
        enums,
    };

    let json = serde_json::to_string_pretty(&ir).unwrap();
    assert!(json.contains("\"primaryKey\""), "Expected camelCase primaryKey");
    assert!(json.contains("\"hasDefault\""), "Expected camelCase hasDefault");
    assert!(!json.contains("\"primary_key\""), "Should not contain snake_case");
    assert!(!json.contains("\"has_default\""), "Should not contain snake_case");
}
