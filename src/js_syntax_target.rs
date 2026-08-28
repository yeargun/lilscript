use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EcmaScriptEdition {
    Es2015,
    Es2016,
    Es2017,
    Es2018,
    Es2019,
    Es2020,
    Es2021,
    #[default]
    Es2022,
    Esnext,
}

impl EcmaScriptEdition {
    pub const fn year(self) -> u16 {
        match self {
            Self::Es2015 => 2015,
            Self::Es2016 => 2016,
            Self::Es2017 => 2017,
            Self::Es2018 => 2018,
            Self::Es2019 => 2019,
            Self::Es2020 => 2020,
            Self::Es2021 => 2021,
            Self::Es2022 | Self::Esnext => 2022,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Es2015 => "es2015",
            Self::Es2016 => "es2016",
            Self::Es2017 => "es2017",
            Self::Es2018 => "es2018",
            Self::Es2019 => "es2019",
            Self::Es2020 => "es2020",
            Self::Es2021 => "es2021",
            Self::Es2022 => "es2022",
            Self::Esnext => "esnext",
        }
    }

    pub const fn allows(self, feature: JsSyntaxFeature) -> bool {
        self.year() >= feature.min_year()
    }

    pub const fn min(self, other: Self) -> Self {
        if self.year() <= other.year() {
            self
        } else {
            other
        }
    }

