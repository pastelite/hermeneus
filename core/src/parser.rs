// pub fn parse_wikitext_links(wikitext: &str, skip)

use regex::Regex;
use std::{collections::HashMap, sync::OnceLock};

pub fn stub_template(wikitext: &str) -> (String, HashMap<String, String>) {
    let mut inside_template = false;
    let mut output = String::new();
    let mut table: HashMap<String, String> = HashMap::new();

    let mut chars = wikitext.chars().peekable();
    let mut current_buffer = String::new();

    while let Some(current) = chars.next() {
        let next_char = chars.peek();

        if inside_template {
            if current == '}' && next_char == Some(&'}') {
                inside_template = false;
                // skip 1 char
                chars.next();
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

                // let template_key = match existing_key {
                //     Some(key) => format!("{}#{}", template_name, table.len()),
                //     None => template_name.to_string(),
                // };
                // Move the completed template into the table and reset the
                // buffer so it can be reused for the next template.
                let template = std::mem::take(&mut current_buffer);
                output.push_str(format!("`[template:{}]`", &template_key).as_str());
                table.insert(template_key, template);
            } else {
                current_buffer.push(current);
            }
        } else {
            if current == '{' && next_char == Some(&'{') {
                inside_template = true;
                // skip 1 char
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetcher::Fetcher;

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
