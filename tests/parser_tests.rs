use insta::assert_yaml_snapshot;
use std::fs;
use vite_plugin_mog_parser::{MogParseResult, extract_metadata, parse_on_bounded_stack, render};

// The napi export is async; this is the same parse without the promise.
fn parse(content: &str) -> MogParseResult {
    parse_on_bounded_stack(content, None).expect("failed to parse mog")
}

#[test]
fn fixture_files_render() {
    for path in [
        "tests/fixtures/basic.mg",
        "tests/fixtures/blocks.mg",
        "tests/fixtures/code-blocks.mg",
        "tests/fixtures/embed-css.mg",
        "tests/fixtures/headings.mg",
        "tests/fixtures/images.mg",
        "tests/fixtures/links.mg",
        "tests/fixtures/nested-lists.mg",
        "tests/fixtures/tasks.mg",
    ] {
        let content = fs::read_to_string(path).unwrap_or_else(|_| panic!("failed to read {path}"));
        let document = mog_parser::parse(&content);
        let rendered =
            render(&document, None).unwrap_or_else(|_| panic!("failed to render {path}"));
        let metadata = extract_metadata(document.meta.as_deref());
        assert_yaml_snapshot!(
            path,
            (
                rendered.parts.join(""),
                rendered.toc,
                metadata,
                rendered.css
            )
        );
    }
}

#[test]
fn css_embeds_do_not_split_parts() {
    let result = parse("``embed:css:\n.test { color: red; }\n``\n");

    assert!(result.embed_components.is_empty());
    assert!(result.embed_css.contains(".test { color: red; }"));
    assert_eq!(result.html_parts.len(), 1);
}

#[test]
fn embed_component_indexes_ignore_css_declarations() {
    let content = "``embed:css:\n.foo {}\n``\n\n``embed:svelte:\n<div>one</div>\n``\n\n``embed:svelte:\n<div>two</div>\n``\n";
    let result = parse_on_bounded_stack(content, Some("svelte")).unwrap();

    let indexes: Vec<_> = result
        .embed_components
        .iter()
        .map(|embed| embed.index)
        .collect();
    assert_eq!(indexes, [0, 1]);
    // One part before each embed, plus the tail.
    assert_eq!(result.html_parts.len(), 3);
}

#[test]
fn embed_errors_report_the_declaration_ordinal() {
    let content = "``embed:css:\n.foo {}\n``\n\n``embed:bogus:\ncontent\n``\n";
    let message = match parse_on_bounded_stack(content, Some("html")) {
        Ok(_) => panic!("expected an embed error"),
        Err(error) => error,
    };

    assert!(message.contains("embed #2"), "{message}");
    assert!(
        message.contains("Offending line: ``embed:bogus:"),
        "{message}"
    );
}

#[test]
fn embeds_are_skipped_without_a_mode() {
    let result = parse("``embed:svelte:\n<div>one</div>\n``\n");

    assert!(result.embed_components.is_empty());
    assert_eq!(result.html_parts.len(), 1);
}

#[test]
fn deep_nesting_parses_on_the_bounded_stack() {
    let content: String = (1..=200)
        .map(|level| format!("{} item\n", "-".repeat(level)))
        .collect();

    assert!(!parse(&content).html_parts.is_empty());
}

#[test]
fn heading_levels_and_empty_ids_stay_valid() {
    let result = parse("####### Deep heading\n\n# ***\n");
    let html = result.html_parts.concat();

    assert!(
        html.contains(r#"<h6 id="deep-heading">Deep heading</h6>"#),
        "{html}"
    );
    assert!(!html.contains("<h7"), "{html}");
    assert!(!html.contains(r#"id="""#), "{html}");
    assert_eq!(result.toc.len(), 1);
    assert_eq!(result.toc[0].level, 6);
}

#[test]
fn duplicate_heading_titles_get_distinct_ids() {
    let result = parse("# Setup\n\n# Setup\n\n# Setup 1\n");
    let ids: Vec<_> = result.toc.iter().map(|entry| entry.id.as_str()).collect();

    assert_eq!(ids, ["setup", "setup-1", "setup-1-1"]);
    let html = result.html_parts.concat();
    for id in ids {
        assert!(html.contains(&format!(r#"id="{id}""#)), "{html}");
    }
}

#[test]
fn each_unsafe_link_emits_one_diagnostic() {
    let result = parse("[[javascript:alert(1)]]((First))\n\n[[javascript:alert(1)]]((Second))\n");
    let diagnostics = result.diagnostics.unwrap_or_default();

    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    assert!(
        diagnostics[0].contains("unsafe URL scheme"),
        "{diagnostics:?}"
    );
}
