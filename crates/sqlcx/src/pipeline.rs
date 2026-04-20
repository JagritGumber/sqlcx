// Codegen pipeline: load config, parse SQL, run generators, write output.
//
// Note: this module exceeds 100 lines because it represents a single
// cohesive concern (the end-to-end pipeline). Splitting it now would
// scatter related functions for no payoff. Revisit if it grows past
// ~300 LOC or accumulates unrelated responsibilities.

use serde::{Deserialize, Serialize};
use sqlcx_core::{
    cache::{SqlFile, compute_hash, read_cache, write_cache},
    config::{SqlcxConfig, TargetConfig, load_config},
    generator::GeneratedFile,
    generator::resolve_language,
    ir::SqlcxIR,
    parser::resolve_parser,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub fn run_pipeline(write_output: bool) -> sqlcx_core::error::Result<()> {
    let cwd = std::env::current_dir()?;

    eprintln!("Loading config from {}", cwd.display());
    let config = load_config(&cwd)?;

    let sql_dir = resolve_path(&cwd, &config.sql);
    eprintln!("Scanning SQL files in {}", sql_dir.display());

    let all_sql = collect_sql_files(&sql_dir)?;

    let sql_file_structs: Vec<SqlFile> = all_sql
        .iter()
        .map(|p| -> sqlcx_core::error::Result<SqlFile> {
            Ok(SqlFile {
                path: p.to_string_lossy().into_owned(),
                content: std::fs::read_to_string(p)?,
            })
        })
        .collect::<sqlcx_core::error::Result<Vec<_>>>()?;

    let queries_dir = sql_dir.join("queries");
    let (schema_files, query_files) = partition_sql_files(&queries_dir, &sql_file_structs);

    let hash = compute_hash(&sql_file_structs, &config.parser);
    let cache_dir = cwd.join(".sqlcx");

    let ir = if let Some(cached) = read_cache(&cache_dir, &hash)? {
        eprintln!("Cache hit — using cached IR");
        cached
    } else {
        eprintln!("Cache miss — parsing SQL files");
        let ir = build_ir(&config.parser, &schema_files, &query_files)?;
        write_cache(&cache_dir, &ir, &hash)?;
        ir
    };

    let validated_targets = validate_targets(&config)?;

    if write_output {
        let mut outputs_by_dir: BTreeMap<PathBuf, Vec<GeneratedFile>> = BTreeMap::new();

        for (target, merged_target, plugin) in validated_targets {
            let files = plugin.generate(&ir, &merged_target)?;
            let out_dir = resolve_path(&cwd, &target.out);
            outputs_by_dir.entry(out_dir).or_default().extend(files);
        }

        for (out_dir, files) in outputs_by_dir {
            std::fs::create_dir_all(&out_dir)?;
            sync_generated_files(&out_dir, &files)?;
            for file in files {
                let dest = out_dir.join(&file.path);
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                eprintln!("Writing {}", dest.display());
                std::fs::write(&dest, &file.content)?;
            }
        }
        eprintln!("Done.");
    } else {
        eprintln!(
            "Check passed — {} tables, {} queries, {} enums",
            ir.tables.len(),
            ir.queries.len(),
            ir.enums.len()
        );
    }

    Ok(())
}

type ValidatedTarget = (
    TargetConfig,
    TargetConfig,
    Box<dyn sqlcx_core::generator::LanguagePlugin>,
);

fn validate_targets(config: &SqlcxConfig) -> sqlcx_core::error::Result<Vec<ValidatedTarget>> {
    let mut validated = Vec::with_capacity(config.targets.len());
    for target in &config.targets {
        let mut merged_target = target.clone();
        for (k, v) in &config.overrides {
            merged_target
                .overrides
                .entry(k.clone())
                .or_insert(v.clone());
        }
        let plugin = resolve_language(
            &merged_target.language,
            &merged_target.schema,
            &merged_target.driver,
        )?;
        validated.push((target.clone(), merged_target, plugin));
    }
    Ok(validated)
}

#[derive(Serialize, Deserialize, Default)]
struct OutputManifest {
    files: Vec<String>,
}

fn sync_generated_files(
    out_dir: &Path,
    files: &[sqlcx_core::generator::GeneratedFile],
) -> sqlcx_core::error::Result<()> {
    let manifest_path = out_dir.join(".sqlcx-manifest.json");
    let previous = if manifest_path.exists() {
        serde_json::from_str::<OutputManifest>(&std::fs::read_to_string(&manifest_path)?)
            .unwrap_or_default()
    } else {
        OutputManifest::default()
    };

    let current: std::collections::BTreeSet<String> =
        files.iter().map(|file| file.path.clone()).collect();

    for stale in previous.files {
        if !current.contains(&stale) {
            let stale_path = out_dir.join(&stale);
            if stale_path.exists() {
                std::fs::remove_file(&stale_path)?;
            }
        }
    }

    let manifest = OutputManifest {
        files: current.into_iter().collect(),
    };
    std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
    Ok(())
}

fn resolve_path(base: &Path, s: &str) -> PathBuf {
    let p = PathBuf::from(s);
    if p.is_absolute() { p } else { base.join(p) }
}

fn collect_sql_files(sql_dir: &Path) -> sqlcx_core::error::Result<Vec<PathBuf>> {
    let pattern = format!("{}/**/*.sql", sql_dir.display());
    let mut paths = Vec::new();
    for entry in glob::glob(&pattern).map_err(|e| {
        sqlcx_core::error::SqlcxError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            e.to_string(),
        ))
    })? {
        match entry {
            Ok(p) => paths.push(p),
            Err(e) => eprintln!("warning: glob error: {e}"),
        }
    }
    paths.sort();
    Ok(paths)
}

fn partition_sql_files(queries_dir: &Path, files: &[SqlFile]) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut schema = Vec::new();
    let mut queries = Vec::new();
    for f in files {
        let p = PathBuf::from(&f.path);
        let is_in_queries_dir = p.starts_with(queries_dir);
        let has_name_annotation = f.content.contains("-- name:");
        if is_in_queries_dir || has_name_annotation {
            queries.push(p);
        } else {
            schema.push(p);
        }
    }
    (schema, queries)
}

fn build_ir(
    parser_name: &str,
    schema_files: &[PathBuf],
    query_files: &[PathBuf],
) -> sqlcx_core::error::Result<SqlcxIR> {
    let parser = resolve_parser(parser_name)?;

    let mut all_tables = Vec::new();
    let mut all_enums = Vec::new();

    for path in schema_files {
        let sql = std::fs::read_to_string(path)?;
        let (tables, enums) = parser.parse_schema(&sql)?;
        all_tables.extend(tables);
        all_enums.extend(enums);
    }

    let mut all_queries = Vec::new();
    for path in query_files {
        let sql = std::fs::read_to_string(path)?;
        let source = path.to_string_lossy().into_owned();
        let queries = parser.parse_queries(&sql, &all_tables, &all_enums, &source)?;
        all_queries.extend(queries);
    }

    Ok(SqlcxIR {
        tables: all_tables,
        queries: all_queries,
        enums: all_enums,
    })
}
