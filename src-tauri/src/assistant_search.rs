//! Ranked, read-only inventory search. Callers supply a fresh Store snapshot.
//! Matching never creates an item, quantity, product or cost claim.
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchQuality {
    /// A complete real numeric Item Code; no synonym or fuzzy identifier matching.
    Exact,
    /// Every meaningful query term matches real record fields or known synonyms.
    Strong,
    /// A partial word, conservative typo, omitted variant, or limited broadening.
    /// The caller must label/confirm this as a suggestion rather than an exact match.
    Broader,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchMatch {
    pub item: Value,
    pub quality: MatchQuality,
    pub score: u32,
    pub matched_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResults {
    pub query: String,
    pub matches: Vec<SearchMatch>,
    pub total_matches: usize,
    pub truncated: bool,
}

const FIELDS: [(&str, u32); 3] = [("name", 100), ("item_code", 95), ("model_description", 90)];

// LCD/OLED, charger/charging-port, front/rear camera, and device generations are
// deliberately not collapsed into interchangeable parts. Generic class terms
// are added only to the index (e.g. an OLED record is discoverable as a display).
const ALIASES: &[(&str, &[&str])] = &[
    (
        "display",
        &[
            "display",
            "displays",
            "screen",
            "screens",
            "disp",
            "شاشه",
            "شاشات",
        ],
    ),
    (
        "battery",
        &["battery", "batteries", "batt", "bat", "بطاريه", "بطاريات"],
    ),
    ("camera", &["camera", "cameras", "cam", "كاميرا", "كاميرات"]),
    ("speaker", &["speaker", "speakers", "سماعه", "سماعات"]),
    ("earpiece", &["earpiece", "earpieces"]),
    ("loudspeaker", &["loudspeaker", "loudspeakers", "buzzer"]),
    (
        "microphone",
        &["microphone", "microphones", "mic", "مايك", "ميكروفون"],
    ),
    ("charger", &["charger", "chargers", "شاحن", "شواحن"]),
    ("charging", &["charging", "charge", "شحن"]),
    (
        "port",
        &["port", "ports", "socket", "منفذ", "منافذ", "سوكت"],
    ),
    ("connector", &["connector", "connectors", "موصل", "موصلات"]),
    ("flex", &["flex", "flexes", "فلاته", "فلات", "فليكس", "فلكس"]),
    (
        "cable",
        &["cable", "cables", "كابل", "كابلات", "كيبل", "كيابل"],
    ),
    (
        "mainboard",
        &[
            "mainboard",
            "motherboard",
            "motherboards",
            "logicboard",
            "mobo",
        ],
    ),
    (
        "board",
        &["board", "boards", "pcb", "بورد", "بورده", "لوحه", "لوحات"],
    ),
    ("housing", &["housing", "housings", "chassis", "هيكل"]),
    ("cover", &["cover", "covers", "غطاء"]),
    ("button", &["button", "buttons", "زر", "ازرار"]),
    ("power", &["power", "باور", "طاقه"]),
    ("volume", &["volume", "صوت"]),
    ("touch", &["touch", "لمس", "تاتش"]),
    ("digitizer", &["digitizer", "digitiser"]),
    ("iphone", &["iphone", "iph", "ايفون", "ايفونات", "ايفن"]),
    ("ipad", &["ipad", "ايباد"]),
    ("apple", &["apple", "ابل"]),
    ("samsung", &["samsung", "sam", "سامسونج", "سامسونغ"]),
    ("galaxy", &["galaxy", "جالاكسي", "جلاكسي"]),
    ("huawei", &["huawei", "هواوي"]),
    ("honor", &["honor", "اونر", "هونر"]),
    ("xiaomi", &["xiaomi", "شاومي", "شياومي"]),
    ("redmi", &["redmi", "ريدمي"]),
    ("poco", &["poco", "بوكو"]),
    ("oppo", &["oppo", "اوبو"]),
    ("vivo", &["vivo", "فيفو"]),
    ("realme", &["realme", "ريلمي"]),
    ("pixel", &["pixel", "بيكسل", "بكسل"]),
    ("nokia", &["nokia", "نوكيا"]),
    ("oneplus", &["oneplus", "ونبلس"]),
    ("macbook", &["macbook", "ماكبوك"]),
    ("pro", &["pro", "برو"]),
    ("max", &["max", "ماكس"]),
    ("plus", &["plus", "بلس"]),
    ("mini", &["mini", "ميني"]),
    ("ultra", &["ultra", "الترا"]),
    ("lite", &["lite", "لايت"]),
    ("front", &["front", "frontal", "امامي", "اماميه"]),
    ("rear", &["rear", "back", "خلفي", "خلفيه"]),
    ("left", &["left", "يسار", "ايسر"]),
    ("right", &["right", "يمين", "ايمن"]),
    ("black", &["black", "اسود", "سوداء"]),
    ("white", &["white", "ابيض", "بيضاء"]),
    ("blue", &["blue", "ازرق", "زرقاء"]),
    ("green", &["green", "اخضر", "خضراء"]),
    ("usbc", &["usbc", "typec"]),
    ("hdmi", &["hdmi"]),
];

/// Search only actual records in a snapshot returned by Store::execute("snapshot", …).
/// Validation belongs to the caller; this pure function has no filesystem,
/// database, network, or mutation capability. Returned item values are unchanged.
pub fn search_items(snapshot: &Value, query: &str, limit: usize) -> Result<SearchResults, String> {
    let query = query.trim();
    if query.chars().count() > 1000 {
        return Err("Inventory search must contain at most 1,000 characters.".into());
    }
    let items = snapshot["products"]
        .as_array()
        .ok_or("Inventory items are unavailable.")?;
    let limit = limit.clamp(1, 100_000);
    let identifier = normalize_identifier(query);
    let mut exact = Vec::new();
    if !identifier.is_empty() {
        for item in items {
            if text(item, "id").is_empty() {
                continue;
            }
            let fields: Vec<_> = ["id", "item_code"]
                .into_iter()
                .filter(|key| {
                    !text(item, key).is_empty()
                        && normalize_identifier(text(item, key)) == identifier
                })
                .map(str::to_owned)
                .collect();
            if !fields.is_empty() {
                exact.push(SearchMatch {
                    item: item.clone(),
                    quality: MatchQuality::Exact,
                    score: if fields.iter().any(|field| field == "id") {
                        1_000_000
                    } else {
                        999_000
                    },
                    matched_fields: fields,
                });
            }
        }
    }
    if !exact.is_empty() {
        return Ok(results(query, exact, limit));
    }

    let query_tokens = canonical_tokens(query, true);
    if query_tokens.len() > 32 {
        return Err("Use a shorter product name or model.".into());
    }
    if query_tokens.is_empty() {
        return Ok(results(query, Vec::new(), limit));
    }
    let mut matches = Vec::new();
    for item in items {
        if text(item, "id").is_empty() {
            continue;
        }
        if let Some(found) = rank_item(item, &query_tokens) {
            matches.push(found);
        }
    }
    Ok(results(query, matches, limit))
}

fn results(query: &str, mut matches: Vec<SearchMatch>, limit: usize) -> SearchResults {
    matches.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| {
                text(&a.item, "name")
                    .to_lowercase()
                    .cmp(&text(&b.item, "name").to_lowercase())
            })
            .then_with(|| text(&a.item, "id").cmp(text(&b.item, "id")))
    });
    let total_matches = matches.len();
    matches.truncate(limit);
    SearchResults {
        query: query.into(),
        truncated: total_matches > matches.len(),
        total_matches,
        matches,
    }
}

