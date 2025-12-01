use csv_managed::index::{CsvIndex, IndexDefinition, SortDirection};
use csv_managed::schema::{ColumnMeta, ColumnType, DecimalSpec, Schema};
use encoding_rs::UTF_8;
use std::fs;
use tempfile::tempdir;

#[test]
fn parse_index_spec_supports_mixed_directions() {
    let spec = IndexDefinition::parse("col1:desc,col2:asc,col3").unwrap();
    assert_eq!(spec.columns, vec!["col1", "col2", "col3"]);
    assert_eq!(
        spec.directions,
        vec![SortDirection::Desc, SortDirection::Asc, SortDirection::Asc]
    );
    assert!(spec.name.is_none());
}

#[test]
fn parse_index_spec_supports_named_variants() {
    let spec = IndexDefinition::parse("top=col1:desc,col2").unwrap();
    assert_eq!(spec.name.as_deref(), Some("top"));
    assert_eq!(spec.columns, vec!["col1", "col2"]);
    assert_eq!(
        spec.directions,
        vec![SortDirection::Desc, SortDirection::Asc]
    );
}

#[test]
fn parse_index_spec_requires_column_name() {
    let err = IndexDefinition::parse("col1,,col2").expect_err("spec with missing column should fail");
    assert!(err.to_string().contains("missing a column name"));
}

#[test]
fn parse_index_spec_rejects_unknown_direction() {
    let err = IndexDefinition::parse("col1:sideways").expect_err("unknown direction should fail");
    assert!(err.to_string().contains("Unknown sort direction"));
}

#[test]
fn index_definition_from_columns_rejects_empty() {
    let err = IndexDefinition::from_columns(vec![" ".to_string()])
        .expect_err("empty column list should fail");
    assert!(err.to_string().contains("At least one column"));
}

#[test]
fn expand_covering_spec_generates_prefix_variants() {
    let variants = IndexDefinition::expand_covering_spec("col1:asc|desc,col2:asc").unwrap();
    assert_eq!(variants.len(), 4);
    let coverings: Vec<(Vec<String>, Vec<SortDirection>, String)> = variants
        .into_iter()
        .map(|definition| {
            (
                definition.columns,
                definition.directions,
                definition.name.unwrap(),
            )
        })
        .collect();
    assert!(coverings.iter().any(|(cols, dirs, _)| {
        cols == &vec!["col1".to_string()] && dirs == &vec![SortDirection::Asc]
    }));
    assert!(coverings.iter().any(|(cols, dirs, _)| {
        cols == &vec!["col1".to_string()] && dirs == &vec![SortDirection::Desc]
    }));
    assert!(coverings.iter().any(|(cols, dirs, name)| {
        cols == &vec!["col1".to_string(), "col2".to_string()]
            && dirs == &vec![SortDirection::Asc, SortDirection::Asc]
            && name.contains("col1-asc")
    }));
}

#[test]
fn save_and_load_index_with_decimal_column() {
    let temp = tempdir().expect("temp dir");
    let csv_path = temp.path().join("decimal.csv");
    fs::write(&csv_path, "id,amount\n1,42.50\n2,13.37\n").expect("write csv");

    let schema = Schema {
        columns: vec![
            ColumnMeta {
                name: "id".to_string(),
                datatype: ColumnType::Integer,
                rename: None,
                value_replacements: Vec::new(),
                datatype_mappings: Vec::new(),
            },
            ColumnMeta {
                name: "amount".to_string(),
                datatype: ColumnType::Decimal(DecimalSpec::new(4, 2).expect("valid decimal spec")),
                rename: None,
                value_replacements: Vec::new(),
                datatype_mappings: Vec::new(),
            },
        ],
        schema_version: None,
        has_headers: true,
    };

    let definition = IndexDefinition::from_columns(vec!["amount".to_string()]).unwrap();
    let index = CsvIndex::build(&csv_path, &[definition], Some(&schema), None, b',', UTF_8)
        .expect("build index");

    let index_path = temp.path().join("decimal.idx");
    index.save(&index_path).expect("save index");

    let loaded = CsvIndex::load(&index_path).expect("load index");
    assert_eq!(loaded.variants().len(), index.variants().len());
    assert_eq!(loaded.row_count(), index.row_count());
}

#[test]
fn expand_covering_spec_honors_name_prefix() {
    let variants =
        IndexDefinition::expand_covering_spec("geo=country:asc|desc,region:asc|desc").unwrap();
    assert!(variants.len() >= 4);
    for definition in variants {
        let name = definition.name.unwrap();
        assert!(name.starts_with("geo_"));
        assert_eq!(definition.columns[0], "country");
    }
}

#[test]
fn build_multiple_variants_and_match() {
    let dir = tempdir().unwrap();
    let csv_path = dir.path().join("data.csv");
    std::fs::write(&csv_path, "a,b,c\n1,x,alpha\n2,y,beta\n3,z,gamma\n").unwrap();

    let definitions = vec![
        IndexDefinition::from_columns(vec!["a".to_string()]).unwrap(),
        IndexDefinition::parse("descending=a:desc,b:asc").unwrap(),
    ];

    let index = CsvIndex::build(&csv_path, &definitions, None, None, b',', UTF_8).unwrap();

    assert_eq!(index.variants().len(), 2);

    let asc_match = index
        .best_match(&[("a".to_string(), SortDirection::Asc)])
        .unwrap();
    assert_eq!(
        asc_match
            .columns()
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>(),
        vec!["a"]
    );

    let desc_match = index
        .best_match(&[
            ("a".to_string(), SortDirection::Desc),
            ("b".to_string(), SortDirection::Asc),
        ])
        .unwrap();
    assert_eq!(desc_match.name(), Some("descending"));
    assert_eq!(
        desc_match
            .columns()
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "b"]
    );

    let offsets: Vec<u64> = desc_match.ordered_offsets().collect();
    assert_eq!(offsets.len(), 3);
    assert!(offsets[0] > offsets[2]);
}
