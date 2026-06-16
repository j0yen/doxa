//! doxa library — framework parsing and OWL generation for the moral `TBox`.
//!
//! This module exposes the `Framework` type, framework TOML parsing, and
//! `build_framework_owlxml` which combines the core `TBox` with a framework's
//! axioms into a single OWL/XML string.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

// ---------------------------------------------------------------------------
// Core spec types (lightweight — used in tests and framework builder)
// ---------------------------------------------------------------------------

/// A single moral class from the core `TBox`.
#[derive(Debug, Clone)]
pub struct MoralClass {
    /// Local IRI (e.g. `"RightAction"`).
    pub iri: String,
    /// Human-readable label.
    pub label: String,
    /// Parent class IRI.
    pub parent: String,
    /// Formal definition.
    pub definition: String,
}

/// An object property from the core `TBox`.
#[derive(Debug, Clone)]
pub struct ObjectProperty {
    /// Local IRI (e.g. `"hasConsequence"`).
    pub iri: String,
    /// Human-readable label.
    pub label: String,
}

/// The parsed moral spec (core `TBox`).
#[derive(Debug, Default, Clone)]
pub struct MoralSpec {
    /// All declared moral classes.
    pub classes: Vec<MoralClass>,
    /// All declared object properties.
    pub properties: Vec<ObjectProperty>,
    /// Ontology IRI from `ontology.toml`.
    pub ontology_iri: String,
}

// ---------------------------------------------------------------------------
// Framework types
// ---------------------------------------------------------------------------

/// A single axiom within a normative framework.
#[derive(Debug, Clone)]
pub struct FrameworkAxiom {
    /// Kind of axiom — currently `"subclass"` (`SubClassOf` in OWL).
    pub kind: String,
    /// The subject class (e.g. `"RightAction"`).
    pub subject: String,
    /// The Manchester-syntax condition string (e.g. `"Action and (maximizes some Wellbeing)"`).
    pub condition: String,
}

/// A parsed normative framework module.
#[derive(Debug, Clone)]
pub struct Framework {
    /// Machine-readable name (e.g. `"consequentialism"`).
    pub name: String,
    /// Human-readable label.
    pub label: String,
    /// One-sentence thesis / description.
    pub description: String,
    /// Philosophical tradition / author.
    pub author: String,
    /// Philosophical grounding annotation (cites primary source).
    pub philosophical_grounding: String,
    /// The defining axioms.
    pub axioms: Vec<FrameworkAxiom>,
}

// ---------------------------------------------------------------------------
// Framework parsing
// ---------------------------------------------------------------------------

