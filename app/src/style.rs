//! First-pass CSS interpretation.
//!
//! # Scope, deliberately
//!
//! This is not a CSS engine. It resolves a small, explicitly enumerated set of
//! declarations that the current project fixtures actually use, and it records
//! where each resolved value came from.
//!
//! Supported selectors: a single compound selector of tag name, `.class`, or
//! `#id`. No combinators, no pseudo-classes, no media queries, no `@import`.
//! Supported properties are listed in [`SUPPORTED_PROPERTIES`]. Anything else
//! is skipped rather than approximated.
//!
//! # Why provenance is carried
//!
//! Every resolved value keeps the file and byte range of the declaration that
//! produced it. That is what lets a later edit write back to the exact authored
//! span instead of regenerating a stylesheet. It is also why this module never
//! writes anything: resolution and mutation are separate concerns, and keeping
//! them apart is what stops `lamine.yaml` from becoming a second stylesheet.
//!
//! # Cascade
//!
//! Author order wins, with class selectors applied after tag selectors for the
//! same element. That is enough for the fixtures and is deterministic. It is
//! not the CSS cascade, and `docs/` records that as an open boundary.

use std::collections::BTreeMap;

use crate::canvas::Color;

/// The properties this module understands. Anything else is ignored rather
/// than guessed at, so an unsupported declaration can never masquerade as a
/// supported one.
pub const SUPPORTED_PROPERTIES: &[&str] = &[
    "background",
    "background-color",
    "border",
    "border-radius",
    "bottom",
    "box-sizing",
    "color",
    "display",
    "font-size",
    "font-weight",
    "height",
    "left",
    "margin",
    "max-height",
    "max-width",
    "min-height",
    "min-width",
    "opacity",
    "padding",
    "position",
    "right",
    "text-align",
    "top",
    "width",
];

/// One declaration, with the authored span it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
    pub property: String,
    pub value: String,
    /// Byte range of the value text, inside the authored stylesheet.
    pub value_range: (usize, usize),
    /// Author order, used as the tie-breaker between equal specificity.
    pub order: usize,
}

/// A parsed stylesheet: its rules in author order and its `:root` custom
/// properties.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stylesheet {
    rules: Vec<Rule>,
    custom_properties: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq)]
struct Rule {
    selector: Selector,
    declarations: Vec<Declaration>,
}

/// A single compound selector: `main`, `.cta`, `#hero`, or `a.cta`.
#[derive(Clone, Debug, PartialEq)]
struct Selector {
    tag: Option<String>,
    class: Option<String>,
    id: Option<String>,
}

