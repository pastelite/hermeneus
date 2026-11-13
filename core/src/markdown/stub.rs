use regex::Regex;
use std::{collections::HashMap, sync::OnceLock};

/// Templates with simple bodies can stay inline in the Markdown placeholder
/// instead of taking an entry in the saved-template map.
pub const TEMPLATE_WHITELIST: &[&str] = &[
    "big",
    "lang",
    "nowrap",
    "small",
    "smaller",
    "smallcaps",
    "sc",
    "convert",
    "lang-*",
    "langx",
    "korean",
    "nihongo",
    "transliteration",
    "infobox*", // test might not work well but let's try
];

/// Replaces `<ref>` tags in `wikitext` with Markdown footnote markers.
///
/// A named ref such as `<ref name="source">Citation</ref>` reuses the same key
/// every time it appears, and a self-closing named ref inherits the body from a
/// later definition. Unnamed refs are numbered in order of appearance. Returns
/// the rewritten text and a map from footnote key to citation body.
pub fn stub_ref(wikitext: &str) -> (String, HashMap<String, String>) {
    // Matches paired refs like `<ref>citation</ref>` and self-closing refs
    // like `<ref name="source" />`. It captures the opening-tag attributes
    // and, when present, the body between the tags.
    static REF_TAG: OnceLock<Regex> = OnceLock::new();

    // Finds the `name` attribute in forms like `name="source"`,
    // `name='source'`, or `name=source`. Capture groups 1, 2, and 3 contain
    // the value for those respective forms.
    static REF_NAME: OnceLock<Regex> = OnceLock::new();

    let ref_tag = REF_TAG.get_or_init(|| {
        Regex::new(r"(?is)<ref\b(?P<attrs>[^>]*?)(?:/\s*>|>(?P<body>.*?)</ref\s*>)")
            .expect("ref tag regex is valid")
    });
    let ref_name = REF_NAME.get_or_init(|| {
        Regex::new(r#"(?i)(?:^|\s)name\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s/>]+))"#)
            .expect("ref name regex is valid")
    });

    let mut references = HashMap::new();
    let mut replacements = Vec::new();

    for captures in ref_tag.captures_iter(wikitext) {
        let whole_ref = captures.get(0).expect("whole ref match exists");
        let attrs = captures
            .name("attrs")
            .map_or("", |capture| capture.as_str());
        let body = captures.name("body").map_or("", |capture| capture.as_str());

        let key = if let Some(name) = ref_name
            .captures(attrs)
            .and_then(|captures| {
                captures
                    .get(1)
                    .or_else(|| captures.get(2))
                    .or_else(|| captures.get(3))
            })
            .map(|capture| capture.as_str())
            .filter(|name| !name.is_empty())
        {
            let value = references
                .entry(name.to_string())
                .or_insert_with(String::new);
            // A self-closing named ref may appear before its definition.
            if value.is_empty() && !body.is_empty() {
                value.push_str(body);
            }
            name.to_string()
        } else {
            let mut number = references.len() + 1;
            while references.contains_key(&number.to_string()) {
                number += 1;
            }
            let key = number.to_string();
            references.insert(key.clone(), body.to_string());
            key
        };

        replacements.push((whole_ref.start()..whole_ref.end(), format!("[^{key}]")));
    }

    let mut output = wikitext.to_string();
    for (range, replacement) in replacements.into_iter().rev() {
        output.replace_range(range, &replacement);
    }

    (output, references)
}

/// Converts `[[target|label]]` wikilinks in `wikitext` to Markdown links.
///
/// Link targets are preserved as written, including section fragments. File,
/// image, and category links are left as wikitext, and destinations containing
/// spaces, parentheses, angle brackets, or backslashes are wrapped in angle
/// brackets.
pub fn stub_links(wikitext: &str) -> String {
    let link = wikilink_regex();

    let mut replacements = Vec::new();

    for captures in link.captures_iter(wikitext) {
        let whole_link = captures.get(0).expect("whole link match exists");
        let target = captures
            .name("target")
            .expect("link target match exists")
            .as_str()
            .trim();

        // File links use their pipe-delimited fields for display options and
        // captions (for example `[[File:photo.jpg|thumb|Caption]]`), not as a
        // normal link label. Preserve the original wikitext so those fields
        // are never mistaken for a page-link label.
        if target.split_once(':').is_some_and(|(namespace, _)| {
            namespace.eq_ignore_ascii_case("file")
                || namespace.eq_ignore_ascii_case("image")
                || namespace.eq_ignore_ascii_case("category")
        }) {
            continue;
        }

        let label = captures
            .name("label")
            .map_or(target, |capture| capture.as_str().trim());

        replacements.push((
            whole_link.start()..whole_link.end(),
            format!("[{label}]({})", markdown_link_destination(target)),
        ));
    }

    let mut output = wikitext.to_string();
    for (range, replacement) in replacements.into_iter().rev() {
        output.replace_range(range, &replacement);
    }

    output
}

/// Returns the shared regex that matches `[[target]]` and `[[target|label]]`.
fn wikilink_regex() -> &'static Regex {
    static LINK: OnceLock<Regex> = OnceLock::new();
    LINK.get_or_init(|| {
        Regex::new(r"\[\[(?P<target>[^\[\]|]+)(?:\|(?P<label>[^\[\]]*))?\]\]")
            .expect("link regex is valid")
    })
}

