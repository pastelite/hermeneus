use std::collections::HashMap;

// Basically uhh, it seems that AI is much better with xml so here is just xml translation
pub fn stub_template_with_whitelist_xml(
    wikitext: &str,
    whitelist: &[&str],
) -> (String, HashMap<String, String>) {
    let mut table: HashMap<String, String> = HashMap::new();
    let mut template_counts: HashMap<String, usize> = HashMap::new();

    let output = render_nested_templates(wikitext, whitelist, &mut table, &mut template_counts);

    (output, table)
}

fn parse_template_saved(
    template: &str,
    table: &mut HashMap<String, String>,
    template_counts: &mut HashMap<String, usize>,
) -> String {
    let template_name = match template.find('|') {
        Some(num) => &template[..num],
        None => template,
    }
    .trim()
    .to_string();
    let count = template_counts
        .entry(template_name.clone())
        .and_modify(|count| *count += 1)
        .or_insert(1);
    let template_key = format!("{}#{}", template_name, count);
    table.insert(template_key.to_owned(), template.to_owned());
    format!("<template name=\"{}\"/>", template_key)
}

// {{a|b|c|test=stuff}} -> <a p1="b" p2="c" test="stuff"/>
// {{a|b|c|test={{stuff}}}} -> <a p1="b" p2="c"><params key="test"><stuff/></params></a>
fn parse_template_whitelist(
    template: &str,
    whitelist: &[&str],
    table: &mut HashMap<String, String>,
    template_counts: &mut HashMap<String, usize>,
) -> String {
    let mut template = template.to_string();
    let template_name = match template.find('|') {
        Some(num) => {
            let name = (&template[..num]).to_owned();
            template = template[num + 1..].to_string();
            name
        }
        None => {
            let name = (&template).to_owned();
            template = "".to_string();
            name
        }
    }
    .trim()
    .to_string();

    // let mut params: IndexMap<String, String> = IndexMap::new();
    let mut params: Vec<(String, String)> = Vec::new();
    let mut numbered_params = 0;

    let mut current_buffer = String::new();
    let mut param_depth = 0;
    let mut contain_template = false;

    let mut chars = template.chars().peekable();

    while let Some(current) = chars.next() {
        let next_char = chars.peek();

        // so basically, we need to handle the {{abc|{{abc|def}}}} case
        // every {{ = +1
        // every }} = -1
        // and only when param_depth == 0 | is recognized as the end of a param
        if param_depth > 0 {
            if current == '{' && next_char == Some(&'{') {
                param_depth += 1;
                current_buffer.push_str("{{");
                chars.next();
                contain_template = true;
            } else if current == '}' && next_char == Some(&'}') {
                param_depth -= 1;
                chars.next();
                current_buffer.push_str("}}");
            } else {
                current_buffer.push(current);
            }
        } else {
            if current == '{' && next_char == Some(&'{') {
                param_depth = 1;
                current_buffer.push_str("{{");
                chars.next();
                contain_template = true;
            } else if current == '|' {
                // let (key, value) = current_buffer.split_once('=').unwrap();
                // params.insert(key.trim().to_string(), value.trim().to_string());
                // current_buffer.clear();
                match split_template_param(&current_buffer) {
                    Some((key, value)) => {
                        params.push((key.trim().to_string(), value.trim().to_string()));
                    }
                    None => {
                        numbered_params += 1;
                        params.push((
                            format!("p{}", numbered_params),
                            current_buffer.trim().to_string(),
                        ));
                    }
                };
                current_buffer.clear();
            } else {
                current_buffer.push(current);
            }
        }
    }
    if !current_buffer.is_empty() {
        match split_template_param(&current_buffer) {
            Some((key, value)) => {
                params.push((key.trim().to_string(), value.trim().to_string()));
            }
            None => {
                numbered_params += 1;
                params.push((
                    format!("p{}", numbered_params),
                    current_buffer.trim().to_string(),
                ));
            }
        };
    }

    if contain_template {
        let mut attrs = String::new();
        let mut nested_params = String::new();
        for (key, value) in params {
            if value.contains("{{") {
                nested_params.push_str(&format!(
                    "<params key=\"{}\">{}</params>",
                    key,
                    render_nested_templates(&value, whitelist, table, template_counts)
                ));
            } else {
                attrs.push_str(&format!(" {}=\"{}\"", key, value));
            }
        }
        format!(
            "<{}{}>{}</{}>",
            template_name, attrs, nested_params, template_name
        )
    } else {
        let mut params_xml = String::new();
        for (key, value) in params {
            params_xml.push_str(&format!(" {}=\"{}\"", key, value));
        }
        format!("<{}{}/>", template_name, params_xml)
    }
}