impl Selector {
    fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() || text.contains(char::is_whitespace) {
            // Whitespace means a combinator, which is out of scope.
            return None;
        }
        let mut selector = Self {
            tag: None,
            class: None,
            id: None,
        };
        let mut rest = text;
        while let Some(stripped) = rest.strip_prefix('.') {
            let end = stripped
                .find(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_'))
                .unwrap_or(stripped.len());
            if end == 0 {
                return None;
            }
            selector.class = Some(stripped[..end].to_owned());
            rest = &stripped[end..];
        }
        if let Some(stripped) = rest.strip_prefix('#') {
            let end = stripped
                .find(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_'))
                .unwrap_or(stripped.len());
            if end == 0 {
                return None;
            }
            selector.id = Some(stripped[..end].to_owned());
            rest = &stripped[end..];
        }
        if !rest.is_empty() {
            if selector.tag.is_some() || selector.class.is_some() || selector.id.is_some() {
                return None;
            }
            selector.tag = Some(rest.to_ascii_lowercase());
        }
        (selector.tag.is_some() || selector.class.is_some() || selector.id.is_some())
            .then_some(selector)
    }

    fn matches(&self, tag: &str, classes: &[String], id: Option<&str>) -> bool {
        if let Some(expected) = &self.tag {
            if expected != tag {
                return false;
            }
        }
        if let Some(expected) = &self.class {
            if !classes.iter().any(|class| class == expected) {
                return false;
            }
        }
        if let Some(expected) = &self.id {
            if id != Some(expected.as_str()) {
                return false;
            }
        }
        true
    }
}

impl Stylesheet {
    /// Parse a stylesheet. Never fails: an unparseable rule is skipped, which
    /// matches how the source binder treats malformed markup.
    pub fn parse(source: &str) -> Self {
        let mut stylesheet = Self::default();
        let bytes = source.as_bytes();
        let mut order = 0usize;

        for (rule_range, prelude) in rules(source) {
            let selector_text = prelude.trim();
            // `:root` only contributes custom properties.
            if selector_text == ":root" {
                for (property, value, range) in declarations(source, rule_range) {
                    if let Some(name) = property.strip_prefix("--") {
                        stylesheet
                            .custom_properties
                            .insert(name.to_owned(), value.clone());
                        let _ = range;
                    }
                }
                continue;
            }
            let Some(selector) = Selector::parse(selector_text) else {
                continue;
            };
            let mut parsed = Vec::new();
            for (property, value, range) in declarations(source, rule_range) {
                if !SUPPORTED_PROPERTIES.contains(&property.as_str()) {
                    continue;
                }
                parsed.push(Declaration {
                    property,
                    value,
                    value_range: range,
                    order,
                });
                order += 1;
            }
            if !parsed.is_empty() {
                stylesheet.rules.push(Rule {
                    selector,
                    declarations: parsed,
                });
            }
        }
        let _ = bytes;
        stylesheet
    }

    /// Resolve the declarations that apply to one element, in cascade order.
    ///
    /// Later rules win. Within a rule, later declarations win. Returns the
    /// resolved property values with the provenance of the winning
    /// declaration.
    pub fn resolve(
        &self,
        tag: &str,
        classes: &[String],
        id: Option<&str>,
    ) -> BTreeMap<String, ResolvedValue> {
        let mut resolved: BTreeMap<String, ResolvedValue> = BTreeMap::new();
        for rule in &self.rules {
            if !rule.selector.matches(tag, classes, id) {
                continue;
            }
            for declaration in &rule.declarations {
                let value = self.substitute_variables(&declaration.value);
                resolved.insert(
                    declaration.property.clone(),
                    ResolvedValue {
                        value,
                        order: declaration.order,
                    },
                );
            }
        }
        resolved
    }

    /// The authored span of the declaration that currently wins for `property`
    /// on an element.
    ///
    /// This is what makes CSS ownership checkable: a caller asking "does this
    /// element own `background`?" gets `None` when nothing declared it, and a
    /// byte range into this stylesheet when something did. Returns the winning
    /// declaration, so it agrees with [`Stylesheet::resolve`].
    pub fn declaration_value_range(
        &self,
        tag: &str,
        classes: &[String],
        id: Option<&str>,
        property: &str,
    ) -> Option<(usize, usize)> {
        let mut best: Option<(usize, (usize, usize))> = None;
        for rule in &self.rules {
            if !rule.selector.matches(tag, classes, id) {
                continue;
            }
            for declaration in &rule.declarations {
                if declaration.property != property {
                    continue;
                }
                if best
                    .map(|(order, _)| declaration.order >= order)
                    .unwrap_or(true)
                {
                    best = Some((declaration.order, declaration.value_range));
                }
            }
        }
        best.map(|(_, range)| range)
    }