    pub fn diagnostic_name(self, feature: JsSyntaxFeature) -> String {
        format!(
            "{} requires javascript.ecmascript {} or newer",
            feature.construct(),
            feature.min_edition().name()
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsSyntaxFeature {
    AsyncAwait,
    ObjectValues,
    ObjectRestSpread,
    OptionalCatchBinding,
    OptionalChain,
    NullishCoalescing,
    LogicalAssignment,
    ObjectHasOwn,
    ClassFields,
}

impl JsSyntaxFeature {
    pub const fn min_year(self) -> u16 {
        match self {
            Self::AsyncAwait | Self::ObjectValues => 2017,
            Self::ObjectRestSpread => 2018,
            Self::OptionalCatchBinding => 2019,
            Self::OptionalChain | Self::NullishCoalescing => 2020,
            Self::LogicalAssignment => 2021,
            Self::ObjectHasOwn | Self::ClassFields => 2022,
        }
    }

    pub const fn min_edition(self) -> EcmaScriptEdition {
        match self.min_year() {
            2017 => EcmaScriptEdition::Es2017,
            2018 => EcmaScriptEdition::Es2018,
            2019 => EcmaScriptEdition::Es2019,
            2020 => EcmaScriptEdition::Es2020,
            2021 => EcmaScriptEdition::Es2021,
            _ => EcmaScriptEdition::Es2022,
        }
    }

    pub const fn construct(self) -> &'static str {
        match self {
            Self::AsyncAwait => "async/await",
            Self::ObjectValues => "Object.values",
            Self::ObjectRestSpread => "object rest/spread",
            Self::OptionalCatchBinding => "optional catch binding",
            Self::OptionalChain => "optional chaining",
            Self::NullishCoalescing => "nullish coalescing",
            Self::LogicalAssignment => "logical assignment",
            Self::ObjectHasOwn => "Object.hasOwn",
            Self::ClassFields => "public class fields",
        }
    }
}

pub fn parse_browser_token(token: &str) -> Result<EcmaScriptEdition, String> {
    let bytes = token.as_bytes();
    let split = token.find(char::is_numeric).ok_or_else(|| {
        format!("`javascript.browsers` entry `{token}` must look like chrome80, firefox78, safari14, or edge80")
    })?;
    let name = &token[..split];
    let version = token[split..]
        .parse::<u16>()
        .map_err(|_| format!("`javascript.browsers` entry `{token}` has an invalid version"))?;
    if bytes[split..].iter().any(|byte| !byte.is_ascii_digit()) {
        return Err(format!(
            "`javascript.browsers` entry `{token}` has an invalid version"
        ));
    }
    let table = match name {
        "chrome" => CHROME_EDITIONS,
        "firefox" => FIREFOX_EDITIONS,
        "safari" => SAFARI_EDITIONS,
        "edge" => EDGE_EDITIONS,
        _ => {
            return Err(format!(
                "unknown `javascript.browsers` engine `{name}`; expected chrome, firefox, safari, or edge"
            ));
        }
    };
    table
        .iter()
        .rev()
        .find(|(required, _)| version >= *required)
        .map(|(_, edition)| *edition)
        .ok_or_else(|| {
            format!("`javascript.browsers` entry `{token}` is below the ES2015 syntax floor")
        })
}

pub fn rewrite_host_alias_spelling(
    spelling: &str,
    edition: EcmaScriptEdition,
    shared_binding: bool,
) -> Result<String, String> {
    if spelling == "Object.hasOwn" && !edition.allows(JsSyntaxFeature::ObjectHasOwn) {
        return Ok(if shared_binding {
            "(a,b)=>Object.prototype.hasOwnProperty.call(a,b)".to_string()
        } else {
            "Object.prototype.hasOwnProperty.call".to_string()
        });
    }
    if spelling.contains("?.") && !edition.allows(JsSyntaxFeature::OptionalChain) {
        return Err(edition.diagnostic_name(JsSyntaxFeature::OptionalChain));
    }
    if spelling.contains("??") && !edition.allows(JsSyntaxFeature::NullishCoalescing) {
        return Err(edition.diagnostic_name(JsSyntaxFeature::NullishCoalescing));
    }
    if (spelling.contains("||=") || spelling.contains("??=") || spelling.contains("&&="))
        && !edition.allows(JsSyntaxFeature::LogicalAssignment)
    {
        return Err(edition.diagnostic_name(JsSyntaxFeature::LogicalAssignment));
    }
    Ok(spelling.to_string())
}

pub fn resolve_ecmascript_target(
    edition: EcmaScriptEdition,
    browsers: &[String],
) -> Result<EcmaScriptEdition, String> {
    let mut unique = std::collections::HashSet::with_capacity(browsers.len());
    let mut resolved = edition;
    for token in browsers {
        if !unique.insert(token.as_str()) {
            return Err(format!(
                "`javascript.browsers` contains duplicate `{token}`"
            ));
        }
        resolved = resolved.min(parse_browser_token(token)?);
    }
    Ok(resolved)
}

const CHROME_EDITIONS: &[(u16, EcmaScriptEdition)] = &[
    (51, EcmaScriptEdition::Es2015),
    (52, EcmaScriptEdition::Es2016),
    (55, EcmaScriptEdition::Es2017),
    (64, EcmaScriptEdition::Es2018),
    (66, EcmaScriptEdition::Es2019),
    (80, EcmaScriptEdition::Es2020),
    (85, EcmaScriptEdition::Es2021),
    (93, EcmaScriptEdition::Es2022),
];

const FIREFOX_EDITIONS: &[(u16, EcmaScriptEdition)] = &[
    (54, EcmaScriptEdition::Es2015),
    (54, EcmaScriptEdition::Es2016),
    (54, EcmaScriptEdition::Es2017),
    (58, EcmaScriptEdition::Es2018),
    (58, EcmaScriptEdition::Es2019),
    (74, EcmaScriptEdition::Es2020),
    (79, EcmaScriptEdition::Es2021),
    (92, EcmaScriptEdition::Es2022),
];

const SAFARI_EDITIONS: &[(u16, EcmaScriptEdition)] = &[
    (10, EcmaScriptEdition::Es2015),
    (10, EcmaScriptEdition::Es2016),
    (11, EcmaScriptEdition::Es2017),
    (12, EcmaScriptEdition::Es2018),
    (12, EcmaScriptEdition::Es2019),
    (14, EcmaScriptEdition::Es2020),
    (14, EcmaScriptEdition::Es2021),
    (15, EcmaScriptEdition::Es2022),
];

const EDGE_EDITIONS: &[(u16, EcmaScriptEdition)] = &[
    (15, EcmaScriptEdition::Es2015),
    (15, EcmaScriptEdition::Es2016),
    (15, EcmaScriptEdition::Es2017),
    (79, EcmaScriptEdition::Es2018),
    (79, EcmaScriptEdition::Es2019),
    (80, EcmaScriptEdition::Es2020),
    (85, EcmaScriptEdition::Es2021),
    (93, EcmaScriptEdition::Es2022),
];

#[cfg(test)]
mod tests {
    use super::{
        parse_browser_token, resolve_ecmascript_target, rewrite_host_alias_spelling,
        EcmaScriptEdition, JsSyntaxFeature,
    };

    #[test]
    fn default_edition_matches_the_current_backend() {
        assert_eq!(EcmaScriptEdition::default(), EcmaScriptEdition::Es2022);
        assert!(EcmaScriptEdition::Es2022.allows(JsSyntaxFeature::ObjectHasOwn));
        assert!(!EcmaScriptEdition::Es2021.allows(JsSyntaxFeature::ObjectHasOwn));
        assert!(EcmaScriptEdition::Esnext.allows(JsSyntaxFeature::ObjectHasOwn));
    }

    #[test]
    fn browser_tokens_intersect_to_the_conservative_floor() {
        assert_eq!(
            parse_browser_token("chrome80").unwrap(),
            EcmaScriptEdition::Es2020
        );
        assert_eq!(
            parse_browser_token("firefox92").unwrap(),
            EcmaScriptEdition::Es2022
        );
        assert_eq!(
            parse_browser_token("safari14").unwrap(),
            EcmaScriptEdition::Es2021
        );
        assert_eq!(
            resolve_ecmascript_target(
                EcmaScriptEdition::Es2022,
                &["chrome80".to_string(), "firefox78".to_string()]
            )
            .unwrap(),
            EcmaScriptEdition::Es2020
        );
        assert!(parse_browser_token("chrome40").is_err());
        assert!(parse_browser_token("opera80").is_err());
        assert!(resolve_ecmascript_target(
            EcmaScriptEdition::Es2022,
            &["chrome80".to_string(), "chrome80".to_string()]
        )
        .is_err());
    }

    #[test]
    fn host_alias_has_own_rewrites_below_es2022() {
        assert_eq!(
            rewrite_host_alias_spelling("Object.hasOwn", EcmaScriptEdition::Es2021, true).unwrap(),
            "(a,b)=>Object.prototype.hasOwnProperty.call(a,b)"
        );
        assert_eq!(
            rewrite_host_alias_spelling("Object.hasOwn", EcmaScriptEdition::Es2021, false).unwrap(),
            "Object.prototype.hasOwnProperty.call"
        );
        assert_eq!(
            rewrite_host_alias_spelling("Object.hasOwn", EcmaScriptEdition::Es2022, true).unwrap(),
            "Object.hasOwn"
        );
        assert!(
            rewrite_host_alias_spelling("n?.parentNode??t", EcmaScriptEdition::Es2019, true)
                .is_err()
        );
    }
}