/// Splits a template parameter at its top-level `=`, ignoring equals signs
/// inside nested template invocations.
fn split_template_param(param: &str) -> Option<(&str, &str)> {
    let mut depth = 0;
    let mut chars = param.char_indices().peekable();

    while let Some((index, current)) = chars.next() {
        match current {
            '{' if chars.peek().is_some_and(|(_, next)| *next == '{') => {
                chars.next();
                depth += 1;
            }
            '}' if chars.peek().is_some_and(|(_, next)| *next == '}') => {
                chars.next();
                depth -= 1;
            }
            '=' if depth == 0 => return Some((&param[..index], &param[index + 1..])),
            _ => {}
        }
    }

    None
}

fn render_nested_templates(
    text: &str,
    whitelist: &[&str],
    table: &mut HashMap<String, String>,
    template_counts: &mut HashMap<String, usize>,
) -> String {
    let mut output = String::new();
    let mut index = 0;

    while index < text.len() {
        let rest = &text[index..];

        if rest.starts_with("{{") {
            let start = index + 2;
            let mut depth = 1;
            let mut cursor = start;

            while cursor < text.len() {
                if text[cursor..].starts_with("{{") {
                    depth += 1;
                    cursor += 2;
                } else if text[cursor..].starts_with("}}") {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    cursor += 2;
                } else {
                    cursor += text[cursor..].chars().next().unwrap().len_utf8();
                }
            }

            if depth == 0 {
                let template = &text[start..cursor];
                let template_name = match template.find('|') {
                    Some(num) => &template[..num],
                    None => template,
                }
                .trim();
                let normalized_template_name = template_name
                    .strip_prefix("Template:")
                    .or_else(|| template_name.strip_prefix("template:"))
                    .unwrap_or(template_name)
                    .trim();

                if whitelist
                    .iter()
                    .any(|pattern| template_name_matches(pattern, normalized_template_name))
                {
                    output.push_str(&parse_template_whitelist(
                        template,
                        whitelist,
                        table,
                        template_counts,
                    ));
                } else {
                    output.push_str(&parse_template_saved(template, table, template_counts));
                }

                index = cursor + 2;
                continue;
            }

            // Unbalanced template: emit the opening braces and keep scanning.
            output.push_str("{{");
            index += 2;
        } else {
            let ch = rest.chars().next().unwrap();
            output.push(ch);
            index += ch.len_utf8();
        }
    }

    output
}

