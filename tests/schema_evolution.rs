use csv_managed::schema::evolution::{SchemaChangeKind, SchemaEvolution};
use csv_managed::schema::{ColumnMeta, ColumnType, Schema, ValueReplacement};

#[test]
fn diff_detects_add_remove_and_datatype_change() {
    let previous = Schema {
        columns: vec![ColumnMeta {
            name: "id".to_string(),
            datatype: ColumnType::Integer,
            rename: None,
            value_replacements: Vec::new(),
            datatype_mappings: Vec::new(),
        }],
        schema_version: None,
        has_headers: true,
    };

    let current = Schema {
        columns: vec![
            ColumnMeta {
                name: "id".to_string(),
                datatype: ColumnType::String,
                rename: None,
                value_replacements: Vec::new(),
                datatype_mappings: Vec::new(),
            },
            ColumnMeta {
                name: "status".to_string(),
                datatype: ColumnType::String,
                rename: None,
                value_replacements: Vec::new(),
                datatype_mappings: Vec::new(),
            },
        ],
        schema_version: None,
        has_headers: true,
    };

    let evolution = SchemaEvolution::diff(&previous, &current);
    assert!(evolution.changes.iter().any(|change| matches!(
        change,
        csv_managed::schema::evolution::SchemaChange {
            column,
            change: SchemaChangeKind::DatatypeChanged { .. }
        } if column == "id"
    )));
    assert!(evolution.changes.iter().any(|change| matches!(
        change,
        csv_managed::schema::evolution::SchemaChange {
            column,
            change: SchemaChangeKind::ColumnAdded
        } if column == "status"
    )));
}

#[test]
fn diff_detects_value_replacement_additions() {
    let previous = Schema {
        columns: vec![ColumnMeta {
            name: "status".to_string(),
            datatype: ColumnType::String,
            rename: None,
            value_replacements: Vec::new(),
            datatype_mappings: Vec::new(),
        }],
        schema_version: None,
        has_headers: true,
    };

    let current = Schema {
        columns: vec![ColumnMeta {
            name: "status".to_string(),
            datatype: ColumnType::String,
            rename: None,
            value_replacements: vec![ValueReplacement {
                from: "pending".to_string(),
                to: "awaiting".to_string(),
            }],
            datatype_mappings: Vec::new(),
        }],
        schema_version: None,
        has_headers: true,
    };

    let evolution = SchemaEvolution::diff(&previous, &current);
    assert!(evolution.changes.iter().any(|change| matches!(
        change,
        csv_managed::schema::evolution::SchemaChange {
            change: SchemaChangeKind::ReplaceMappingAdded { from_value, to_value },
            ..
        } if from_value == "pending" && to_value == "awaiting"
    )));
}
