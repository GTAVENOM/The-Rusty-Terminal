use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct FuzzyCandidate {
    pub path: PathBuf,
    pub display: String,
    pub score: i64,
}

pub fn is_home_alias(target: &str) -> bool {
    let lower = target.trim().trim_end_matches('/').to_lowercase();
    matches!(
        lower.as_str(),
        "~" | "home"
            | "/home"
            | "home dir"
            | "/home dir"
            | "home directory"
            | "/home directory"
            | "user home"
            | "my home"
    )
}

pub fn is_parent_alias(target: &str) -> bool {
    let lower = target.trim().trim_end_matches('/').to_lowercase();
    matches!(
        lower.as_str(),
        ".." | "parent" | "parent dir" | "parent directory" | "go back" | "up" | "back"
    )
}

pub fn resolve_multi_segment_path(target: &str) -> Option<PathBuf> {
    let clean = target
        .trim()
        .replace(['"', '\''], "")
        .replace('\\', " ");

    if clean.is_empty() {
        return None;
    }

    let home = dirs::home_dir();

    if clean == "~" || is_home_alias(&clean) {
        return home;
    }

    let is_absolute = clean.starts_with('/');
    let is_home_rel = clean.starts_with("~/");

    let segments: Vec<&str> = if is_home_rel {
        clean[2..].split('/').collect()
    } else if is_absolute {
        clean[1..].split('/').collect()
    } else {
        clean.split('/').collect()
    };

    let walk_segments = |start_base: PathBuf| -> Option<PathBuf> {
        let mut current = start_base;
        let matcher = SkimMatcherV2::default();

        for seg in &segments {
            let seg_trimmed = seg.trim();
            if seg_trimmed.is_empty() || seg_trimmed == "." {
                continue;
            }
            if seg_trimmed == ".." {
                if let Some(parent) = current.parent() {
                    current = parent.to_path_buf();
                }
                continue;
            }

            // Direct child directory match
            let direct_child = current.join(seg_trimmed);
            if direct_child.exists() && direct_child.is_dir() {
                current = direct_child;
                continue;
            }

            // Fuzzy match segment against subdirectories in current
            let seg_normalized = seg_trimmed.replace('-', " ").to_lowercase();
            let seg_compact = seg_normalized.replace(' ', "");
            let mut best_sub_match: Option<(i64, PathBuf)> = None;

            if let Ok(entries) = fs::read_dir(&current) {
                for entry in entries.flatten() {
                    if entry.metadata().map(|m| m.is_dir()).unwrap_or(false) {
                        let name = entry.file_name();
                        let name_str = match name.to_str() {
                            Some(s) => s,
                            None => continue,
                        };
                        if name_str.starts_with('.') {
                            continue;
                        }

                        let name_normalized = name_str.replace('-', " ").to_lowercase();
                        let name_compact = name_normalized.replace(' ', "");

                        let score = if name_normalized == seg_normalized {
                            10000
                        } else if name_compact == seg_compact {
                            9000
                        } else if name_normalized.contains(&seg_normalized)
                            || seg_normalized.contains(&name_normalized)
                        {
                            7000
                        } else {
                            matcher
                                .fuzzy_match(&name_normalized, &seg_normalized)
                                .unwrap_or(0)
                        };

                        if score > 0 {
                            if let Some((best_score, _)) = best_sub_match {
                                if score > best_score {
                                    best_sub_match = Some((score, entry.path()));
                                }
                            } else {
                                best_sub_match = Some((score, entry.path()));
                            }
                        }
                    }
                }
            }

            if let Some((_, matched_child)) = best_sub_match {
                current = matched_child;
            } else {
                return None;
            }
        }

        Some(current)
    };

    if is_home_rel {
        walk_segments(home?)
    } else if is_absolute {
        if is_home_alias(&clean) {
            return home;
        }
        walk_segments(PathBuf::from("/"))
    } else {
        if let Ok(cwd) = std::env::current_dir() {
            if let Some(res) = walk_segments(cwd) {
                return Some(res);
            }
        }
        if let Some(h) = home {
            if let Some(res) = walk_segments(h) {
                return Some(res);
            }
        }
        None
    }
}

