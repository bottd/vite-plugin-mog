use insta::assert_yaml_snapshot;
use std::fs;
use vite_plugin_mog_parser::DataFilter;
use vite_plugin_mog_parser::parse_metadata_on_bounded_stack;
use vite_plugin_mog_parser::{
    MogParseResult, OutputMode, Segment, extract_metadata, parse_on_bounded_stack, render,
    segments_html,
};

// The napi export is async; this is the same parse without the promise.
fn parse(content: &str) -> MogParseResult {
    parse_on_bounded_stack(content, None, DataFilter::All).expect("failed to parse mog")
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
        "tests/fixtures/nested-blocks.mg",
        "tests/fixtures/nested-lists.mg",
        "tests/fixtures/tasks.mg",
    ] {
        let content = fs::read_to_string(path).unwrap_or_else(|_| panic!("failed to read {path}"));
        let document = mog_parser::parse(&content);
        let rendered = render(&document, None, DataFilter::All)
            .unwrap_or_else(|_| panic!("failed to render {path}"));
        let metadata = extract_metadata(document.attributes.as_deref());
        let summary = parse_metadata_on_bounded_stack(&content).unwrap();
        assert_eq!(summary.metadata, metadata, "metadata differs for {path}");
        assert_eq!(summary.toc, rendered.toc, "outline differs for {path}");
        assert_yaml_snapshot!(
            path,
            (
                segments_html(&rendered.segments),
                rendered.toc,
                metadata,
                rendered.css
            )
        );
    }
}

#[test]
fn metadata_only_keeps_document_warnings_without_rendering_warnings() {
    let source = "``attr:\ntitle \"First\"\ntitle \"Second\"\n``\n\n# Setup\n# Setup\n\n[[javascript:alert(1)]]\n";
    let summary = parse_metadata_on_bounded_stack(source).unwrap();
    let rendered = parse(source);
    assert_eq!(summary.toc, rendered.toc);
    assert_eq!(summary.metadata, rendered.metadata);
    assert_eq!(summary.diagnostics.len(), 1);
    assert!(summary.diagnostics[0].contains("first value was kept"));
    assert_eq!(rendered.diagnostics.unwrap().len(), 2);
}

#[test]
fn metadata_only_validates_embed_ordinals_without_rendering_components() {
    let source = "``embed:css:\n.foo {}\n``\n\n``embed:bogus:\ncontent\n``\n";
    let error = parse_metadata_on_bounded_stack(source)
        .err()
        .expect("invalid embed");
    assert!(error.contains("embed #2"), "{error}");
    assert!(parse_metadata_on_bounded_stack("``embed:vue:\n<X/>\n``\n").is_ok());
}

#[test]
fn empty_and_single_line_fenced_code_stay_blocks_inside_markers() {
    for body in ["", "one\n", "one\ntwo\n"] {
        let source = format!("-\n``text:\n{body}``\n-\n");
        let html = segments_html(&parse(&source).segments);
        assert!(html.contains("<li><pre>"), "{html}");
    }
}

#[test]
fn inline_embed_at_the_start_of_an_item_remains_code() {
    let result = parse_on_bounded_stack(
        "- ``embed:vue: <X/>``\n",
        Some(OutputMode::svelte),
        DataFilter::All,
    )
    .unwrap();
    assert!(result.embed_components.is_empty());
    assert!(segments_html(&result.segments).contains("<code>&lt;X/&gt;</code>"));
}

#[test]
fn css_embeds_leave_the_markup_whole() {
    let result = parse("``embed:css:\n.test { color: red; }\n``\n");

    assert!(result.embed_components.is_empty());
    assert!(result.embed_css.contains(".test { color: red; }"));
    assert!(result.segments.is_empty(), "{:?}", result.segments);
}

#[test]
fn embed_component_indexes_ignore_css_declarations() {
    let content = "``embed:css:\n.foo {}\n``\n\n``embed:svelte:\n<div>one</div>\n``\n\n``embed:svelte:\n<div>two</div>\n``\n";
    let result =
        parse_on_bounded_stack(content, Some(OutputMode::svelte), DataFilter::All).unwrap();

    let indexes: Vec<_> = result
        .embed_components
        .iter()
        .map(|embed| embed.index)
        .collect();
    assert_eq!(indexes, [0, 1]);
    assert_eq!(
        result.segments,
        [Segment::Embed { index: 0 }, Segment::Embed { index: 1 }]
    );
}

#[test]
fn embed_errors_report_the_declaration_ordinal() {
    let content = "``embed:css:\n.foo {}\n``\n\n``embed:bogus:\ncontent\n``\n";
    let message = match parse_on_bounded_stack(content, Some(OutputMode::html), DataFilter::All) {
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
    assert!(result.segments.is_empty(), "{:?}", result.segments);
}

#[test]
fn deep_nesting_parses_on_the_bounded_stack() {
    let content: String = (1..=200)
        .map(|level| format!("{} item\n", "-".repeat(level)))
        .collect();

    assert!(!parse(&content).segments.is_empty());
}

#[test]
fn heading_levels_and_empty_ids_stay_valid() {
    let result = parse("####### Deep heading\n\n# ***\n");
    let html = segments_html(&result.segments);

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
    let html = segments_html(&result.segments);
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

/// `dist/parser/index.d.ts` is written by hand, because the tree crosses the
/// boundary as JSON and there is no napi object for the type generator to
/// describe. This fixture is what ties the two together: the TypeScript test
/// walks the same document and checks every node against the declared shape.
///
/// So the fixture has to keep exercising everything. Adding a variant to
/// `NodeKind`, `MarkerKind`, `Delimiter` or `Value` breaks the exhaustive
/// matches in `src/parser/ast.rs`; when you fix those, add the new spelling
/// here and to `tests/types/parser.ts`, and give the fixture something that
/// produces it.
#[test]
fn the_variant_fixture_still_exercises_every_spelling_the_ast_can_emit() {
    let content = std::fs::read_to_string("tests/fixtures/ast-variants.mg").unwrap();
    let json = vite_plugin_mog_parser::parse_ast_json(&content, true, true).unwrap();

    let expected = [
        (
            "kind",
            [
                "marker",
                "delimiter",
                "raw",
                "link",
                "attributes",
                "paragraph",
                "table",
                "text",
            ]
            .as_slice(),
        ),
        (
            "marker",
            [
                "heading",
                "unordered-list",
                "ordered-list",
                "blockquote",
                "free",
            ]
            .as_slice(),
        ),
        (
            "delimiter",
            [
                "strong",
                "italic",
                "verbatim",
                "strikethrough",
                "table-header",
                "table-row",
                "table-cell",
                "footnote",
                "link",
                "link-name",
            ]
            .as_slice(),
        ),
        (
            "kind",
            ["null", "bool", "int", "float", "string", "node"].as_slice(),
        ),
    ];

    for (field, names) in expected {
        for name in names {
            assert!(
                json.contains(&format!("\"{field}\":\"{name}\"")),
                "the fixture no longer produces {field} {name:?}"
            );
        }
    }
}
