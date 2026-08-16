//! Git plugin.
//!
//! Contributes the git-family intents (status, log, diff, branch-list,
//! pull) and biases their relevance when a `.git` marker is present in
//! the working directory tree. All intents are Tier 1 or Tier 2 by
//! construction — Tier 3 forms (`push --force`, `reset --hard`, branch
//! deletion) are simply absent from the schema, per the safety contract.

use super::Plugin;
use crate::context::scanner::ProjectContext;

pub struct GitPlugin;

impl Plugin for GitPlugin {
    fn name(&self) -> &'static str {
        "git"
    }

    fn intent_names(&self) -> &'static [&'static str] {
        &[
            "git_status",
            "git_log",
            "git_diff",
            "git_branch_list",
            "git_pull",
        ]
    }

    fn is_relevant(&self, context: &ProjectContext) -> bool {
        context.has_git()
    }

    fn context_hint(&self) -> &'static str {
        "This is a git repository — favor git intents where they fit."
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::schema::Intent;
    use crate::safety::tier_classifier::Tier;
    use crate::intent::schema::{DockerLogsArgs, FindProcessByPortArgs};

    #[test]
    fn no_git_intent_is_tier3() {
        // Tier 1
        assert_eq!(Intent::GitStatus.tier(), Tier::ReadOnly);
        assert_eq!(
            Intent::GitLog(Default::default()).tier(),
            Tier::ReadOnly
        );
        // Tier 2
        assert_eq!(
            Intent::GitPull(Default::default()).tier(),
            Tier::Idempotent
        );

        // Sanity: plugin owns nothing outside its list.
        assert!(!GitPlugin.intent_names().contains(&"docker_ps"));

        // Sanity: the schema types compile without unrelated intents
        // gaining tier 3.
        assert_ne!(
            Intent::DockerLogs(DockerLogsArgs {
                container: "x".into(),
                tail: None,
                follow: false
            })
            .tier(),
            Tier::Destructive
        );
        assert_ne!(
            Intent::FindProcessByPort(FindProcessByPortArgs { port: 80 })
                .tier(),
            Tier::Destructive
        );
    }

    #[test]
    fn relevance_tracks_git_marker() {
        let plugin = GitPlugin;
        let empty = ProjectContext {
            markers: vec![],
        };
        let with_git = ProjectContext {
            markers: vec![(".git".to_string(), 0)],
        };
        let with_parent_git = ProjectContext {
            markers: vec![(".git".to_string(), 1)],
        };
        assert!(!plugin.is_relevant(&empty));
        assert!(plugin.is_relevant(&with_git));
        assert!(plugin.is_relevant(&with_parent_git));
    }
}