pub fn resolve_fuzzy_candidates(target: &str) -> Vec<FuzzyCandidate> {
    let clean_target = target.trim();
    if clean_target.is_empty() {
        return Vec::new();
    }

    let home = dirs::home_dir();
    let is_home = is_home_alias(clean_target);
    let is_parent = is_parent_alias(clean_target);
    let mut candidates: Vec<FuzzyCandidate> = Vec::new();

    // 1. Handle "parent directory" / ".." alias
    if is_parent {
        if let Ok(cwd) = std::env::current_dir() {
            if let Some(parent) = cwd.parent() {
                candidates.push(FuzzyCandidate {
                    path: parent.to_path_buf(),
                    display: format!("Parent directory ({})", parent.display()),
                    score: 10000,
                });
                return candidates;
            }
        }
    }

    // 2. Alias recognition for "home" / "home dir" / "~" / "/home"
    if is_home {
        if let Some(ref h) = home {
            candidates.push(FuzzyCandidate {
                path: h.clone(),
                display: format!("Home directory ({})", h.display()),
                score: 10000,
            });
        }
    }

    // 3. Multi-segment path resolution (e.g. Coding/Personal Projects or Coding/Personal\ Projects)
    let is_multi_segment = (clean_target.contains('/') || clean_target.contains('\\')) && !is_home;
    if is_multi_segment {
        if let Some(multi_path) = resolve_multi_segment_path(clean_target) {
            return vec![FuzzyCandidate {
                display: format!("{}", multi_path.display()),
                path: multi_path,
                score: 10000,
            }];
        }
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let matcher = SkimMatcherV2::default();
    let query_normalized = clean_target.replace(['/', '\\', '-'], " ");
    let query_lower = query_normalized.to_lowercase();

    // 4. Scan CWD for directories matching clean_target
    if let Ok(entries) = fs::read_dir(&cwd) {
        for entry in entries.flatten() {
            if entry.metadata().map(|m| m.is_dir()).unwrap_or(false) {
                let name = entry.file_name();
                let name_str = match name.to_str() {
                    Some(s) => s,
                    None => continue,
                };
                if name_str.starts_with('.') {
                    continue;
                }

                let name_normalized = name_str.replace('-', " ").to_lowercase();
                let base_score = if name_normalized == query_lower
                    || (name_normalized == "home" && is_home)
                {
                    9000
                } else if name_normalized.contains(&query_lower)
                    || query_lower.contains(&name_normalized)
                {
                    7000
                } else {
                    matcher
                        .fuzzy_match(&name_normalized, &query_lower)
                        .unwrap_or(0)
                };

                if base_score > 0 {
                    let path = entry.path();
                    if !candidates.iter().any(|c| c.path == path) {
                        candidates.push(FuzzyCandidate {
                            display: format!("./{}", name_str),
                            path,
                            score: base_score + 500,
                        });
                    }
                }
            }
        }
    }

    // 5. Fallback: Scan Home Directory (if not CWD)
    if let Some(ref h) = home {
        if cwd != *h {
            if let Ok(entries) = fs::read_dir(h) {
                for entry in entries.flatten() {
                    if entry.metadata().map(|m| m.is_dir()).unwrap_or(false) {
                        let name = entry.file_name();
                        let name_str = match name.to_str() {
                            Some(s) => s,
                            None => continue,
                        };
                        if name_str.starts_with('.') {
                            continue;
                        }

                        let name_normalized = name_str.replace('-', " ").to_lowercase();
                        let score = if name_normalized == query_lower {
                            8000
                        } else if name_normalized.contains(&query_lower)
                            || query_lower.contains(&name_normalized)
                        {
                            6000
                        } else {
                            matcher
                                .fuzzy_match(&name_normalized, &query_lower)
                                .unwrap_or(0)
                        };

                        if score > 0 {
                            let path = entry.path();
                            if !candidates.iter().any(|c| c.path == path) {
                                candidates.push(FuzzyCandidate {
                                    display: format!("~/{}", name_str),
                                    path,
                                    score,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort by score descending
    candidates.sort_by(|a, b| b.score.cmp(&a.score));
    candidates
}

pub fn resolve_fuzzy_path(target: &str) -> Option<PathBuf> {
    let candidates = resolve_fuzzy_candidates(target);
    candidates.first().map(|c| c.path.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_home_alias() {
        assert!(is_home_alias("home"));
        assert!(is_home_alias("/home"));
        assert!(is_home_alias("/home/"));
        assert!(is_home_alias("home dir"));
        assert!(is_home_alias("home directory"));
        assert!(is_home_alias("~"));
    }

    #[test]
    fn test_home_candidates() {
        let candidates = resolve_fuzzy_candidates("/home");
        assert!(!candidates.is_empty());
        assert_eq!(candidates[0].score, 10000);
        assert!(candidates[0].display.contains("Home directory"));
        assert_eq!(candidates[0].path, dirs::home_dir().unwrap());
    }

    #[test]
    fn test_parent_alias() {
        assert!(is_parent_alias("parent directory"));
        assert!(is_parent_alias(".."));
        assert!(is_parent_alias("go back"));
        let candidates = resolve_fuzzy_candidates("parent directory");
        assert!(!candidates.is_empty());
        assert!(candidates[0].display.contains("Parent directory"));
    }

    #[test]
    fn test_multi_segment_path() {
        let res = resolve_multi_segment_path("src");
        assert!(res.is_some());
        assert!(res.unwrap().ends_with("src"));
    }

    #[test]
    fn test_multi_segment_candidates() {
        let candidates = resolve_fuzzy_candidates("Coding/Personal Projects");
        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].path.ends_with("Coding/Personal Projects"));
    }
}