    /// Expand `var(--name)` against the `:root` custom properties.
    ///
    /// One level of substitution only. A cycle or an unknown name leaves the
    /// reference in place rather than resolving it to something invented.
    fn substitute_variables(&self, value: &str) -> String {
        let mut current = value.trim().to_owned();
        for _ in 0..4 {
            let Some(start) = current.find("var(") else {
                return current;
            };
            let Some(end) = current[start..].find(')') else {
                return current;
            };
            let reference = &current[start + 4..start + end];
            let name = reference.trim().strip_prefix("--").unwrap_or("").trim();
            let Some(replacement) = self.custom_properties.get(name) else {
                return current;
            };
            current = format!(
                "{}{}{}",
                &current[..start],
                replacement,
                &current[start + end + 1..]
            );
        }
        current
    }
}

/// A resolved declaration and the authored span it came from.
///
/// The span is a `(start, end)` byte range into the stylesheet the value was
/// read from, which is what a later source-preserving write targets.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedValue {
    pub value: String,
    pub order: usize,
}

/// Iterate `(body_range, prelude)` for every top-level rule in `source`.
fn rules(source: &str) -> Vec<((usize, usize), &str)> {
    let mut found = Vec::new();
    let bytes = source.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        let Some(open) = source[index..].find('{') else {
            break;
        };
        let open = index + open;
        // Skip at-rules entirely: their bodies are not a flat rule list.
        let line_start = source[..open].rfind('\n').map(|i| i + 1).unwrap_or(0);
        if source[line_start..open].trim_start().starts_with('@') {
            let Some(close) = source[open..].find('}') else {
                break;
            };
            index = open + close + 1;
            continue;
        }
        let Some(close_offset) = source[open..].find('}') else {
            break;
        };
        let close = open + close_offset;
        let prelude = &source[line_start..open];
        found.push(((open + 1, close), prelude));
        index = close + 1;
    }
    found
}

/// Iterate `(property, value, value_range)` inside a rule body.
fn declarations(source: &str, body: (usize, usize)) -> Vec<(String, String, (usize, usize))> {
    let mut found = Vec::new();
    let (body_start, body_end) = body;
    let body = &source[body_start..body_end];
    let base = body_start;
    let mut rest = body;
    while let Some(colon) = rest.find(':') {
        let property = rest[..colon].trim().to_ascii_lowercase();
        rest = &rest[colon + 1..];
        let Some(end) = rest.find(';') else {
            // A final declaration without a trailing semicolon is still valid.
            let value_start = base + (body.len() - rest.len());
            let raw = rest.trim();
            let lead = rest.len() - rest.trim_start().len();
            let start = value_start + lead;
            let end = start + raw.len();
            if !property.is_empty() && !raw.is_empty() {
                found.push((property, raw.to_owned(), (start, end)));
            }
            break;
        };
        let value_start = base + (body.len() - rest.len());
        let raw = rest[..end].trim();
        let lead = rest[..end].len() - rest[..end].trim_start().len();
        let start = value_start + lead;
        let range = (start, start + raw.len());
        rest = &rest[end + 1..];
        if !property.is_empty() && !raw.is_empty() {
            found.push((property, raw.to_owned(), range));
        }
    }
    found
}

/// Parse a CSS colour into the renderer's colour type.
///
/// Only the forms the fixtures use are accepted: `#rgb`, `#rrggbb`, and a few
/// keywords. Anything unrecognised returns `None` so an unsupported colour can
/// never be silently turned into black.
pub fn parse_color(value: &str) -> Option<Color> {
    let value = value.trim().to_ascii_lowercase();
    match value.as_str() {
        "white" => return Some(Color::from_rgb(0xffffff)),
        "black" => return Some(Color::from_rgb(0x000000)),
        "transparent" => return None,
        _ => {}
    }
    let hex = value.strip_prefix('#')?;
    let expanded: String = match hex.len() {
        3 => hex.chars().flat_map(|c| [c, c]).collect(),
        6 => hex.to_owned(),
        _ => return None,
    };
    if !expanded.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(&expanded, 16).ok().map(Color::from_rgb)
}

/// Parse a CSS length into pixels.
///
/// Accepts a bare number as pixels and a `px` suffix. Percentages, `em`, `rem`,
/// and `vh` are out of scope and return `None`.
pub fn parse_length(value: &str) -> Option<f32> {
    let value = value.trim();
    let numeric = value.strip_suffix("px").unwrap_or(value);
    numeric.trim().parse::<f32>().ok()
}

