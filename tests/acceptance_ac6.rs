//! AC6 (MUST): Every core class has a BFO parent IRI and a non-empty definition annotation.

/// Verify that every [[classes]] block in spec-core has `parent` and `definition` fields.
#[test]
fn acceptance_ac6_all_classes_have_bfo_parent_and_definition() {
    let dir = std::path::Path::new("spec-core");
    let mut violations: Vec<String> = Vec::new();

    let entries = std::fs::read_dir(dir).expect("spec-core/ must exist");
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        // Skip ontology.toml — it declares object_properties, not classes.
        if path.file_name().and_then(|n| n.to_str()) == Some("ontology.toml") {
            continue;
        }
        let content =
            std::fs::read_to_string(&path).expect("toml file must be readable");
        check_classes_in_file(&path.to_string_lossy(), &content, &mut violations);
    }

    assert!(
        violations.is_empty(),
        "AC6 failures — classes missing BFO parent or definition:\n{}",
        violations.join("\n")
    );
}

fn check_classes_in_file(filename: &str, content: &str, violations: &mut Vec<String>) {
    // Each [[classes]] block starts at "[[classes]]" and ends at the next "[[" or EOF.
    let blocks = split_class_blocks(content);
    for (i, block) in blocks.iter().enumerate() {
        let iri = extract_field(block, "iri").unwrap_or_default();
        let has_parent = extract_field(block, "parent").is_some_and(|p| !p.is_empty());
        let has_definition =
            extract_field(block, "definition").is_some_and(|d| !d.is_empty());

        if !has_parent {
            violations.push(format!(
                "{filename} class block #{i} (iri={iri:?}): missing or empty `parent`"
            ));
        }
        if !has_definition {
            violations.push(format!(
                "{filename} class block #{i} (iri={iri:?}): missing or empty `definition`"
            ));
        }
    }
}

fn split_class_blocks(content: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for line in content.lines() {
        if line.trim() == "[[classes]]" {
            if let Some(block) = current.take() {
                blocks.push(block);
            }
            current = Some(String::new());
        } else if let Some(ref mut block) = current {
            block.push_str(line);
            block.push('\n');
        }
    }
    if let Some(block) = current {
        blocks.push(block);
    }
    blocks
}

fn extract_field<'a>(block: &'a str, key: &str) -> Option<&'a str> {
    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(key) && trimmed.contains('=') {
            // Extract value after '=' and strip surrounding whitespace + quotes.
            let val = trimmed.splitn(2, '=').nth(1)?.trim();
            // Remove wrapping quotes if present.
            let val = val.trim_matches('"');
            return Some(val);
        }
    }
    None
}