/// Parse a single framework TOML file into a [`Framework`].
///
/// # Errors
/// Returns an error if the file cannot be read or is missing required fields.
pub fn parse_framework(path: &Path) -> Result<Framework> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read framework TOML: {}", path.display()))?;

    let table: toml::Table = toml::from_str(&content)
        .with_context(|| format!("failed to parse TOML: {}", path.display()))?;

    let fw = table
        .get("framework")
        .and_then(toml::Value::as_table)
        .with_context(|| {
            format!(
                "missing [framework] section in: {}",
                path.display()
            )
        })?;

    let name = fw
        .get("name")
        .and_then(toml::Value::as_str)
        .unwrap_or("")
        .to_owned();
    let label = fw
        .get("label")
        .and_then(toml::Value::as_str)
        .unwrap_or("")
        .to_owned();
    let description = fw
        .get("description")
        .and_then(toml::Value::as_str)
        .unwrap_or("")
        .to_owned();
    let author = fw
        .get("author")
        .and_then(toml::Value::as_str)
        .unwrap_or("")
        .to_owned();
    let philosophical_grounding = fw
        .get("philosophicalGrounding")
        .and_then(toml::Value::as_str)
        .unwrap_or("")
        .to_owned();

    let axioms = table
        .get("axioms")
        .and_then(toml::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    let t = v.as_table()?;
                    Some(FrameworkAxiom {
                        kind: t
                            .get("kind")
                            .and_then(toml::Value::as_str)
                            .unwrap_or("subclass")
                            .to_owned(),
                        subject: t
                            .get("subject")
                            .and_then(toml::Value::as_str)
                            .unwrap_or("")
                            .to_owned(),
                        condition: t
                            .get("condition")
                            .and_then(toml::Value::as_str)
                            .unwrap_or("")
                            .to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(Framework {
        name,
        label,
        description,
        author,
        philosophical_grounding,
        axioms,
    })
}

/// Scan a `spec-frameworks/` directory and return all parsed [`Framework`]s.
///
/// # Errors
/// Returns an error if the directory cannot be read. Individual TOML parse
/// errors are propagated.
pub fn list_frameworks(dir: &Path) -> Result<Vec<Framework>> {
    let mut frameworks = Vec::new();
    let entries = fs::read_dir(dir)
        .with_context(|| format!("failed to read frameworks directory: {}", dir.display()))?;

    let mut paths: Vec<std::path::PathBuf> = entries
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("toml"))
        .collect();
    paths.sort(); // deterministic order

    for path in paths {
        let fw = parse_framework(&path)?;
        frameworks.push(fw);
    }

    Ok(frameworks)
}

// ---------------------------------------------------------------------------
// Core spec parsing (lightweight TOML scanner, not a full ousia-forge parse)
// ---------------------------------------------------------------------------

/// Parse the core moral spec from TOML files in `dir`.
///
/// This is a best-effort scanner sufficient for framework OWL generation and
/// tests; it does not replicate the full ousia-forge validator.
///
/// # Errors
/// Returns an error if the directory cannot be read.
pub fn parse_spec_dir(dir: &Path) -> Result<MoralSpec> {
    let mut spec = MoralSpec::default();

    let entries = fs::read_dir(dir)
        .with_context(|| format!("failed to read spec directory: {}", dir.display()))?;

    let mut paths: Vec<std::path::PathBuf> = entries
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("toml"))
        .collect();
    paths.sort();

    for path in &paths {
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;

        let table: toml::Table = toml::from_str(&content)
            .with_context(|| format!("failed to parse TOML: {}", path.display()))?;

        // ontology.toml: grab iri + object_properties
        if path.file_name().and_then(|n| n.to_str()) == Some("ontology.toml") {
            if let Some(iri) = table.get("iri").and_then(toml::Value::as_str) {
                iri.clone_into(&mut spec.ontology_iri);
            }
            if let Some(props) = table.get("object_properties").and_then(toml::Value::as_array) {
                for p in props {
                    if let Some(t) = p.as_table() {
                        let iri = t
                            .get("iri")
                            .and_then(toml::Value::as_str)
                            .unwrap_or("")
                            .to_owned();
                        let label = t
                            .get("label")
                            .and_then(toml::Value::as_str)
                            .unwrap_or("")
                            .to_owned();
                        spec.properties.push(ObjectProperty { iri, label });
                    }
                }
            }
            continue;
        }

        // Domain files: grab [[classes]]
        if let Some(classes) = table.get("classes").and_then(toml::Value::as_array) {
            for c in classes {
                if let Some(t) = c.as_table() {
                    let iri = t
                        .get("iri")
                        .and_then(toml::Value::as_str)
                        .unwrap_or("")
                        .to_owned();
                    let label = t
                        .get("label")
                        .and_then(toml::Value::as_str)
                        .unwrap_or(&iri)
                        .to_owned();
                    let parent = t
                        .get("parent")
                        .and_then(toml::Value::as_str)
                        .unwrap_or("")
                        .to_owned();
                    let definition = t
                        .get("definition")
                        .and_then(toml::Value::as_str)
                        .unwrap_or("")
                        .to_owned();
                    spec.classes.push(MoralClass {
                        iri,
                        label,
                        parent,
                        definition,
                    });
                }
            }
        }
    }

    Ok(spec)
}

// ---------------------------------------------------------------------------
// OWL/XML generation
// ---------------------------------------------------------------------------

/// Emit a minimal OWL/XML ontology for the core `TBox` alone.
///
/// Used by `doxa build-core` when ousia-forge is absent (tests).
#[must_use]
pub fn emit_owlxml(spec: &MoralSpec) -> String {
    let base = "https://w3id.org/doxa/moral-core";
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\"?>\n");
    out.push_str("<Ontology xmlns=\"http://www.w3.org/2002/07/owl#\"\n");
    out.push_str(&format!("  ontologyIRI=\"{base}\"\n"));
    out.push_str("  xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"\n");
    out.push_str("  xmlns:rdfs=\"http://www.w3.org/2000/01/rdf-schema#\">\n\n");

    for cls in &spec.classes {
        out.push_str(&format!(
            "  <Declaration><Class IRI=\"#{}\"/></Declaration>\n",
            cls.iri
        ));
    }
    out.push('\n');
    for cls in &spec.classes {
        if !cls.parent.is_empty() {
            out.push_str(&format!(
                "  <SubClassOf><Class IRI=\"#{}\"/><Class IRI=\"{}\"/></SubClassOf>\n",
                cls.iri, cls.parent
            ));
        }
    }
    out.push_str("</Ontology>\n");
    out
}

