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

/// Replaces MediaWiki template invocations in `wikitext` with placeholder tokens.
///
/// Templates in [`TEMPLATE_WHITELIST`] stay inline as
/// `` `[template:name|args]` ``. Every other template is stored in the returned
/// map under a `name#n` key and replaced with `` `[template:name#n]` ``, where
/// `n` distinguishes repeated uses of the same template name.
pub fn stub_template(wikitext: &str) -> (String, HashMap<String, String>) {
    stub_template_with_whitelist(wikitext, TEMPLATE_WHITELIST)
}

/// Replaces templates in `wikitext` like [`stub_template`], using `whitelist`
/// to decide which templates stay inline.
///
/// Each whitelist pattern is matched case-insensitively against the template
/// name with any `Template:` prefix removed. A pattern ending in `*` matches
/// every template whose name starts with the text before the `*`. Templates
/// that do not match are stored in the returned map under a `name#n` key.
pub fn stub_template_with_whitelist(
    wikitext: &str,
    whitelist: &[&str],
) -> (String, HashMap<String, String>) {
    let mut template_depth = 0;
    let mut output = String::new();
    let mut table: HashMap<String, String> = HashMap::new();
    let mut template_counts: HashMap<String, usize> = HashMap::new();

    let mut chars = wikitext.chars().peekable();
    let mut current_buffer = String::new();

    while let Some(current) = chars.next() {
        let next_char = chars.peek();

        if template_depth > 0 {
            if current == '{' && next_char == Some(&'{') {
                template_depth += 1;
                current_buffer.push_str("{{");
                chars.next();
            } else if current == '}' && next_char == Some(&'}') {
                template_depth -= 1;
                chars.next();

                if template_depth == 0 {
                    let template_name = match current_buffer.find('|') {
                        Some(num) => &current_buffer[..num],
                        None => &current_buffer,
                    }
                    .trim()
                    .to_string();
                    let normalized_template_name = template_name
                        .strip_prefix("Template:")
                        .or_else(|| template_name.strip_prefix("template:"))
                        .unwrap_or(&template_name)
                        .trim();

                    let template = std::mem::take(&mut current_buffer);
                    if whitelist
                        .iter()
                        .any(|pattern| template_name_matches(pattern, normalized_template_name))
                    {
                        output.push_str(&format!(
                            "`[template:{}]`",
                            escape_template_token(&template)
                        ));
                    } else {
                        let count = template_counts
                            .entry(template_name.clone())
                            .and_modify(|count| *count += 1)
                            .or_insert(1);
                        let template_key = format!("{}#{}", template_name, count);
                        output.push_str(&format!(
                            "`[template:{}]`",
                            escape_template_token(&template_key)
                        ));
                        table.insert(template_key, template);
                    }
                } else {
                    current_buffer.push_str("}}");
                }
            } else {
                current_buffer.push(current);
            }
        } else {
            if current == '{' && next_char == Some(&'{') {
                template_depth = 1;
                chars.next();
            } else {
                output.push(current);
            }
        }
    }

    (output, table)
}

/// Escapes characters that would otherwise terminate a template placeholder.
fn escape_template_token(template: &str) -> String {
    template.replace('\\', "\\\\").replace(']', "\\]")
}

