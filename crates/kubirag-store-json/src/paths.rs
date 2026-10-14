//! Percorsi come `java.io.File`: canonici quando esistono, altrimenti assoluti e normalizzati,
//! e relativi scritti sempre con `/` (`invariantSeparatorsPath`).

use std::path::{Component, Path, PathBuf};

/// `File.canonicalFile`: risolve i link se il percorso esiste. `dunce` evita il prefisso
/// `\\?\` di Windows, che renderebbe i percorsi non confrontabili.
pub fn canonical(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| {
        let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        normalize(&absolute)
    })
}

fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }
    normalized
}

/// Percorso relativo con separatori `/`, se `path` sta dentro `base`.
pub fn relative_invariant(path: &Path, base: &Path) -> Option<String> {
    let relative = path.strip_prefix(base).ok()?;
    Some(
        relative
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

/// `File(parent, child)` della JVM: un figlio assoluto viene accodato, non sostituisce.
pub fn java_child(parent: &Path, child: &str) -> PathBuf {
    if Path::new(child).is_absolute() {
        parent.join(child.trim_start_matches(['/', '\\']))
    } else {
        parent.join(child)
    }
}

/// `name.replace(Regex("[^a-zA-Z0-9._-]"), "_")`: la regex lavora su unità UTF-16, quindi un
/// carattere fuori dal piano base (emoji) diventa due `_`.
pub fn safe_file_name(name: &str) -> String {
    name.chars()
        .flat_map(|c| {
            let keep = c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
            let repeat = if keep { 1 } else { c.len_utf16() };
            std::iter::repeat_n(if keep { c } else { '_' }, repeat)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_characters_become_underscores_per_utf16_unit() {
        assert_eq!(safe_file_name("città 🎉.md"), "citt____.md");
        assert_eq!(safe_file_name("a b"), "a_b");
        assert_eq!(safe_file_name("🎉"), "__");
        assert_eq!(safe_file_name("città.md"), "citt_.md");
    }
}