struct IndexedField {
    name: &'static str,
    weight: u32,
    canonical: Vec<String>,
    expanded: Vec<String>,
}

fn rank_item(item: &Value, query: &[String]) -> Option<SearchMatch> {
    let fields: Vec<_> = FIELDS
        .iter()
        .filter_map(|&(name, weight)| {
            let canonical = canonical_tokens(text(item, name), false);
            if canonical.is_empty() {
                return None;
            }
            let mut expanded = canonical.clone();
            for token in &canonical {
                if let Some((_, aliases)) = ALIASES.iter().find(|(word, _)| word == token) {
                    expanded.extend(aliases.iter().map(|alias| (*alias).to_owned()));
                }
                if matches!(
                    token.as_str(),
                    "lcd" | "oled" | "amoled" | "tft" | "digitizer"
                ) {
                    expanded.push("display".into());
                }
                if matches!(token.as_str(), "earpiece" | "loudspeaker") {
                    expanded.push("speaker".into());
                }
                if token == "mainboard" {
                    expanded.push("board".into());
                }
            }
            Some(IndexedField {
                name,
                weight,
                canonical,
                expanded,
            })
        })
        .collect();
    let is_location_query = query.iter().any(|term| {
        matches!(
            term.as_str(),
            "shelf" | "box" | "drawer" | "storage" | "area" | "رف" | "صندوق" | "درج" | "مخزن"
        )
    });
    let mut score = 0u32;
    let mut matched = 0usize;
    let mut direct = 0usize;
    let mut approximate = false;
    let mut matched_fields = Vec::new();
    for term in query {
        let has_number = term.chars().any(|c| c.is_ascii_digit());
        let mut best: Option<(u32, &'static str, bool)> = None;
        for field in &fields {
            // Device/model numbers must not accidentally match a shelf number.
            if has_number
                && !is_location_query
                && !matches!(field.name, "name" | "item_code" | "model_description")
            {
                continue;
            }
            let exact = field.expanded.iter().any(|word| word == term);
            let quality = if exact {
                Some((300 + field.weight, false))
            } else if !has_number && !strict_qualifier(term) {
                field
                    .expanded
                    .iter()
                    .filter_map(|word| fuzzy_word(term, word))
                    .max()
                    .map(|bonus| (bonus + field.weight, true))
            } else {
                None
            };
            if let Some((value, broad)) = quality {
                if best.as_ref().is_none_or(|current| value > current.0) {
                    best = Some((value, field.name, broad));
                }
            }
        }
        if let Some((value, field, broad)) = best {
            score += value;
            matched += 1;
            direct += usize::from(!broad);
            approximate |= broad;
            if !matched_fields.iter().any(|name| name == field) {
                matched_fields.push(field.to_owned());
            }
        } else if has_number || strict_qualifier(term) || essential_term(term) {
            return None;
        }
    }
    if matched == 0 {
        return None;
    }
    let all = matched == query.len();
    // Broader suggestions may omit one descriptive qualifier, but cannot omit
    // a part type, brand/model, requested variant, position, or technology.
    if !all && !(query.len() >= 3 && matched * 4 >= query.len() * 3 && direct >= 2) {
        return None;
    }
    let model_query = query
        .iter()
        .any(|term| term.chars().any(|c| c.is_ascii_digit()))
        && !is_location_query;
    let extra_variant = model_query
        && fields
            .iter()
            .filter(|field| matches!(field.name, "name" | "model_description"))
            .flat_map(|field| &field.canonical)
            .any(|term| variant(term) && !query.contains(term));
    let quality = if all && !approximate && !extra_variant {
        MatchQuality::Strong
    } else {
        MatchQuality::Broader
    };
    score += if quality == MatchQuality::Strong {
        50_000
    } else {
        10_000
    };
    let phrase = query.join(" ");
    if fields
        .iter()
        .any(|field| field.canonical.join(" ") == phrase)
    {
        score += 500;
    }
    Some(SearchMatch {
        item: item.clone(),
        quality,
        score,
        matched_fields,
    })
}

fn variant(term: &str) -> bool {
    matches!(
        term,
        "pro" | "max" | "plus" | "mini" | "ultra" | "lite" | "fe" | "se"
    )
}
fn strict_qualifier(term: &str) -> bool {
    variant(term)
        || matches!(
            term,
            "front"
                | "rear"
                | "left"
                | "right"
                | "lcd"
                | "oled"
                | "amoled"
                | "tft"
                | "incell"
                | "black"
                | "white"
                | "blue"
                | "green"
                | "oem"
        )
}
fn essential_term(term: &str) -> bool {
    matches!(
        term,
        "display"
            | "battery"
            | "camera"
            | "speaker"
            | "earpiece"
            | "loudspeaker"
            | "microphone"
            | "charger"
            | "charging"
            | "port"
            | "connector"
            | "flex"
            | "cable"
            | "mainboard"
            | "board"
            | "housing"
            | "cover"
            | "button"
            | "digitizer"
            | "iphone"
            | "ipad"
            | "apple"
            | "samsung"
            | "galaxy"
            | "huawei"
            | "honor"
            | "xiaomi"
            | "redmi"
            | "poco"
            | "oppo"
            | "vivo"
            | "realme"
            | "pixel"
            | "nokia"
            | "oneplus"
            | "macbook"
    )
}

fn fuzzy_word(query: &str, candidate: &str) -> Option<u32> {
    if candidate.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let q: Vec<char> = query.chars().collect();
    let c: Vec<char> = candidate.chars().collect();
    // Prefixes are suggestions, even when a single part currently matches.
    if q.len() >= 3 && c.starts_with(&q) && q.len() * 2 >= c.len() {
        return Some(180);
    }
    if q.len() < 5 || c.len() < 5 || q.len() > 48 || c.len() > 48 {
        return None;
    }
    let allowed = if q.len().min(c.len()) >= 9 && q.first() == c.first() {
        2
    } else {
        1
    };
    if q.len().abs_diff(c.len()) > allowed {
        return None;
    }
    let distance = edit_distance(&q, &c);
    if distance <= allowed && (q.len().max(c.len()) - distance) * 5 >= q.len().max(c.len()) * 4 {
        Some(if distance == 1 { 130 } else { 90 })
    } else {
        None
    }
}

// Bounded optimal-string-alignment distance (one adjacent transposition counts
// as one typo). It is never used for IDs, codes, numbers, or short technology names.
fn edit_distance(a: &[char], b: &[char]) -> usize {
    let mut previous_previous: Vec<usize> = (0..=b.len()).collect();
    let mut previous = previous_previous.clone();
    for (i, ac) in a.iter().enumerate() {
        let mut current = vec![i + 1; b.len() + 1];
        for (j, bc) in b.iter().enumerate() {
            current[j + 1] = (previous[j + 1] + 1)
                .min(current[j] + 1)
                .min(previous[j] + usize::from(ac != bc));
            if i > 0 && j > 0 && *ac == b[j - 1] && a[i - 1] == *bc {
                current[j + 1] = current[j + 1].min(previous_previous[j - 1] + 1);
            }
        }
        previous_previous = previous;
        previous = current;
    }
    previous[b.len()]
}

fn canonical_tokens(input: &str, remove_filler: bool) -> Vec<String> {
    let normalized = normalize_text(input);
    let words: Vec<String> = normalized.split_whitespace().map(strip_article).collect();
    let mut joined = Vec::new();
    let mut index = 0;
    while index < words.len() {
        let current = words[index].as_str();
        let next = words.get(index + 1).map(String::as_str).unwrap_or("");
        let pair = match (current, next) {
            ("type" | "usb", "c") => Some("usbc"),
            ("logic" | "main", "board") | ("لوحه" | "لوحات", "ام") => Some("mainboard"),
            ("اي" | "ايه", "فون") => Some("iphone"),
            ("اي", "باد") => Some("ipad"),
            ("ون", "بلس") | ("one", "plus") => Some("oneplus"),
            ("سماعه", "مكالمات") => Some("earpiece"),
            _ => None,
        };
        if let Some(pair) = pair {
            joined.push(pair.to_owned());
            index += 2;
            continue;
        }
        if matches!(current, "s" | "a" | "m" | "g" | "j" | "p" | "x")
            && !next.is_empty()
            && next.chars().all(|c| c.is_ascii_digit())
        {
            joined.push(format!("{current}{next}"));
            index += 2;
            continue;
        }
        if current == "sm" && next.chars().any(|c| c.is_ascii_digit()) {
            joined.push(next.to_owned());
            index += 2;
            continue;
        }
        if current.starts_with("sm") && current[2..].chars().any(|c| c.is_ascii_digit()) {
            joined.push(current[2..].to_owned());
        } else {
            expand_compound(current, &mut joined);
        }
        index += 1;
    }
    let mut seen = HashSet::new();
    joined
        .into_iter()
        .map(|word| {
            ALIASES
                .iter()
                .find(|(_, aliases)| aliases.contains(&word.as_str()))
                .map(|(canonical, _)| (*canonical).to_owned())
                .unwrap_or(word)
        })
        .filter(|word| {
            !word.is_empty() && (!remove_filler || !filler(word)) && seen.insert(word.clone())
        })
        .collect()
}

fn expand_compound(word: &str, output: &mut Vec<String>) {
    for prefix in ["iphone", "ipad", "iph", "ايفون", "ايباد"] {
        if let Some(suffix) = word
            .strip_prefix(prefix)
            .filter(|suffix| suffix.starts_with(|c: char| c.is_ascii_digit()))
        {
            output.push(prefix.into());
            expand_compound(suffix, output);
            return;
        }
    }
    let numeric_length = word.chars().take_while(|c| c.is_ascii_digit()).count();
    if numeric_length > 0 && numeric_length < word.len() {
        let suffix = &word[numeric_length..];
        if matches!(
            suffix,
            "pro" | "plus" | "max" | "mini" | "ultra" | "lite" | "promax"
        ) {
            output.push(word[..numeric_length].into());
            if suffix == "promax" {
                output.push("pro".into());
                output.push("max".into());
            } else {
                output.push(suffix.into());
            }
            return;
        }
    }
    if word == "promax" {
        output.push("pro".into());
        output.push("max".into());
    } else {
        output.push(word.into());
    }
}

fn strip_article(word: &str) -> String {
    for prefix in ["وال", "بال", "لل", "ال"] {
        if let Some(rest) = word
            .strip_prefix(prefix)
            .filter(|rest| rest.chars().count() >= 3)
        {
            return rest.into();
        }
    }
    word.into()
}

fn filler(word: &str) -> bool {
    matches!(
        word,
        "how"
            | "many"
            | "much"
            | "where"
            | "when"
            | "what"
            | "which"
            | "is"
            | "are"
            | "the"
            | "a"
            | "an"
            | "do"
            | "does"
            | "we"
            | "you"
            | "i"
            | "have"
            | "has"
            | "any"
            | "in"
            | "of"
            | "for"
            | "to"
            | "from"
            | "and"
            | "or"
            | "please"
            | "show"
            | "find"
            | "search"
            | "look"
            | "looking"
            | "check"
            | "get"
            | "me"
            | "our"
            | "my"
            | "this"
            | "that"
            | "it"
            | "its"
            | "they"
            | "them"
            | "their"
            | "available"
            | "availability"
            | "quantity"
            | "quantities"
            | "stock"
            | "inventory"
            | "item"
            | "items"
            | "part"
            | "parts"
            | "unit"
            | "units"
            | "stored"
            | "located"
            | "details"
            | "about"
            | "tell"
            | "need"
            | "want"
            | "would"
            | "like"
            | "there"
            | "can"
            | "could"
            | "compatible"
            | "compatibility"
            | "fits"
            | "fit"
            | "كم"
            | "اين"
            | "هل"
            | "يوجد"
            | "توجد"
            | "عندكم"
            | "عندنا"
            | "لدينا"
            | "عندي"
            | "اريد"
            | "ابحث"
            | "عن"
            | "لي"
            | "ما"
            | "هي"
            | "هو"
            | "هذه"
            | "هذا"
            | "ذلك"
            | "تلك"
            | "موجود"
            | "موجوده"
            | "متوفر"
            | "متوفره"
            | "متاح"
            | "متاحه"
            | "كميه"
            | "مخزون"
            | "قطعه"
            | "قطع"
            | "غيار"
            | "عدد"
            | "من"
            | "في"
            | "مع"
            | "لديكم"
            | "اعرض"
            | "اعطني"
            | "رجاء"
            | "فضلك"
            | "مكان"
            | "موقع"
            | "اوجد"
            | "احتاج"
    )
}

fn normalize_identifier(value: &str) -> String {
    let value = normalized_chars(value);
    let value = value.trim();
    if value.chars().all(|c| c.is_ascii_digit()) {
        value.trim_start_matches('0').to_owned()
    } else {
        value.to_owned()
    }
}
fn normalize_text(value: &str) -> String {
    normalized_chars(value)
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
fn normalized_chars(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter_map(|character| {
            let normalized = match character {
                '\u{0610}'..='\u{061a}'
                | '\u{064b}'..='\u{065f}'
                | '\u{0670}'
                | '\u{06d6}'..='\u{06ed}'
                | '\u{0640}'
                | '\u{200c}'..='\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}' => return None,
                'أ' | 'إ' | 'آ' | 'ٱ' => 'ا',
                'ى' | 'ی' => 'ي',
                'ة' => 'ه',
                'ک' => 'ك',
                '٠'..='٩' => char::from_u32('0' as u32 + character as u32 - '٠' as u32)?,
                '۰'..='۹' => char::from_u32('0' as u32 + character as u32 - '۰' as u32)?,
                _ => character,
            };
            Some(normalized)
        })
        .collect()
}
fn text<'a>(record: &'a Value, key: &str) -> &'a str {
    record[key].as_str().unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> Value {
        json!({"products":[
        {"id":"a","item_code":"11234","name":"Samsung S3 LCD screen","model_description":"Galaxy S3 GT-I9300","quantity":5},
        {"id":"b","item_code":"11235","name":"iPhone 14 Pro OLED display","model_description":"Black replacement","quantity":2},
        {"id":"c","item_code":"11236","name":"بطارية آيفون 14","model_description":"Original battery","quantity":1}
        ]})
    }
    #[test]
    fn search_numeric_codes_names_models_arabic_and_abbreviations() {
        let data = fixture();
        for q in [
            "11234",
            "٠١١٢٣٤",
            "Samsung S3 screen",
            "GT-I9300",
            "شاشة سامسونج S3",
        ] {
            let r = search_items(&data, q, 20).unwrap();
            assert_eq!(r.matches[0].item["id"], "a", "{q}");
            assert_ne!(r.matches[0].quality, MatchQuality::Broader);
        }
        assert_eq!(
            search_items(&data, "batt iphone 14", 20).unwrap().matches[0].item["id"],
            "c"
        );
    }
    #[test]
    fn similar_names_are_suggestions_and_models_never_fabricated() {
        let data = fixture();
        assert_eq!(
            search_items(&data, "Samsng S3 screeen", 20)
                .unwrap()
                .matches[0]
                .quality,
            MatchQuality::Broader
        );
        for q in [
            "S4 screen",
            "iPhone 14 LCD",
            "iPhone 15 battery",
            "",
            "how many parts",
        ] {
            assert_eq!(search_items(&data, q, 20).unwrap().total_matches, 0, "{q}");
        }
        assert_eq!(
            search_items(&data, "iPhone 14 screen", 20).unwrap().matches[0].quality,
            MatchQuality::Broader
        );
    }
    #[test]
    fn limits_never_change_underlying_records() {
        let data = fixture();
        let original = data.clone();
        let r = search_items(&data, "screen", 1).unwrap();
        assert_eq!(r.total_matches, 2);
        assert!(r.truncated);
        assert_eq!(data, original);
        assert!(search_items(&data, &"a".repeat(1001), 20).is_err());
    }
}
