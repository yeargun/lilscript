//! Bundled declarations use the ordinary module/checker path. There is no
//! runtime registry: checked extern identities name the existing host values.
use std::path::{Path, PathBuf};

const ROOT: &str = "<lilscript-platform>";
const MODULES: [(&str, &str); 2] = [
    ("ecmascript", include_str!("ecmascript.lil")),
    ("dom", include_str!("dom.lil")),
];

pub(crate) fn resolve(specifier: &str) -> Option<Result<PathBuf, String>> {
    let name = specifier.strip_prefix("lil:")?;
    Some(if MODULES.iter().any(|&(module, _)| module == name) {
        Ok(Path::new(ROOT).join(name).with_extension("lil"))
    } else {
        Err(format!(
            "unknown platform catalog `{specifier}`; available catalogs: lil:ecmascript, lil:dom"
        ))
    })
}

pub(crate) fn source(path: &Path) -> Option<&'static str> {
    if path.parent()? != Path::new(ROOT) || path.extension()? != "lil" {
        return None;
    }
    let name = path.file_stem()?.to_str()?;
    MODULES
        .iter()
        .find_map(|&(module, source)| (module == name).then_some(source))
}

pub(crate) fn delivery_name(path: &Path) -> Option<String> {
    source(path)?;
    Some(format!(
        "lilscript-platform/{}",
        path.file_stem()?.to_str()?
    ))
}