/// Build an OWL/XML ontology combining the core `TBox` with a framework's axioms.
///
/// The result is a valid (though minimal) OWL/XML document that:
/// - declares all core classes and object properties
/// - encodes each framework axiom as a `SubClassOf` expression
/// - includes a `philosophicalGrounding` annotation on the ontology
#[must_use]
pub fn build_framework_owlxml(spec: &MoralSpec, framework: &Framework) -> String {
    let fw_iri = format!("https://w3id.org/doxa/{}", framework.name);
    let mut out = String::new();

    out.push_str("<?xml version=\"1.0\"?>\n");
    out.push_str("<Ontology xmlns=\"http://www.w3.org/2002/07/owl#\"\n");
    out.push_str(&format!("  ontologyIRI=\"{fw_iri}\"\n"));
    out.push_str("  xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"\n");
    out.push_str("  xmlns:rdfs=\"http://www.w3.org/2000/01/rdf-schema#\"\n");
    out.push_str("  xmlns:doxa=\"https://w3id.org/doxa#\">\n\n");

    // Ontology-level annotations
    out.push_str("  <Annotation>\n");
    out.push_str("    <AnnotationProperty abbreviatedIRI=\"rdfs:label\"/>\n");
    out.push_str(&format!(
        "    <Literal>{}</Literal>\n",
        xml_escape(&framework.label)
    ));
    out.push_str("  </Annotation>\n");

    out.push_str("  <Annotation>\n");
    out.push_str("    <AnnotationProperty IRI=\"https://w3id.org/doxa/philosophicalGrounding\"/>\n");
    out.push_str(&format!(
        "    <Literal>{}</Literal>\n",
        xml_escape(&framework.philosophical_grounding)
    ));
    out.push_str("  </Annotation>\n\n");

    // Import core TBox
    out.push_str("  <Import>https://w3id.org/doxa/moral-core</Import>\n\n");

    // Declare core classes
    for cls in &spec.classes {
        out.push_str(&format!(
            "  <Declaration><Class IRI=\"#{}\"/></Declaration>\n",
            cls.iri
        ));
    }

    // Declare object properties
    for prop in &spec.properties {
        out.push_str(&format!(
            "  <Declaration><ObjectProperty IRI=\"{}\"/></Declaration>\n",
            prop.iri
        ));
    }
    out.push('\n');

    // Core SubClassOf axioms
    for cls in &spec.classes {
        if !cls.parent.is_empty() {
            out.push_str(&format!(
                "  <SubClassOf><Class IRI=\"#{}\"/><Class IRI=\"{}\"/></SubClassOf>\n",
                cls.iri, cls.parent
            ));
        }
    }
    out.push('\n');

    // Framework axioms
    out.push_str(&format!(
        "  <!-- Framework: {} — {} -->\n",
        framework.name,
        xml_escape(&framework.description)
    ));
    for axiom in &framework.axioms {
        match axiom.kind.as_str() {
            "subclass" => {
                out.push_str("  <SubClassOf>\n");
                out.push_str(&format!(
                    "    <Class IRI=\"#{}\"/>\n",
                    axiom.subject
                ));
                out.push_str(&format!(
                    "    <!-- condition: {} -->\n",
                    xml_escape(&axiom.condition)
                ));
                // Encode the condition as a comment + annotation for now;
                // full Manchester→OWL translation requires a parser beyond scope.
                out.push_str(&format!(
                    "    <Class IRI=\"#{}__{}__condition\"/>\n",
                    axiom.subject,
                    sanitize_iri(&axiom.condition)
                ));
                out.push_str("  </SubClassOf>\n");
            }
            other => {
                out.push_str(&format!(
                    "  <!-- unsupported axiom kind: {other} — subject: {} -->\n",
                    axiom.subject
                ));
            }
        }
    }
    out.push_str("</Ontology>\n");
    out
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn sanitize_iri(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn list_frameworks_returns_at_least_three() {
        let dir = Path::new("spec-frameworks");
        let frameworks = list_frameworks(dir)
            .expect("list_frameworks must succeed with spec-frameworks/ present");
        assert!(
            frameworks.len() >= 3,
            "expected ≥3 frameworks; found {}",
            frameworks.len()
        );
    }

    #[test]
    fn each_framework_has_at_least_two_axioms() {
        let dir = Path::new("spec-frameworks");
        let frameworks =
            list_frameworks(dir).expect("list_frameworks must succeed");
        for fw in &frameworks {
            assert!(
                fw.axioms.len() >= 2,
                "framework '{}' must have ≥2 axioms; found {}",
                fw.name,
                fw.axioms.len()
            );
        }
    }

    #[test]
    fn build_framework_owlxml_includes_core_and_framework_axioms() {
        let spec_dir = Path::new("spec-core");
        let fw_dir = Path::new("spec-frameworks");

        let spec = parse_spec_dir(spec_dir).expect("parse_spec_dir must succeed");
        let frameworks = list_frameworks(fw_dir).expect("list_frameworks must succeed");

        assert!(
            !frameworks.is_empty(),
            "must have at least one framework to test"
        );

        let fw = &frameworks[0];
        let owl = build_framework_owlxml(&spec, fw);

        // Must contain core TBox classes
        assert!(
            owl.contains("RightAction"),
            "OWL output must include RightAction from core TBox"
        );
        assert!(
            owl.contains("WrongAction"),
            "OWL output must include WrongAction from core TBox"
        );

        // Must contain framework-specific content
        assert!(
            owl.contains(&fw.name),
            "OWL output must reference the framework name '{}'",
            fw.name
        );
        assert!(
            !fw.axioms.is_empty(),
            "framework must have axioms"
        );
        let first_subject = &fw.axioms[0].subject;
        assert!(
            owl.contains(first_subject.as_str()),
            "OWL output must reference the first axiom subject '{first_subject}'"
        );
    }

    #[test]
    fn consequentialism_and_deontology_produce_different_owl() {
        let spec_dir = Path::new("spec-core");
        let spec = parse_spec_dir(spec_dir).expect("parse_spec_dir must succeed");

        let cons_path = Path::new("spec-frameworks/consequentialism.toml");
        let deon_path = Path::new("spec-frameworks/deontology.toml");

        let cons = parse_framework(cons_path).expect("parse consequentialism");
        let deon = parse_framework(deon_path).expect("parse deontology");

        let owl_cons = build_framework_owlxml(&spec, &cons);
        let owl_deon = build_framework_owlxml(&spec, &deon);

        assert_ne!(
            owl_cons, owl_deon,
            "consequentialism and deontology must produce structurally different OWL outputs"
        );

        // Consequentialism references Wellbeing/maximizes; deontology does NOT
        assert!(
            owl_cons.contains("Wellbeing") || owl_cons.contains("maximizes"),
            "consequentialism OWL must reference Wellbeing or maximizes"
        );
        assert!(
            owl_deon.contains("Maxim") || owl_deon.contains("instantiatesMaxim") || owl_deon.contains("violates"),
            "deontology OWL must reference Maxim/instantiatesMaxim or violates"
        );
    }

    #[test]
    fn frameworks_have_non_empty_descriptions() {
        let dir = Path::new("spec-frameworks");
        let frameworks =
            list_frameworks(dir).expect("list_frameworks must succeed");
        for fw in &frameworks {
            assert!(
                !fw.description.is_empty(),
                "framework '{}' must have a non-empty description",
                fw.name
            );
            assert!(
                !fw.label.is_empty(),
                "framework '{}' must have a non-empty label",
                fw.name
            );
        }
    }

    #[test]
    fn philosophical_groundings_cite_primary_sources() {
        let dir = Path::new("spec-frameworks");
        let frameworks =
            list_frameworks(dir).expect("list_frameworks must succeed");

        let cons = frameworks.iter().find(|f| f.name == "consequentialism")
            .expect("consequentialism must be present");
        let deon = frameworks.iter().find(|f| f.name == "deontology")
            .expect("deontology must be present");
        let virt = frameworks.iter().find(|f| f.name == "virtue-ethics")
            .expect("virtue-ethics must be present");

        assert!(
            cons.philosophical_grounding.contains("Mill"),
            "consequentialism grounding must cite Mill"
        );
        assert!(
            deon.philosophical_grounding.contains("Kant"),
            "deontology grounding must cite Kant"
        );
        assert!(
            virt.philosophical_grounding.contains("Aristotle"),
            "virtue-ethics grounding must cite Aristotle"
        );
    }
}