/// Returns whether a whitelist `pattern` matches `template_name`.
///
/// Matching is case-insensitive. A trailing `*` turns the pattern into a
/// prefix match, so `lang-*` matches `lang-en` and `Lang-de`.
fn template_name_matches(pattern: &str, template_name: &str) -> bool {
    let Some(prefix) = pattern.strip_suffix('*') else {
        return pattern.eq_ignore_ascii_case(template_name);
    };
    template_name
        .get(..prefix.len())
        .is_some_and(|name_prefix| name_prefix.eq_ignore_ascii_case(prefix))
}

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
    fn stub_template_keeps_nested_templates_until_the_outer_template_closes() {
        let wikitext =
            "Before {{Outer|value={{Middle|value={{Inner}}}}}} after {{Sibling|name=soy}}.";

        let (stubbed, templates) = stub_template(wikitext);

        assert_eq!(
            stubbed,
            "Before `[template:Outer#1]` after `[template:Sibling#1]`."
        );
        assert_eq!(
            templates,
            HashMap::from([
                (
                    "Outer#1".to_string(),
                    "Outer|value={{Middle|value={{Inner}}}}".to_string()
                ),
                ("Sibling#1".to_string(), "Sibling|name=soy".to_string()),
            ])
        );
    }

    #[test]
    fn stub_and_restub_nested_templates_round_trip() {
        let wikitext =
            "Before {{Outer|value={{Middle|value={{Inner}}}}}} after {{Sibling|name=soy}}.";
        let (stubbed, templates) = stub_template(wikitext);
        let page = StubbedPage {
            stubbed,
            template: templates,
            references: HashMap::new(),
        };

        assert_eq!(
            page.stubbed,
            "Before `[template:Outer#1]` after `[template:Sibling#1]`."
        );
        assert_eq!(page.restub(), wikitext);
    }

    #[test]
    fn stub_template_numbers_repeated_template_names_with_a_separate_counter_map() {
        let (stubbed, templates) =
            stub_template("{{Box|one}} {{Box|two}} {{Other|three}} {{Box|four}}");

        assert_eq!(
            stubbed,
            "`[template:Box#1]` `[template:Box#2]` `[template:Other#1]` `[template:Box#3]`"
        );
        assert_eq!(templates.get("Box#2").map(String::as_str), Some("Box|two"));
        assert_eq!(templates.len(), 4);
    }

    #[test]
    fn whitelisted_templates_stay_inline_and_escape_closing_brackets() {
        let wikitext = r"{{small|text [[Target|label]] and \ path}}";
        let (stubbed, templates) = stub_template(wikitext);

        assert_eq!(
            stubbed,
            r"`[template:small|text [[Target|label\]\] and \\ path]`"
        );
        assert!(templates.is_empty());
        assert_eq!(
            StubbedPage {
                stubbed,
                template: templates,
                references: HashMap::new(),
            }
            .restub(),
            wikitext
        );
    }

    #[test]
    fn template_whitelist_supports_case_insensitive_prefix_wildcards() {
        let (stubbed, templates) = stub_template(
            "{{lang-en|hello}} {{lang-de|hallo}} {{Lang-en|Hi}} {{other-lang-en|ignored}}",
        );

        assert_eq!(
            stubbed,
            "`[template:lang-en|hello]` `[template:lang-de|hallo]` `[template:Lang-en|Hi]` `[template:other-lang-en#1]`"
        );
        assert_eq!(templates.len(), 1);
        assert_eq!(
            templates.get("other-lang-en#1").map(String::as_str),
            Some("other-lang-en|ignored")
        );
    }

    #[tokio::test]
    async fn stub_template_english_page_after_ref() {
        let client = crate::wiki::wikipedia_client("en").await.unwrap();
        let fetcher = RawFetcher::new(client);
        let wikitext = fetcher.fetch_wikitext("Hello").await.unwrap();
        assert!(!wikitext.is_empty());
        let stubbed_ref = stub_ref(&wikitext);
        let stubbed = stub_template(&stubbed_ref.0);
        println!("{:#?}", stubbed);
    }

    #[test]
    fn stub_template_with_whitelist_uses_the_supplied_whitelist() {
        let (stubbed, templates) =
            stub_template_with_whitelist("{{keep|one}} {{store|two}}", &["keep"]);

        assert_eq!(stubbed, "`[template:keep|one]` `[template:store#1]`");
        assert_eq!(
            templates.get("store#1").map(String::as_str),
            Some("store|two")
        );
    }

    #[test]
    fn escape_template_token_escapes_backslashes_and_closing_brackets() {
        assert_eq!(escape_template_token(r"a\b]c"), r"a\\b\]c");
    }

    #[test]
    fn template_name_matches_supports_exact_names_and_prefix_wildcards() {
        assert!(template_name_matches("small", "Small"));
        assert!(!template_name_matches("small", "smaller"));
        assert!(template_name_matches("lang-*", "lang-en"));
        assert!(template_name_matches("lang-*", "Lang-de"));
        assert!(!template_name_matches("lang-*", "other-lang-en"));
    }

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
        let wikitext = "{{Infobox|image=Soja_texturiert.jpg}} [[Image:Soja texturiert.jpg|thumb|Textured soy chunks]] and [[File:other.jpg|alt=Soy|A caption]] and [[Image|the Image article]]";
        let (without_templates, templates) = stub_template(wikitext);

        assert_eq!(
            templates.get("Infobox#1").map(String::as_str),
            Some("Infobox|image=Soja_texturiert.jpg")
        );
        assert_eq!(
            stub_links(&without_templates),
            "`[template:Infobox#1]` [[Image:Soja texturiert.jpg|thumb|Textured soy chunks]] and [[File:other.jpg|alt=Soy|A caption]] and [the Image article](Image)"
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
