//! Docker plugin.
//!
//! Contributes the docker-family intents (ps, logs, compose up) and biases
//! their relevance when a `docker-compose.yml` marker is present. All
//! intents are Tier 1 or Tier 2 — destructive docker forms (`system
//! prune`, `rm -f`, `volume rm`, `rmi`, …) are absent from the schema.

use super::Plugin;
use crate::context::scanner::ProjectContext;

pub struct DockerPlugin;

impl Plugin for DockerPlugin {
    fn name(&self) -> &'static str {
        "docker"
    }

    fn intent_names(&self) -> &'static [&'static str] {
        &["docker_ps", "docker_logs", "docker_compose_up"]
    }

    fn is_relevant(&self, context: &ProjectContext) -> bool {
        context.has_docker_compose()
    }

    fn context_hint(&self) -> &'static str {
        "A docker-compose.yml is present — favor docker intents where they \
         fit."
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::schema::Intent;
    use crate::safety::tier_classifier::Tier;

    #[test]
    fn no_docker_intent_is_tier3() {
        assert_eq!(
            Intent::DockerPs(Default::default()).tier(),
            Tier::ReadOnly
        );
        assert_eq!(
            Intent::DockerLogs(crate::intent::schema::DockerLogsArgs {
                container: "x".into(),
                tail: None,
                follow: false,
            })
            .tier(),
            Tier::ReadOnly
        );
        assert_eq!(
            Intent::DockerComposeUp(Default::default()).tier(),
            Tier::Idempotent
        );
        assert!(!DockerPlugin.intent_names().contains(&"git_status"));
    }

    #[test]
    fn relevance_tracks_compose_marker() {
        let plugin = DockerPlugin;
        let empty = ProjectContext { markers: vec![] };
        let with_compose = ProjectContext {
            markers: vec![("docker-compose.yml".to_string(), 0)],
        };
        assert!(!plugin.is_relevant(&empty));
        assert!(plugin.is_relevant(&with_compose));
    }
}