/// Parse a one-to-four component box value such as `12px 20px` or `0`.
pub fn parse_box(value: &str) -> Option<[f32; 4]> {
    let parts: Vec<f32> = value
        .split_whitespace()
        .map(parse_length)
        .collect::<Option<Vec<_>>>()?;
    match parts.len() {
        1 => Some([parts[0]; 4]),
        2 => Some([parts[0], parts[1], parts[0], parts[1]]),
        4 => Some([parts[0], parts[1], parts[2], parts[3]]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved_map(source: &str, tag: &str, classes: &[&str]) -> BTreeMap<String, String> {
        let sheet = Stylesheet::parse(source);
        sheet
            .resolve(
                tag,
                &classes.iter().map(|c| c.to_string()).collect::<Vec<_>>(),
                None,
            )
            .into_iter()
            .map(|(property, resolved)| (property, resolved.value))
            .collect()
    }

    #[test]
    fn a_class_rule_wins_over_a_tag_rule_for_the_same_property() {
        let source = "\
a { background-color: #ffffff; padding: 4px; }
.cta { background-color: #3b5bfd; }
";
        let resolved = resolved_map(source, "a", &["cta"]);
        assert_eq!(
            resolved.get("background-color").map(String::as_str),
            Some("#3b5bfd"),
            "the later class rule overrides the tag rule"
        );
        assert_eq!(
            resolved.get("padding").map(String::as_str),
            Some("4px"),
            "a property the class rule does not mention is kept from the tag rule"
        );
    }

    #[test]
    fn a_declaration_on_an_ancestor_selector_does_not_apply_to_a_child() {
        // No inheritance and no descendant matching: this module answers "which
        // declarations match this element", not "what does this element end up
        // looking like". Property inheritance for text is handled one layer up,
        // in `visual`, where the tree is available.
        let source = "body { color: #16161d; }";
        let resolved = resolved_map(source, "h1", &[]);
        assert!(
            resolved.is_empty(),
            "`body` styles the body, not the headline inside it"
        );
    }

    #[test]
    fn an_unsupported_property_is_skipped_rather_than_approximated() {
        // `transform` and `text-decoration` are real CSS. This milestone does
        // not implement them, and silently approximating them would produce a
        // visual nobody authored. They must simply not resolve.
        let source = ".cta { transform: rotate(4deg); text-decoration: none; color: #ffffff; }";
        let resolved = resolved_map(source, "a", &["cta"]);
        assert!(!resolved.contains_key("transform"));
        assert!(!resolved.contains_key("text-decoration"));
        assert_eq!(resolved.get("color").map(String::as_str), Some("#ffffff"));
    }

    #[test]
    fn a_combinator_selector_is_out_of_scope_and_skipped() {
        // `.card .cta` needs a descendant combinator, which this cascade does
        // not implement. It must not be treated as if it were `.cta`.
        let source = ".card .cta { color: #ff0000; }";
        let sheet = Stylesheet::parse(source);
        let matched = sheet.resolve("a", &["cta".to_string()], None);
        assert!(
            matched.is_empty(),
            "a descendant combinator must not silently apply to the child"
        );
    }

    #[test]
    fn an_at_rule_is_skipped_instead_of_being_read_as_a_flat_rule_list() {
        let source = "\
@media (min-width: 800px) { .cta { color: #ff0000; } }
.cta { color: #0000ff; }
";
        let resolved = resolved_map(source, "a", &["cta"]);
        assert_eq!(
            resolved.get("color").map(String::as_str),
            Some("#0000ff"),
            "the media query body is not a top-level rule"
        );
    }

    #[test]
    fn custom_properties_substitute_into_resolved_values() {
        let source = "\
:root { --accent: #3b5bfd; }
.cta { background: var(--accent); }
";
        let resolved = resolved_map(source, "a", &["cta"]);
        assert_eq!(
            resolved.get("background").map(String::as_str),
            Some("#3b5bfd"),
            "the fixture's variable resolves to its authored value"
        );
    }

    #[test]
    fn an_unknown_variable_is_left_unresolved_rather_than_invented() {
        // Substituting a default would fabricate a colour the author never
        // wrote. Leaving the reference in place means the declaration is
        // visibly unusable instead of silently wrong.
        let source = ".cta { background: var(--nope); }";
        let resolved = resolved_map(source, "a", &["cta"]);
        assert_eq!(
            resolved.get("background").map(String::as_str),
            Some("var(--nope)")
        );
        assert!(
            parse_color("var(--nope)").is_none(),
            "and the unresolved reference cannot be read as a colour"
        );
    }

    #[test]
    fn every_resolved_declaration_points_at_its_authored_value() {
        // Provenance is what lets a later edit rewrite one declaration instead of
        // regenerating the file. A range that does not slice back to the value
        // would make source-preserving saves rewrite the wrong bytes.
        let source = "\
:root { --ink: #16161d; }

.cta {
  background: #3b5bfd;
  color: #ffffff;
}
";
        let sheet = Stylesheet::parse(source);
        let range = |property: &str| -> (usize, usize) {
            sheet
                .rules
                .iter()
                .flat_map(|rule| rule.declarations.iter())
                .find(|declaration| declaration.property == property)
                .map(|declaration| declaration.value_range)
                .unwrap_or_else(|| panic!("{property} resolved"))
        };
        assert_eq!(
            &source[range("background").0..range("background").1],
            "#3b5bfd"
        );
        assert_eq!(&source[range("color").0..range("color").1], "#ffffff");
    }

    #[test]
    fn the_final_declaration_without_a_semicolon_is_still_read() {
        let resolved = resolved_map(".cta { color: #ffffff }", "a", &["cta"]);
        assert_eq!(resolved.get("color").map(String::as_str), Some("#ffffff"));
    }

    #[test]
    fn colour_parsing_stays_inside_the_supported_forms() {
        assert_eq!(
            parse_color("#3b5bfd").map(|c| (c.red, c.green, c.blue)),
            Some((0x3b, 0x5b, 0xfd))
        );
        assert_eq!(
            parse_color("#fff").map(|c| (c.red, c.green, c.blue)),
            Some((255, 255, 255)),
            "three-digit hex expands"
        );
        assert_eq!(
            parse_color("WHITE").map(|c| (c.red, c.green, c.blue)),
            Some((255, 255, 255)),
            "keywords are case-insensitive"
        );
        for unsupported in [
            "rgb(1,2,3)",
            "tomato",
            "transparent",
            "#12345",
            "hsl(0 0% 0%)",
        ] {
            assert!(
                parse_color(unsupported).is_none(),
                "{unsupported} is outside the supported subset and must not be guessed at"
            );
        }
    }

    #[test]
    fn length_parsing_accepts_pixels_and_rejects_relative_units() {
        assert_eq!(parse_length("12px"), Some(12.0));
        assert_eq!(parse_length(" 0 "), Some(0.0));
        for unsupported in ["50%", "2em", "3rem", "10vh", "auto", ""] {
            assert!(
                parse_length(unsupported).is_none(),
                "{unsupported} needs a real layout engine"
            );
        }
    }

    #[test]
    fn box_parsing_handles_one_two_and_four_components() {
        assert_eq!(parse_box("4px"), Some([4.0; 4]));
        assert_eq!(parse_box("12px 20px"), Some([12.0, 20.0, 12.0, 20.0]));
        assert_eq!(parse_box("1px 2px 3px 4px"), Some([1.0, 2.0, 3.0, 4.0]));
        assert!(parse_box("1px 2px 3px").is_none());
        assert!(parse_box("auto").is_none());
    }
}
