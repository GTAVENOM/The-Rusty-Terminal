//! Domain plugins. Each plugin owns a set of intents, declares the tier of
//! every one of them, and can bias its relevance based on static project
//! context (marker files present in the working directory).
//!
//! Plugins never introduce Tier-3 intents — the `no_tier3_intents` test in
//! each plugin asserts this, and the schema/tier tables are the source of
//! truth.

pub mod docker;
pub mod git;

use crate::context::scanner::ProjectContext;

/// A plugin contributes intent names to the toolset and a relevance
/// signal derived from static context.
pub trait Plugin {
    /// Stable plugin name (used in the context block sent to the model).
    #[allow(dead_code)]
    fn name(&self) -> &'static str;

    /// Names of the intents this plugin owns, in the order they should be
    /// offered to the model.
    fn intent_names(&self) -> &'static [&'static str];

    /// True when the working directory looks like this plugin's domain
    /// (e.g. a `.git` directory is present).
    fn is_relevant(&self, context: &ProjectContext) -> bool;

    /// A one-line hint appended to the model's context block when this
    /// plugin is relevant.
    fn context_hint(&self) -> &'static str;
}

/// All registered plugins.
pub fn all() -> Vec<Box<dyn Plugin>> {
    vec![Box::new(git::GitPlugin), Box::new(docker::DockerPlugin)]
}

/// Order intent names so relevant plugins' intents come first; intents not
/// owned by any plugin keep their original relative order at the end.
pub fn prioritize_intents(
    all_names: &[&str],
    context: &ProjectContext,
) -> Vec<String> {
    let plugins = all();
    let mut relevant: Vec<String> = Vec::new();
    let mut rest: Vec<String> = Vec::new();

    for name in all_names {
        let owner = plugins
            .iter()
            .find(|p| p.intent_names().contains(name));
        match owner {
            Some(plugin) if plugin.is_relevant(context) => {
                relevant.push(name.to_string())
            },
            _ => rest.push(name.to_string()),
        }
    }
    relevant.extend(rest);
    relevant
}

/// Context hints from every relevant plugin, for the model's context block.
pub fn context_hints(context: &ProjectContext) -> Vec<&'static str> {
    all()
        .iter()
        .filter(|p| p.is_relevant(context))
        .map(|p| p.context_hint())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_with(markers: &[&str]) -> ProjectContext {
        ProjectContext {
            markers: markers.iter().map(|m| (m.to_string(), 0)).collect(),
        }
    }

    #[test]
    fn git_intents_come_first_in_a_repo() {
        let names = [
            "docker_ps",
            "list_files",
            "git_status",
            "docker_compose_up",
            "git_log",
        ];
        let ordered = prioritize_intents(&names, &ctx_with(&[".git"]));
        assert_eq!(ordered[0], "git_status");
        assert_eq!(ordered[1], "git_log");
        // Non-git intents follow, order preserved.
        assert_eq!(
            &ordered[2..],
            &["docker_ps", "list_files", "docker_compose_up"]
        );
    }

    #[test]
    fn docker_intents_come_first_in_a_compose_project() {
        let names = ["git_status", "docker_ps", "list_files"];
        let ordered =
            prioritize_intents(&names, &ctx_with(&["docker-compose.yml"]));
        assert_eq!(ordered[0], "docker_ps");
    }

    #[test]
    fn no_markers_preserves_original_order() {
        let names = ["git_status", "docker_ps", "list_files"];
        let ordered = prioritize_intents(&names, &ctx_with(&[]));
        assert_eq!(ordered, vec!["git_status", "docker_ps", "list_files"]);
    }

    #[test]
    fn hints_only_for_relevant_plugins() {
        assert!(context_hints(&ctx_with(&[])).is_empty());
        let hints = context_hints(&ctx_with(&[".git"]));
        assert_eq!(hints.len(), 1);
        assert!(hints[0].contains("git"));
    }
}
