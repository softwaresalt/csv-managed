use csv_managed::schema::{ColumnMeta, ColumnType, Schema};
use csv_managed::schema_cmd::test_support;

#[test]
fn parse_columns_accepts_comma_and_repeats() {
    let columns = test_support::parse_columns(&["id:integer,name:string", "amount:float"])
        .expect("parsed");
    assert_eq!(columns.len(), 3);
    assert_eq!(columns[0].name, "id");
    assert_eq!(columns[1].name, "name");
    assert_eq!(columns[2].name, "amount");
    assert_eq!(columns[0].datatype, ColumnType::Integer);
    assert_eq!(columns[1].datatype, ColumnType::String);
    assert_eq!(columns[2].datatype, ColumnType::Float);
}

#[test]
fn duplicate_columns_are_rejected() {
    let err = test_support::parse_columns(&["id:integer,id:string"])
        .unwrap_err();
    assert!(err.to_string().contains("Duplicate column name"));
}

#[test]
fn missing_type_is_rejected() {
    let err = test_support::parse_columns(&["id"]).unwrap_err();
    assert!(err.to_string().contains("must use the form"));
}

#[test]
fn parse_columns_supports_output_rename() {
    let columns = test_support::parse_columns(&["id:integer->Identifier,name:string"])
        .expect("parsed");
    assert_eq!(columns.len(), 2);
    assert_eq!(columns[0].rename.as_deref(), Some("Identifier"));
    assert!(columns[1].rename.is_none());
}

#[test]
fn duplicate_output_names_are_rejected() {
    let err = test_support::parse_columns(&["id:integer->Identifier", "code:string->Identifier"])
        .unwrap_err();
    assert!(err
        .to_string()
        .contains("Duplicate output column name"));
}

#[test]
fn replacements_apply_to_columns() {
    let mut columns = test_support::parse_columns(&["status:string"])
        .expect("parsed");
    test_support::apply_replacements(&mut columns, &["status=pending->shipped"])
        .expect("applied");
    assert_eq!(columns[0].value_replacements.len(), 1);
    assert_eq!(columns[0].value_replacements[0].from, "pending");
    assert_eq!(columns[0].value_replacements[0].to, "shipped");
}

#[test]
fn replacements_validate_column_names() {
    let mut columns = test_support::parse_columns(&["status:string"])
        .expect("parsed");
    let err = test_support::apply_replacements(&mut columns, &["missing=pending->shipped"])
        .unwrap_err();
    assert!(err.to_string().contains("unknown column"));
}

#[test]
fn snake_case_conversion_handles_various_tokens() {
    assert_eq!(test_support::to_lower_snake_case("OrderDate"), "order_date");
    assert_eq!(test_support::to_lower_snake_case("customer-name"), "customer_name");
    assert_eq!(test_support::to_lower_snake_case("customer  name"), "customer_name");
    assert_eq!(test_support::to_lower_snake_case("APIKey"), "api_key");
    assert_eq!(test_support::to_lower_snake_case("HTTPStatus"), "http_status");
}

#[test]
fn apply_overrides_updates_types() {
    let mut schema = Schema {
        columns: vec![ColumnMeta {
            name: "amount".to_string(),
            datatype: ColumnType::Float,
            rename: None,
            value_replacements: Vec::new(),
            datatype_mappings: Vec::new(),
        }],
        schema_version: None,
        has_headers: true,
    };

    let applied = test_support::apply_overrides(&mut schema, &["amount:integer", ""])
        .expect("overrides applied");
    assert_eq!(schema.columns[0].datatype, ColumnType::Integer);
    assert!(applied.contains(&"amount".to_string()));
}

#[test]
fn apply_default_name_mappings_returns_suggested_set() {
    let mut schema = Schema {
        columns: vec![
            ColumnMeta {
                name: "OrderID".to_string(),
                datatype: ColumnType::Integer,
                rename: None,
                value_replacements: Vec::new(),
                datatype_mappings: Vec::new(),
            },
            ColumnMeta {
                name: "CustomerName".to_string(),
                datatype: ColumnType::String,
                rename: Some("customer_name".to_string()),
                value_replacements: Vec::new(),
                datatype_mappings: Vec::new(),
            },
        ],
        schema_version: None,
        has_headers: true,
    };

    let suggested = test_support::apply_default_name_mappings(&mut schema);
    assert_eq!(suggested.len(), 1);
    assert_eq!(suggested[0].0, "OrderID");
    assert_eq!(suggested[0].1, "order_id");
    assert_eq!(schema.columns[0].rename.as_deref(), Some("order_id"));
    assert_eq!(schema.columns[1].rename.as_deref(), Some("customer_name"));
}