fn template_name_matches(pattern: &str, template_name: &str) -> bool {
    let Some(prefix) = pattern.strip_suffix('*') else {
        return pattern.eq_ignore_ascii_case(template_name);
    };
    template_name
        .get(..prefix.len())
        .is_some_and(|name_prefix| name_prefix.eq_ignore_ascii_case(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_whitelisted(template: &str, whitelist: &[&str]) -> (String, HashMap<String, String>) {
        let mut table = HashMap::new();
        let mut template_counts = HashMap::new();
        let output =
            parse_template_whitelist(template, whitelist, &mut table, &mut template_counts);
        (output, table)
    }

    #[test]
    fn renders_whitelisted_templates_as_xml() {
        let (xml, table) = render_whitelisted("no_params", &[]);
        assert_eq!(xml, "<no_params/>");
        assert!(table.is_empty());

        let (xml, table) = render_whitelisted("a|p1=b|p2=c|test=stuff", &[]);
        assert_eq!(xml, "<a p1=\"b\" p2=\"c\" test=\"stuff\"/>");
        assert!(table.is_empty());

        let (xml, table) = render_whitelisted("a|b|c|test={{stuff}}", &["stuff"]);
        assert_eq!(
            xml,
            "<a p1=\"b\" p2=\"c\"><params key=\"test\"><stuff/></params></a>"
        );
        assert!(table.is_empty());
    }

    #[test]
    fn nested_non_whitelisted_templates_are_saved_like_top_level_ones() {
        let (xml, table) = render_whitelisted("a|b|c|test={{stuff|value=1}}", &[]);

        assert_eq!(
            xml,
            "<a p1=\"b\" p2=\"c\"><params key=\"test\"><template name=\"stuff#1\"/></params></a>"
        );
        assert_eq!(
            table.get("stuff#1").map(String::as_str),
            Some("stuff|value=1")
        );
    }

    #[test]
    fn stub_renders_deeply_nested_whitelisted_templates() {
        let (xml, table) =
            stub_template_with_whitelist_xml("Before {{a|{{b|{{c}}}}}} after", &["a", "b", "c"]);

        assert_eq!(
            xml,
            "Before <a><params key=\"p1\"><b><params key=\"p1\"><c/></params></b></params></a> after"
        );
        assert!(table.is_empty());
    }

    #[test]
    fn stub_saves_non_whitelisted_templates_without_touching_their_nested_content() {
        let (xml, table) =
            stub_template_with_whitelist_xml("Before {{Outer|value={{Inner}}}} after", &[]);

        assert_eq!(xml, "Before <template name=\"Outer#1\"/> after");
        assert_eq!(
            table.get("Outer#1").map(String::as_str),
            Some("Outer|value={{Inner}}")
        );
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn stub_mixes_inline_whitelisted_and_saved_non_whitelisted_templates() {
        let wikitext = "{{small|text {{Infobox|name=x}}}} {{Outer|{{small|hi}}}}";
        let (xml, table) = stub_template_with_whitelist_xml(wikitext, &["small"]);

        assert_eq!(
            xml,
            "<small><params key=\"p1\">text <template name=\"Infobox#1\"/></params></small> <template name=\"Outer#1\"/>"
        );
        assert_eq!(
            table.get("Infobox#1").map(String::as_str),
            Some("Infobox|name=x")
        );
        assert_eq!(
            table.get("Outer#1").map(String::as_str),
            Some("Outer|{{small|hi}}")
        );
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn saved_template_counts_are_shared_between_nested_and_top_level_templates() {
        let (xml, table) = stub_template_with_whitelist_xml(
            "{{small|{{Infobox|nested}}}} {{Infobox|top}}",
            &["small"],
        );

        assert_eq!(
            xml,
            "<small><params key=\"p1\"><template name=\"Infobox#1\"/></params></small> <template name=\"Infobox#2\"/>"
        );
        assert_eq!(
            table.get("Infobox#1").map(String::as_str),
            Some("Infobox|nested")
        );
        assert_eq!(
            table.get("Infobox#2").map(String::as_str),
            Some("Infobox|top")
        );
    }

    #[test]
    fn wildcard_whitelist_applies_to_nested_templates() {
        let (xml, table) = stub_template_with_whitelist_xml("{{langx|{{lang-en|hi}}}}", &["lang*"]);

        assert_eq!(
            xml,
            "<langx><params key=\"p1\"><lang-en p1=\"hi\"/></params></langx>"
        );
        assert!(table.is_empty());
    }

    #[test]
    fn unbalanced_templates_are_left_as_text() {
        let (xml, table) = stub_template_with_whitelist_xml("Before {{unclosed", &["unclosed"]);

        assert_eq!(xml, "Before {{unclosed");
        assert!(table.is_empty());
    }

    #[test]
    fn split_template_param_ignores_equals_signs_inside_nested_templates() {
        assert_eq!(split_template_param("key=value"), Some(("key", "value")));
        assert_eq!(split_template_param("text {{Infobox|name=x}}"), None);
        assert_eq!(
            split_template_param("key={{Other|a=b}}"),
            Some(("key", "{{Other|a=b}}"))
        );
    }

    #[test]
    fn template_name_matches_supports_exact_names_and_prefix_wildcards() {
        assert!(template_name_matches("small", "Small"));
        assert!(!template_name_matches("small", "smaller"));
        assert!(template_name_matches("lang-*", "lang-en"));
        assert!(template_name_matches("lang-*", "Lang-de"));
        assert!(!template_name_matches("lang-*", "other-lang-en"));
    }
}
