//! `GreptimeDB` wide-metrics grpc sink for vector.
//!
//! This sink writes Vector's metric data into GreptimeDB using a wide-table
//! schema model. Unlike the standard `greptimedb_metrics` sink which stores
//! all metric tags as STRING TAGs, this sink treats most attributes as
//! FLOAT64 FIELDs, keeping only an explicit whitelist as TAGs.
//!
//! This is useful when OTLP metrics carry many numeric telemetry attributes
//! that should be queryable and aggregatable as GreptimeDB FIELD columns.
//!
//! Schema rules:
//! - Table name: `{namespace}_{metric_name}` (same as metrics sink)
//! - Timestamp: stored as `ts` (or `greptime_timestamp` with new_naming)
//! - Whitelisted tags: stored as STRING TAGs
//! - All other attributes: parsed as f64 and stored as FLOAT64 FIELDs
//! - Metric value (counter/gauge): stored in `val` (or `greptime_value`)
//! - Distribution/Histogram/Summary/Sket  ch: statistical fields same as metrics sink

mod batch;
mod config;
#[cfg(all(test, feature = "greptimedb-integration-tests"))]
mod integration_tests;
mod request;
mod request_builder;
mod service;
mod sink;
