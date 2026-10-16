// pub fn parse_wikitext_links(wikitext: &str, skip)

use regex::Regex;
use std::{collections::HashMap, sync::OnceLock};

use crate::fetcher::LangLink;

pub fn stub_template(wikitext: &str) -> (String, HashMap<String, String>) {
    let mut template_depth = 0;
    let mut output = String::new();
    let mut table: HashMap<String, String> = HashMap::new();

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
                    // push template to hash map
                    // naming let's do templateName#numberifexists
                    let template_name = match current_buffer.find('|') {
                        Some(num) => &current_buffer[..num],
                        None => &current_buffer,
                    };

                    let existing_key = table
                        .keys()
                        .filter(|key| key.starts_with(template_name))
                        .map(|key| key.to_string())
                        .collect::<Vec<String>>();
                    let template_key = if existing_key.len() > 0 {
                        format!("{}#{}", template_name, existing_key.len())
                    } else {
                        template_name.to_string()
                    };

                    // Move the completed template into the table and reset
                    // the buffer so it can be reused for the next template.
                    let template = std::mem::take(&mut current_buffer);
                    output.push_str(format!("`[template:{}]`", &template_key).as_str());
                    table.insert(template_key, template);
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

pub fn stub_links(wikitext: &str, lang_links: &[LangLink]) -> String {
    let lang_links_map: HashMap<&str, &str> = lang_links
        .iter()
        .map(|link| (link.english_title.as_str(), link.title.as_str()))
        .collect();

    static LINK: OnceLock<Regex> = OnceLock::new();
    let link = LINK.get_or_init(|| {
        Regex::new(r"\[\[(?P<target>[^\[\]|]+)(?:\|(?P<label>[^\[\]]*))?\]\]")
            .expect("link regex is valid")
    });

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

        // MediaWiki treats underscores and spaces equivalently in page titles.
        let lookup_title = target.replace('_', " ");
        let destination = lang_links_map
            .get(lookup_title.as_str())
            .copied()
            .unwrap_or(target);

        replacements.push((
            whole_link.start()..whole_link.end(),
            format!("[{label}]({})", markdown_link_destination(destination)),
        ));
    }

    let mut output = wikitext.to_string();
    for (range, replacement) in replacements.into_iter().rev() {
        output.replace_range(range, &replacement);
    }

    output
}

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
    use crate::fetcher::Fetcher;
    use crate::restub::StubbedPage;
    use std::collections::HashMap;

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
    fn stub_template_keeps_nested_templates_until_the_outer_template_closes() {
        let wikitext =
            "Before {{Outer|value={{Middle|value={{Inner}}}}}} after {{Sibling|name=soy}}.";

        let (stubbed, templates) = stub_template(wikitext);

        assert_eq!(
            stubbed,
            "Before `[template:Outer]` after `[template:Sibling]`."
        );
        assert_eq!(
            templates,
            HashMap::from([
                (
                    "Outer".to_string(),
                    "Outer|value={{Middle|value={{Inner}}}}".to_string()
                ),
                ("Sibling".to_string(), "Sibling|name=soy".to_string()),
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
            "Before `[template:Outer]` after `[template:Sibling]`."
        );
        assert_eq!(page.restub(), wikitext);
    }

    #[test]
    fn stub_links_uses_translations_and_keeps_untranslated_targets() {
        let lang_links = [
            LangLink {
                english_title: "123".to_string(),
                title: "123Translated".to_string(),
                lang: "th".to_string(),
            },
            LangLink {
                english_title: "ABC".to_string(),
                title: "ABCTranslated".to_string(),
                lang: "th".to_string(),
            },
        ];

        let output = stub_links(
            "[[123|ABC]] [[ABC]] [[Missing|Shown]] [[Plain]]",
            &lang_links,
        );

        assert_eq!(
            output,
            "[ABC](123Translated) [ABC](ABCTranslated) [Shown](Missing) [Plain](Plain)"
        );
    }

    #[test]
    fn stub_links_angle_brackets_destinations_with_spaces_or_parentheses() {
        assert_eq!(
            stub_links("[[ABC (link)|Stuff]] [[Some page]]", &[]),
            "[Stuff](<ABC (link)>) [Some page](<Some page>)"
        );
    }

    #[test]
    fn stub_links_preserves_file_links_and_their_pipe_options() {
        let wikitext = "{{Infobox|image=Soja_texturiert.jpg}} [[Image:Soja texturiert.jpg|thumb|Textured soy chunks]] and [[File:other.jpg|alt=Soy|A caption]] and [[Image|the Image article]]";
        let (without_templates, templates) = stub_template(wikitext);

        assert_eq!(
            templates.get("Infobox").map(String::as_str),
            Some("Infobox|image=Soja_texturiert.jpg")
        );
        assert_eq!(
            stub_links(&without_templates, &[]),
            "`[template:Infobox]` [[Image:Soja texturiert.jpg|thumb|Textured soy chunks]] and [[File:other.jpg|alt=Soy|A caption]] and [the Image article](Image)"
        );
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
        let fetcher = Fetcher::new("en").await.unwrap();
        let wikitext = fetcher.fetch_wikitext("Hello").await.unwrap();
        assert!(!wikitext.is_empty());
        let stubbed = stub_ref(&wikitext);
        println!("{:#?}", stubbed);
    }

    #[tokio::test]
    async fn stub_template_english_page_after_ref() {
        let fetcher = Fetcher::new("en").await.unwrap();
        let wikitext = fetcher.fetch_wikitext("Hello").await.unwrap();
        assert!(!wikitext.is_empty());
        let stubbed_ref = stub_ref(&wikitext);
        let stubbed = stub_template(&stubbed_ref.0);
        println!("{:#?}", stubbed);
    }
}
