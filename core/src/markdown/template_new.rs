use std::collections::HashMap;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

// Basically uhh, it seems that AI is much better with xml so here is just xml translation
pub fn stub_template_with_whitelist_xml(
    wikitext: &str,
    whitelist: &[&str],
) -> (String, HashMap<String, String>) {
    let mut table: HashMap<String, String> = HashMap::new();
    let mut template_counts: HashMap<String, usize> = HashMap::new();

    // only attribute values need escaping. text (even inside <params>) stays
    // readable, so markdown links like `[label](<Target (disambiguation)>)`
    // don't turn into `&lt;...&gt;` for the AI.
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
    format!(
        "<template name=\"{}\"/>",
        escape_xml_attribute(&template_key)
    )
}

// {{a|b|c|test=stuff}} -> <a p1="b" p2="c" test="stuff"/>
// {{a|b|c|test={{stuff}}}} -> <a p1="b" p2="c"><params key="test"><stuff/></params></a>

/// It's written on the can man.
/// Basically, if we got "abc {{def}}" and cursor = 4, it will return Some(9), aka the end of template syntax
///
/// Returns `None` when the braces never close.
fn move_to_the_end_of_template(text: &str, start: usize) -> Option<usize> {
    let mut depth = 0;
    let mut cursor = start;

    while cursor < text.len() {
        if text[cursor..].starts_with("{{") {
            depth += 1;
            cursor += 2;
        } else if text[cursor..].starts_with("}}") {
            depth -= 1;
            cursor += 2;

            if depth == 0 {
                return Some(cursor);
            }
        } else {
            cursor += text[cursor..].chars().next().unwrap().len_utf8();
        }
    }

    None
}

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
    let mut cursor = 0;

    while cursor < template.len() {
        // so basically, we need to handle the {{abc|{{abc|def}}}} case
        // every {{ = +1
        // every }} = -1
        // ^ this extracted to move_to_the_end_of_template
        // and only when we are not inside a nested template is | the end of a param
        if template[cursor..].starts_with("{{") {
            match move_to_the_end_of_template(&template, cursor) {
                Some(end) => {
                    current_buffer.push_str(&template[cursor..end]);
                    cursor = end;
                }
                None => {
                    current_buffer.push_str(&template[cursor..]);
                    break;
                }
            }
        } else if template[cursor..].starts_with('|') {
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
            cursor += 1;
        } else {
            let current = template[cursor..].chars().next().unwrap();
            current_buffer.push(current);
            cursor += current.len_utf8();
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

    // names like `Infobox food` have a space, so they can't be a tag name.
    // those still stay inline, but inside a `<template name="...">` wrapper.
    // every parameter becomes a `<params>` child there: attributes always come
    // before children, which would reorder params around the wrapper's `name`.
    let can_be_tag = is_valid_xml_name(&template_name);
    let mut attrs = String::new();
    let mut params_xml = String::new();
    for (key, value) in params {
        if !can_be_tag || needs_params_element(&key, &value) {
            params_xml.push_str(&format!(
                "<params key=\"{}\">{}</params>",
                escape_xml_attribute(&key),
                render_nested_templates(&value, whitelist, table, template_counts)
            ));
        } else {
            attrs.push_str(&format!(" {}=\"{}\"", key, escape_xml_attribute(&value)));
        }
    }

    let (open, close) = if can_be_tag {
        (
            format!("<{}", template_name),
            format!("</{}>", template_name),
        )
    } else {
        (
            format!(
                "<template name=\"{}\"",
                escape_xml_attribute(&template_name)
            ),
            "</template>".to_string(),
        )
    };

    if params_xml.is_empty() {
        format!("{}{}/>", open, attrs)
    } else {
        format!("{}{}>{}{}", open, attrs, params_xml, close)
    }
}

/// Splits a template parameter at its top-level `=`, ignoring equals signs
/// inside nested template invocations.
fn split_template_param(param: &str) -> Option<(&str, &str)> {
    let mut depth = 0;
    let mut chars = param.char_indices().peekable();

    while let Some((index, current)) = chars.next() {
        let next_char = chars.peek().map(|(_, next)| *next);

        if current == '{' && next_char == Some('{') {
            chars.next();
            depth += 1;
        } else if current == '}' && next_char == Some('}') {
            chars.next();
            depth -= 1;
        } else if current == '=' && depth == 0 {
            return Some((&param[..index], &param[index + 1..]));
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
            if let Some(end) = move_to_the_end_of_template(text, index) {
                let template = &text[index + 2..end - 2];
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

                index = end;
                continue;
            }

            // Unbalanced template: emit the opening braces and keep scanning.
            output.push_str("{{");
            index += 2;
        } else {
            let ch = rest.chars().next().unwrap();
            // leave text alone. only attribute values get escaped, so links like
            // `[label](<Target (disambiguation)>)` stay readable for the AI.
            output.push(ch);
            index += ch.len_utf8();
        }
    }

    output
}

/// Escapes an attribute value.
///
/// `&` and `"` still have to be escaped so the value doesn't end the XML by
/// accident, but `<` / `>` are left alone so links inside parameters stay
/// readable (`p1="[abc (link)](<abc (link)>)"`).
fn escape_xml_attribute(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '"' => escaped.push_str("&quot;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

/// Named parameters can only be written as XML attributes when their key is a
/// valid XML name. Otherwise they are emitted as `<params>` elements.
fn needs_params_element(key: &str, value: &str) -> bool {
    value.contains("{{")
        || !is_valid_xml_name(key)
        || value
            .chars()
            .any(|character| matches!(character, '\n' | '\r' | '\t'))
}

fn is_valid_xml_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first == ':' || first.is_alphabetic()) {
        return false;
    }
    chars.all(|character| {
        character == '_'
            || character == ':'
            || character == '-'
            || character == '.'
            || character.is_alphanumeric()
    })
}

/// Restores the XML-like template output to MediaWiki wikitext.
///
/// Saved templates are looked up in `templates`; a missing key is left as its
/// `<template name="..."/>` element so no template content is fabricated.
/// Anything that is not one of our whitelisted template tags is copied back
/// verbatim, so stray `<...>` in the page text survives the XML reader.
pub fn restub_template_with_whitelist_xml(
    xml: &str,
    templates: &HashMap<String, String>,
    whitelist: &[&str],
) -> String {
    // page text is not escaped anymore, so a stray `<` from a markdown link or
    // wikitext would make the xml reader panic. escape just those, not the
    // `<` that starts one of our template tags.
    let escaped = escape_non_template_angle_brackets(xml, whitelist);
    let mut reader = Reader::from_str(&escaped);
    let mut buffer = Vec::new();
    let mut output = String::new();
    let mut last_position = 0;
    // a `<template name="...">` wrapper can hold an inline template whose real
    // name has a space. remember how we treated each opening tag so its end
    // tag knows whether to close the template or pass through.
    let mut open_templates: Vec<TemplateStart> = Vec::new();

    loop {
        let event = reader.read_event_into(&mut buffer);
        let position = reader.buffer_position() as usize;
        // keep the raw slice around so anything that is not one of our tags can
        // be put back exactly as it came in, `<Target (disambiguation)>` and all
        let raw = escaped.get(last_position..position).unwrap_or("");

        match event {
            Ok(Event::Start(event)) => {
                let name = tag_name(&event);
                // only `params`, `template`, and whitelisted template names are
                // ours. any other start tag is page text that just happens to
                // start with `<`, so we leave it alone.
                if name == "params" {
                    if let Some(key) = attribute_value(&event, b"key") {
                        output.push('|');
                        if !is_positional_param(&key) {
                            output.push_str(&key);
                            output.push('=');
                        }
                    }
                } else if name == "template" {
                    let key = attribute_value(&event, b"name").unwrap_or_default();
                    if let Some(template) = templates.get(&key) {
                        output.push_str("{{");
                        output.push_str(template);
                        output.push_str("}}");
                        open_templates.push(TemplateStart::Saved);
                    } else if is_inline_template_key(&key, whitelist) {
                        output.push_str("{{");
                        output.push_str(&key);
                        append_template_attributes(&mut output, &event, Some(b"name"));
                        open_templates.push(TemplateStart::Inline);
                    } else {
                        output.push_str(raw);
                        open_templates.push(TemplateStart::Text);
                    }
                } else if is_whitelisted_template(&name, whitelist) {
                    output.push_str("{{");
                    output.push_str(&name);
                    append_template_attributes(&mut output, &event, None);
                } else {
                    output.push_str(raw);
                }
            }
            Ok(Event::Empty(event)) => {
                let name = tag_name(&event);
                if name == "template" {
                    if let Some(key) = attribute_value(&event, b"name") {
                        if let Some(template) = templates.get(&key) {
                            output.push_str("{{");
                            output.push_str(template);
                            output.push_str("}}");
                        } else if is_inline_template_key(&key, whitelist) {
                            output.push_str("{{");
                            output.push_str(&key);
                            append_template_attributes(&mut output, &event, Some(b"name"));
                            output.push_str("}}");
                        } else {
                            output.push_str("<template name=\"");
                            output.push_str(&escape_xml_attribute(&key));
                            output.push_str("\"/>");
                        }
                    }
                } else if is_whitelisted_template(&name, whitelist) {
                    output.push_str("{{");
                    output.push_str(&name);
                    append_template_attributes(&mut output, &event, None);
                    output.push_str("}}");
                } else {
                    output.push_str(raw);
                }
            }
            Ok(Event::End(event)) => {
                let name = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if name == "params" {
                    // The opening `<params>` already wrote the `|key=`.
                } else if name == "template" {
                    match open_templates.pop() {
                        Some(TemplateStart::Inline) => output.push_str("}}"),
                        Some(TemplateStart::Saved) => {}
                        _ => output.push_str(raw),
                    }
                } else if is_whitelisted_template(&name, whitelist) {
                    output.push_str("}}");
                } else {
                    output.push_str(raw);
                }
            }
            Ok(Event::Text(text)) => {
                output.push_str(&unescape_xml_text(&String::from_utf8_lossy(text.as_ref())))
            }
            Ok(Event::CData(text)) => output.push_str(&String::from_utf8_lossy(text.as_ref())),
            Ok(Event::Eof) => break,
            Ok(_) => output.push_str(raw),
            // if the xml is too broken, hand the text back untouched rather
            // than half-restoring it
            Err(_) => return xml.to_string(),
        }
        last_position = position;
        buffer.clear();
    }

    output
}

/// Escapes a `<` unless it starts one of our template tags.
///
/// Page text and attribute values are displayed unescaped, so leftover markdown
/// links like `[label](<Target (disambiguation)>)` would otherwise confuse the
/// xml reader. Quotes only count as attribute delimiters once we are already
/// inside one of our tags, so a stray `"` in prose can't swallow the next
/// template.
fn escape_non_template_angle_brackets(xml: &str, whitelist: &[&str]) -> String {
    let mut output = String::with_capacity(xml.len());
    let mut index = 0;

    while index < xml.len() {
        let rest = &xml[index..];

        if rest.starts_with('<') {
            let after = rest
                .strip_prefix("</")
                .or_else(|| rest.strip_prefix('<'))
                .unwrap_or("");
            let name: String = after
                .chars()
                .take_while(|ch| {
                    *ch == '_' || *ch == ':' || *ch == '-' || *ch == '.' || ch.is_alphanumeric()
                })
                .collect();
            let tail = &after[name.len()..];

            // a markdown link like `[Korean](<Korean cuisine>)` is text, not a
            // `<Korean ...>` tag. our tags always have `>` / `/>` right after
            // the name, or `key="value"` attributes, so a bare word after the
            // name means page text.
            let is_our_tag = looks_like_our_tag(tail)
                && (name == "template"
                    || name == "params"
                    || is_whitelisted_template(&name, whitelist));

            if is_our_tag {
                // one of our tags, so copy it as a whole. a `<` inside one of
                // its quoted attributes still has to become `&lt;`.
                output.push('<');
                index += 1;
                let mut in_quotes = false;

                while index < xml.len() {
                    let ch = xml[index..].chars().next().unwrap();

                    if ch == '"' {
                        in_quotes = !in_quotes;
                        output.push('"');
                    } else if ch == '<' && in_quotes {
                        output.push_str("&lt;");
                    } else if ch == '>' && !in_quotes {
                        output.push('>');
                        index += ch.len_utf8();
                        break;
                    } else {
                        output.push(ch);
                    }

                    index += ch.len_utf8();
                }
            } else {
                output.push_str("&lt;");
                index += 1;
            }
        } else {
            let ch = rest.chars().next().unwrap();
            output.push(ch);
            index += ch.len_utf8();
        }
    }

    output
}

/// Tells our generated tag apart from page text that starts with `<`.
///
/// Our tags look like `<small/>`, `<small p1="x"/>`, or `<template name="...">`.
/// A markdown link target like `<Korean cuisine>` has a bare word after the
/// name instead of an attribute, so it is text.
fn looks_like_our_tag(tail: &str) -> bool {
    if tail.starts_with('>') || tail.starts_with("/>") {
        return true;
    }

    if !tail.chars().next().is_some_and(char::is_whitespace) {
        return false;
    }

    let mut saw_equals = false;
    let mut previous = '\0';
    for ch in tail.chars() {
        if ch == '>' {
            return saw_equals || previous == '/';
        }

        if ch == '=' {
            saw_equals = true;
        }

        previous = ch;
    }

    false
}

/// Unescapes the xml entities we care about, but leaves broken ones alone.
///
/// The page can still contain a plain `&` from wikitext, so `text.unescape()`
/// would fail on the whole chunk just because of one character.
fn unescape_xml_text(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut index = 0;

    while index < text.len() {
        let rest = &text[index..];
        let (replacement, length) = if rest.starts_with("&lt;") {
            ("<", 4)
        } else if rest.starts_with("&gt;") {
            (">", 4)
        } else if rest.starts_with("&amp;") {
            ("&", 5)
        } else if rest.starts_with("&quot;") {
            ("\"", 6)
        } else if rest.starts_with("&apos;") {
            ("'", 6)
        } else {
            let ch = rest.chars().next().unwrap();
            output.push(ch);
            index += ch.len_utf8();
            continue;
        };

        output.push_str(replacement);
        index += length;
    }

    output
}

/// What we did with an opening `<template ...>` tag, so the matching end tag
/// can do the same.
#[derive(Clone, Copy)]
enum TemplateStart {
    /// Inline template inside a `<template name="...">` wrapper. The end tag
    /// still has to close the `{{...}}`.
    Inline,
    /// A saved template written with an explicit end tag (not what stubbing
    /// produces, but handle it anyway).
    Saved,
    /// Page text that only looked like a `<template>` tag.
    Text,
}

/// Checks whether a tag name is one of the whitelisted templates we rendered
/// into XML (with any `Template:` prefix stripped, just like stubbing does).
fn is_whitelisted_template(name: &str, whitelist: &[&str]) -> bool {
    let normalized = name
        .strip_prefix("Template:")
        .or_else(|| name.strip_prefix("template:"))
        .unwrap_or(name)
        .trim();
    whitelist
        .iter()
        .any(|pattern| template_name_matches(pattern, normalized))
}

/// Saved templates always use a `name#n` key. A `<template name="...">` that
/// isn't one of those is an inline template whose real name can't be a tag
/// name (it has a space or a slash).
fn is_inline_template_key(key: &str, whitelist: &[&str]) -> bool {
    !looks_like_saved_key(key) && is_whitelisted_template(key, whitelist)
}

/// `Infobox#1`, `small#12`, ... all come from the saved-template map.
fn looks_like_saved_key(key: &str) -> bool {
    let Some((_, digits)) = key.rsplit_once('#') else {
        return false;
    };

    !digits.is_empty() && digits.chars().all(|character| character.is_ascii_digit())
}

/// just gets the tag name, like `params` or `template`
fn tag_name(event: &BytesStart<'_>) -> String {
    String::from_utf8_lossy(event.name().as_ref()).into_owned()
}

/// just finds one attribute on the tag and gives back its unescaped value
/// aka. (<abc params="d">, "params") -> Some("d")
/// Basically just a helper to make quick xml life easier
fn attribute_value(event: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    event
        .attributes()
        .with_checks(false)
        .flatten()
        .find(|attribute| attribute.key.as_ref() == key)
        .and_then(|attribute| attribute.unescape_value().ok())
        .map(|value| value.into_owned())
}

/// turns all the attributes back into |params.
/// p1, p2, ... are positional, so they stay as just |value
/// everything else becomes |key=value
///
/// `skip` is for attributes we added ourselves, like the `name` on a
/// `<template name="...">` wrapper.
fn append_template_attributes(output: &mut String, event: &BytesStart<'_>, skip: Option<&[u8]>) {
    for attribute in event.attributes().with_checks(false).flatten() {
        if skip.is_some_and(|key| attribute.key.as_ref() == key) {
            continue;
        }

        let key = String::from_utf8_lossy(attribute.key.as_ref());
        let value = attribute
            .unescape_value()
            .map(|value| value.into_owned())
            .unwrap_or_default();

        output.push('|');
        if !is_positional_param(&key) {
            output.push_str(&key);
            output.push('=');
        }
        output.push_str(&value);
    }
}

/// so we make every params without a key to begin with p. this is just check that
fn is_positional_param(key: &str) -> bool {
    let Some(digits) = key.strip_prefix('p') else {
        return false;
    };
    !digits.is_empty() && digits.chars().all(|character| character.is_ascii_digit())
}

/// just matching the whitelist
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

    fn round_trip(wikitext: &str, whitelist: &[&str]) -> (String, HashMap<String, String>, String) {
        let (xml, table) = stub_template_with_whitelist_xml(wikitext, whitelist);
        let restored = restub_template_with_whitelist_xml(&xml, &table, whitelist);
        (xml, table, restored)
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
    fn restub_restores_inline_and_nested_templates() {
        let wikitext = "Before {{a|b|c|test=stuff}} {{a|b|c|test={{stuff}}}} after";
        let (_, _, restored) = round_trip(wikitext, &["a", "stuff"]);

        assert_eq!(restored, wikitext);
    }

    #[test]
    fn restub_restores_saved_top_level_and_nested_templates() {
        let wikitext = "Before {{Outer|value={{Inner}}}} after {{Sibling|name=soy}}.";
        let (_, _, restored) = round_trip(wikitext, &[]);

        assert_eq!(restored, wikitext);
    }

    #[test]
    fn restub_restores_mixed_inline_and_saved_templates() {
        let wikitext = "{{small|text {{Infobox|name=x}}}} {{Outer|{{small|hi}}}}";
        let (_, _, restored) = round_trip(wikitext, &["small"]);

        assert_eq!(restored, wikitext);
    }

    #[test]
    fn restub_restores_wildcard_nested_templates() {
        let wikitext = "{{langx|{{lang-en|hi}}}}";
        let (_, _, restored) = round_trip(wikitext, &["lang*"]);

        assert_eq!(restored, wikitext);
    }

    #[test]
    fn restub_restores_xml_special_characters() {
        let wikitext = "A & B < C {{small|a & b \"quoted\" <tag>}}";
        let (xml, _, restored) = round_trip(wikitext, &["small"]);

        assert!(xml.contains("&amp;"));
        assert!(xml.contains("&quot;"));
        assert!(xml.contains("<tag>"));
        assert_eq!(restored, wikitext);
    }

    #[test]
    fn restub_restores_repeated_saved_template_counts() {
        let wikitext = "{{small|{{Infobox|nested}}}} {{Infobox|top}}";
        let (_, _, restored) = round_trip(wikitext, &["small"]);

        assert_eq!(restored, wikitext);
    }

    #[test]
    fn restub_restores_parameter_keys_that_cannot_be_xml_attributes() {
        let wikitext = "{{small|1=first|some key=second}}";
        let (xml, _, restored) = round_trip(wikitext, &["small"]);

        assert!(xml.contains("<params key=\"1\">first</params>"));
        assert!(xml.contains("<params key=\"some key\">second</params>"));
        assert_eq!(restored, wikitext);
    }

    #[test]
    fn restub_restores_parameter_values_with_newlines() {
        let wikitext = "{{small|line1\nline2}}";
        let (_, _, restored) = round_trip(wikitext, &["small"]);

        assert_eq!(restored, wikitext);
    }

    #[test]
    fn restub_leaves_unknown_saved_templates_as_xml() {
        let restored = restub_template_with_whitelist_xml(
            "Before <template name=\"Unknown#1\"/> after",
            &HashMap::new(),
            &[],
        );

        assert_eq!(restored, "Before <template name=\"Unknown#1\"/> after");
    }

    #[test]
    fn whitelisted_template_names_with_spaces_stay_inline_in_a_template_wrapper() {
        let wikitext = "{{Infobox Korean name/auto|hangul=김치}}";
        let (xml, table, restored) = round_trip(wikitext, &["infobox*"]);

        assert_eq!(
            xml,
            "<template name=\"Infobox Korean name/auto\"><params key=\"hangul\">김치</params></template>"
        );
        assert!(table.is_empty());
        assert_eq!(restored, wikitext);
    }

    #[test]
    fn infobox_name_parameter_does_not_collide_with_the_wrapper_attribute() {
        let wikitext = "{{Infobox food|name=Kimchi|image=Various kimchi.jpg}}";
        let (xml, table, restored) = round_trip(wikitext, &["infobox*"]);

        assert_eq!(
            xml,
            "<template name=\"Infobox food\"><params key=\"name\">Kimchi</params><params key=\"image\">Various kimchi.jpg</params></template>"
        );
        assert!(table.is_empty());
        assert_eq!(restored, wikitext);
    }

    #[test]
    fn link_targets_that_look_like_tags_inside_params_stay_text() {
        let wikitext = "{{Infobox food|associated_cuisine=[Korean](<Korean cuisine>)}}";
        let (xml, _, restored) = round_trip(wikitext, &["infobox*", "korean"]);

        assert_eq!(
            xml,
            "<template name=\"Infobox food\"><params key=\"associated_cuisine\">[Korean](<Korean cuisine>)</params></template>"
        );
        assert_eq!(restored, wikitext);
    }

    #[test]
    fn empty_whitelisted_template_with_spaces_round_trips() {
        let wikitext = "{{Infobox food}}";
        let (xml, _, restored) = round_trip(wikitext, &["infobox*"]);

        assert_eq!(xml, "<template name=\"Infobox food\"/>");
        assert_eq!(restored, wikitext);
    }

    #[test]
    fn unknown_saved_keys_are_not_mistaken_for_inline_templates() {
        let restored = restub_template_with_whitelist_xml(
            "Before <template name=\"Infobox food#1\"/> after",
            &HashMap::new(),
            &["infobox*"],
        );

        assert_eq!(restored, "Before <template name=\"Infobox food#1\"/> after");
    }

    #[test]
    fn stray_quotes_in_page_text_do_not_break_restub() {
        let wikitext = "5' 6\" {{Korean|hangul=만두}}";
        let (xml, _, restored) = round_trip(wikitext, &["korean"]);

        assert!(xml.contains("<Korean hangul=\"만두\"/>"));
        assert_eq!(restored, wikitext);
    }

    #[test]
    fn only_attribute_values_are_escaped() {
        let wikitext = "{{Outer|value=x < y & z {{Inner}}}}";
        let (xml, _, restored) = round_trip(wikitext, &["Outer"]);

        assert!(xml.contains("x < y & z"));
        assert!(!xml.contains("&lt;"));
        assert_eq!(restored, wikitext);
    }

    #[test]
    fn stub_keeps_angle_bracket_links_readable() {
        let wikitext = "[โดเมน](<โดเมน (ชีววิทยา)>), A & B < C";
        let (xml, _, restored) = round_trip(wikitext, &[]);

        assert!(!xml.contains("&lt;"));
        assert!(!xml.contains("&gt;"));
        assert_eq!(restored, wikitext);
    }

    #[test]
    fn restub_restores_unbalanced_template_text() {
        let (_, _, restored) = round_trip("Before {{unclosed", &["unclosed"]);

        assert_eq!(restored, "Before {{unclosed");
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