/// Converts MediaWiki section headings in `wikitext` to Markdown headings.
///
/// Each pair of `=` markers becomes one `#`, so `== Heading ==` becomes
/// `## Heading`. Headings whose opening and closing marker counts differ are
/// left unchanged.
pub fn stub_title(wikitext: &str) -> String {
    static TITLE: OnceLock<Regex> = OnceLock::new();
    let title = TITLE.get_or_init(|| {
        Regex::new(r"(?m)^(?P<open>={2,})[ \t]*(?P<title>.*?)[ \t]*(?P<close>={2,})[ \t]*$")
            .expect("title regex is valid")
    });

    title
        .replace_all(wikitext, |captures: &regex::Captures<'_>| {
            let open = captures
                .name("open")
                .expect("opening markers exist")
                .as_str();
            let close = captures
                .name("close")
                .expect("closing markers exist")
                .as_str();
            if open.len() != close.len() {
                return captures
                    .get(0)
                    .expect("whole heading exists")
                    .as_str()
                    .to_string();
            }

            let heading = captures
                .name("title")
                .expect("heading text exists")
                .as_str();
            format!("{} {}", "#".repeat(open.len()), heading.trim())
        })
        .into_owned()
}

/// Formats `destination` for a Markdown link, wrapping it in angle brackets
/// when it contains whitespace or characters that would end the link early.
fn markdown_link_destination(destination: &str) -> String {
    if destination.chars().any(|character| {
        character.is_whitespace() || matches!(character, '(' | ')' | '<' | '>' | '\\')
    }) {
        let escaped = destination
            .replace('\\', "\\\\")
            .replace('<', "\\<")
            .replace('>', "\\>");
        format!("<{escaped}>")
    } else {
        destination.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::StubbedPage;
    use crate::wiki::RawFetcher;
    use std::collections::HashMap;

    #[test]
    fn stub_ref_reuses_named_refs_and_numbers_unnamed_refs() {
        let (output, references) = stub_ref(
            "Before <ref name=\"source\">Citation</ref>, after <ref name='source' /> and <ref>Another citation</ref>.",
        );

        assert_eq!(output, "Before [^source], after [^source] and [^2].");
        assert_eq!(
            references.get("source").map(String::as_str),
            Some("Citation")
        );
        assert_eq!(
            references.get("2").map(String::as_str),
            Some("Another citation")
        );
    }

    #[test]
    fn stub_ref_fills_forward_named_reference_definition() {
        let (output, references) =
            stub_ref("First <ref name=\"source\" /> then <ref name=\"source\">Citation</ref>.");

        assert_eq!(output, "First [^source] then [^source].");
        assert_eq!(
            references.get("source").map(String::as_str),
            Some("Citation")
        );
    }

    #[tokio::test]
    async fn stub_ref_english_page() {
        let client = crate::wiki::wikipedia_client("en").await.unwrap();
        let fetcher = RawFetcher::new(client);
        let wikitext = fetcher.fetch_wikitext("Hello").await.unwrap();
        assert!(!wikitext.is_empty());
        let stubbed = stub_ref(&wikitext);
        println!("{:#?}", stubbed);
    }

    #[test]
    fn stub_links_converts_wikilinks_without_translating_targets() {
        assert_eq!(
            stub_links("[[123|ABC]] [[ABC]] [[Missing|Shown]] [[Plain]]"),
            "[ABC](123) [ABC](ABC) [Shown](Missing) [Plain](Plain)"
        );
    }

    #[test]
    fn stub_links_angle_brackets_destinations_with_spaces_or_parentheses() {
        assert_eq!(
            stub_links("[[ABC (link)|Stuff]] [[Some page]]"),
            "[Stuff](<ABC (link)>) [Some page](<Some page>)"
        );
    }

    #[test]
    fn stub_links_preserves_underscored_parenthetical_targets() {
        assert_eq!(
            stub_links("[[Mercury_(planet)|Mercury]]"),
            "[Mercury](<Mercury_(planet)>)"
        );
        assert_eq!(
            stub_links("[[Mercury_(planet)#History|Mercury]]"),
            "[Mercury](<Mercury_(planet)#History>)"
        );
    }

    #[test]
    fn stub_links_preserves_file_links_and_their_pipe_options() {
        let wikitext = "{{Box|image=Soja_texturiert.jpg}} [[Image:Soja texturiert.jpg|thumb|Textured soy chunks]] and [[File:other.jpg|alt=Soy|A caption]] and [[Image|the Image article]]";
        let (without_templates, templates) =
            crate::markdown::template_new::stub_template_with_whitelist_xml(
                wikitext,
                TEMPLATE_WHITELIST,
            );

        assert_eq!(
            templates.get("Box#1").map(String::as_str),
            Some("Box|image=Soja_texturiert.jpg")
        );
        assert_eq!(
            stub_links(&without_templates),
            "<template name=\"Box#1\"/> [[Image:Soja texturiert.jpg|thumb|Textured soy chunks]] and [[File:other.jpg|alt=Soy|A caption]] and [the Image article](Image)"
        );
    }

    #[test]
    fn wikilink_regex_captures_target_and_optional_label() {
        let captures = wikilink_regex().captures("[[Target|Label]]").unwrap();

        assert_eq!(&captures["target"], "Target");
        assert_eq!(&captures["label"], "Label");
    }

    #[test]
    fn stub_title_converts_wikitext_heading_levels_to_markdown() {
        assert_eq!(
            stub_title("==test==\n=== abc ===\n==== def ====\n===== ghi =====\n====== jkl ======"),
            "## test\n### abc\n#### def\n##### ghi\n###### jkl"
        );
        assert_eq!(stub_title("== mismatched ==="), "== mismatched ===");
    }

    #[test]
    fn stub_and_restub_title_round_trip() {
        let wikitext = "Before\n== test ==\n=== abc ===\n==== def ====\nAfter";
        let page = StubbedPage {
            stubbed: stub_title(wikitext),
            template: HashMap::new(),
            references: HashMap::new(),
        };

        assert_eq!(page.stubbed, "Before\n## test\n### abc\n#### def\nAfter");
        assert_eq!(page.restub(), wikitext);
    }

    #[test]
    fn markdown_link_destination_angle_brackets_destinations_when_needed() {
        assert_eq!(markdown_link_destination("Plain"), "Plain");
        assert_eq!(markdown_link_destination("ABC (link)"), "<ABC (link)>");
        assert_eq!(markdown_link_destination(r"a\b"), r"<a\\b>");
    }
}
