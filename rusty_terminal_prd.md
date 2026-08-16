# Rusty Terminal — Full Vision PRD (Unphased)

## How to read this document

Every layer below is specced in equal detail, as requested. But equal detail is not the same as equal buildability, and pretending otherwise is how you end up with a document that looks finished and ships something that deletes a production database with full confidence. So each layer has three parts: **Spec** (what it does), **Mechanism** (how, concretely), **Status** (buildable now / buildable with caveats / depends on unsolved capability). Status is not padding — it's the part of the doc that tells an engineer whether they're implementing a known pattern or doing original research. Skipping it doesn't make the underlying problem solved, it just hides where the doc is lying.

---

## 1. Problem Statement

Developers already think in goals for routine tasks — the friction is command recall for infrequent operations and context-switching across tools during diagnosis, not an inability to conceive of goals. This document specs a system that spans from "solves the recall problem" (well-understood) to "autonomously diagnoses and fixes production incidents" (not well-understood, by anyone, on any product, at acceptable reliability today). Both are described below. They are not the same kind of claim.

---

## 2. Layer 1 — Terminal Engine

**Spec:** Full terminal emulator. Shell integration (bash/zsh/fish/PowerShell/cmd), tabs, split panes, search, history, themes, session restore.

**Mechanism:** Wrap an existing PTY/renderer implementation rather than write a new emulator from scratch — Alacritty's and WezTerm's rendering pipelines are open-source and solve font rendering, ligatures, GPU acceleration, and cross-platform PTY handling, which are multi-year problems if built fresh.

**Status:** Buildable now. Not a differentiator — table stakes. Every hour spent here beyond "good enough" is an hour not spent on what actually makes Rusty different from a themed terminal.

---

## 3. Layer 2 — Intent Engine

**Spec:** Natural language → structured intent → command.

**Mechanism:** LLM function-calling against a fixed schema of known intents (`open_folder`, `docker_list`, `git_status`, etc.), with the model constrained to emit only commands within scanned, whitelisted binaries/tools present on the user's system.

**Status:** Buildable now for read-only and idempotent operations — this is a solved pattern, multiple shipped products already do it (Copilot CLI, Amazon Q, ShellGPT). Not buildable at production-safe reliability for **destructive or state-mutating** commands without Layer 5's rollback model existing first (see Layer 5). Claiming otherwise here would just be restating the earlier mistake with different words.

---

## 4. Layer 3 — Context Engine

**Spec:** System-wide awareness — current project, branch, running services, cloud environment, open ports — used to disambiguate intent before generating actions.

**Mechanism:**
- *Static context* (buildable now): parse `.git`, `package.json`/`requirements.txt`/`Cargo.toml`, `docker-compose.yml`, local env files. This tells you what stack you're likely in.
- *Live local context* (buildable with caveats): query running processes, open ports, active Docker containers via existing local APIs (`docker ps`, `lsof`, `systemctl status`). Reliable, but scope creep risk — this can silently turn into "index everything on the machine," which is both a privacy problem and a maintenance burden.
- *Live cloud/infra context* (depends on unsolved capability, practically speaking): correctly inferring *why* a system is in a given state — not just that a pod is failing, but which of five plausible causes is the actual one — requires the kind of state-correlation engineering that companies like Datadog and PagerDuty have built dedicated, large teams around for years. This is not "call the AWS SDK and read the response." It's building an inference layer on top of that data that's actually right often enough to trust. There is no shortcut here; wanting it in the doc doesn't produce the engineering team or the multi-year dataset that made those companies' versions of this work.

**Status:** Static and live-local context: buildable. Live cloud/infra causal inference: this is the single biggest unsolved item in the entire document, full stop. It should be written into the doc as a stated research bet, not as a shipped feature, because that's what it is.

---

## 5. Layer 4 — Planning Engine

**Spec:** Given a goal, decompose into an ordered, dependency-aware sequence of steps before any execution occurs.

**Mechanism:** LLM-driven task decomposition, using Context Engine output as grounding, producing a step list with preconditions/postconditions per step.

**Status:** Buildable for short, well-bounded plans (3-5 steps, all reversible, all locally scoped — e.g., "set up a new dev environment"). Not reliable for long-horizon plans with real-world side effects (deploy pipelines, infra changes) — current agentic systems across the industry degrade sharply past roughly 5-10 sequential steps when errors compound, and a deploy pipeline is exactly a compounding-error domain (a bad step 2 makes steps 3-6 wrong in ways the planner won't necessarily detect). This isn't a claim about Rusty specifically; it's the current state of agentic LLM systems generally, and nothing in this doc changes that by asserting a bigger scope.

---

## 6. Layer 5 — Security / Safety Engine

**Spec:** Every planned action is assessed for risk before execution; user approves before anything runs.

**Mechanism, corrected from the original doc:** A single numeric or label-based "risk score" per command (e.g., `rm -rf` = CRITICAL) is not a safety mechanism — it's a false sense of one, because risk is sequence- and state-dependent (`kubectl delete pod` is safe with 5 replicas, catastrophic with 1). The actual mechanism needed:
1. **Command-level allowlist** for auto-suggestable actions (read-only, idempotent) — real and buildable now.
2. **Sequence-level review**: before executing a multi-step plan, surface the full literal command list (not a natural-language summary) for explicit approval, so state-dependent danger is visible to a human even if the risk-scorer can't reason about it.
3. **Rollback/transaction model**: for anything destructive or infra-mutating, there needs to be a way to reverse the action — snapshotting Terraform state, Docker volume backups before mutation, etc. — before that action is allowed to be automated at all. **This does not exist in any form today in this vision doc.** Building it is a real, substantial engineering project on its own (one per action type — Terraform rollback and Docker volume rollback are not the same problem), and none of the destructive-action features in this document should ship before it does, no matter what the target scope is.

**Status:** Allowlist + sequence review: buildable now. Rollback/transaction model: buildable, but is its own multi-month project, not a subcomponent of "the security layer." It should be tracked and staffed as such, not folded into a bullet point.

---

## 7. Layer 6 — Execution Engine

**Spec:** OS-specific command dispatch (macOS/Linux/Windows).

**Mechanism:** Standard subprocess execution with per-OS path/binary resolution.

**Status:** Buildable now. Tedious, not novel.

---

## 8. Layer 7 — Observation Engine

**Spec:** After execution, monitor logs, health checks, exit codes, resource usage; detect failure and identify root cause.

**Mechanism:** Log tailing and exit-code checking are buildable now — genuinely simple. "Identify root cause" is the same unsolved problem as Layer 3's live-infra inference: pattern-matching a log line to "container unhealthy" is doable; correctly distinguishing that from five other plausible causes with the same symptom, reliably, is not solved technology today.

**Status:** Monitoring/alerting: buildable now. Automated root-cause diagnosis: same caveat as Layer 3 — this is a stated bet, not a shipped capability, until there's evidence otherwise.

---

## 9. Layer 8 — Learning Engine

**Spec:** Learn repeated command patterns and preferred workflows locally, no cloud dependency.

**Mechanism:** Local usage-frequency tracking, mapping repeated multi-command sequences to user-defined shortcuts.

**Status:** Buildable now, genuinely low-risk since it's local-only and additive (worst case: a bad suggestion, not a bad action). The lowest-controversy layer in the whole doc.

---

## 10. Plugin Ecosystem

**Spec:** Git, Docker, Kubernetes, Database, SSH plugins exposing domain-specific intents.

**Mechanism:** Each plugin defines its own intent schema and maps to the underlying CLI/SDK.

**Status:** Git and Docker plugins: buildable now, following the same tiering as Layer 2 (read-only/idempotent auto-suggested, destructive gated behind Layer 5's unresolved rollback model). Kubernetes, Database, and SSH plugins: buildable in the same pattern, but each one meaningfully raises blast radius (a DB plugin with write access, an SSH plugin with prod access) and should not ship before the rollback model exists for that specific action class — an SSH plugin with no undo mechanism for a bad remote command is not a smaller risk than the cloud layer, it's the same risk with a different name.

---

## 11. Cloud Layer (AWS / Azure / GCP / Cloudflare)

**Spec:** Cloud plugin layer routes AI-derived intent through official SDKs; credentials never touch the AI model directly.

**Mechanism:** Intent → cloud-specific plugin → official SDK call, using locally stored/scoped credentials.

**Status:** The "never send credentials to the AI" design is genuinely correct and worth keeping as a hard architectural rule, not a suggestion. But four SDKs, four auth/permission models, and four sets of provider-specific failure modes is a large, ongoing maintenance surface with, as of this document, zero validated user demand. Buildable, but sequence it behind at least one cloud provider proving out demand — building all four before any user has asked for one is capacity spent on a guess.

---

## 12. Agent Mode ("Create a Flask API and deploy it to AWS")

**Spec:** Fully autonomous goal-to-deployment execution: generate code, infra, build, deploy, verify, with only approval gates.

**Mechanism:** Chains Layers 2 through 7 end to end with no human step-by-step intervention beyond plan approval.

**Status:** This is the most severe unsolved-dependency stack in the document. It requires Layer 3's causal infra-inference, Layer 4's long-horizon planning, and Layer 5's rollback model to *all* independently work — and their failure modes compound rather than cancel out. A planner that's 90% reliable per step, chained across a 10-step deploy, is well under 50% reliable end to end. This is a straightforward math problem, not a pessimistic guess. Do not schedule this as a feature with a target date; schedule it as "revisit once Layers 3, 4, and 5 each have independent evidence of reliability," because right now none of them do.

---

## 13. "The Final End-State" (autonomous production incident remediation)

**Spec:** User reports "production is failing," Rusty investigates, diagnoses, proposes and executes fixes, and verifies resolution — autonomously.

**Status:** This is the highest-liability feature achievable in this entire document and it should be treated as one, explicitly, in writing, not softened into "the vision." An autonomous system taking real action against production infrastructure based on an LLM's diagnosis, before Layer 5's rollback model exists and before Layer 3/4/7's reliability is independently evidenced, is the scenario where this product causes an actual outage or data-loss incident and the postmortem says "the AI took an action nobody reviewed." Keeping this in the document as the "end-state" is fine as a north star; keeping it un-flagged as a liability item, the way the original doc did, is not a documentation style choice, it's a missing risk disclosure.

---

## 14. Cross-cutting gaps not covered by any layer above

- **No wedge/first-user definition.** Still true from the last pass — "developers" is not a target user.
- **No trust-calibration mechanism specified for how much detail/confidence is shown to the user per suggestion**, beyond "show the literal command." For agent-mode and remediation features this is a much bigger unsolved design problem than a terminal UI question — how does a user calibrate trust in a system whose failure mode is confident and wrong?
- **No metric anywhere in the original doc for what "working" means** — this version doesn't fix that either, because you can't set a target reliability number for capabilities (Layers 3/4/7 causal inference, long-horizon planning) that don't have a working reference implementation yet to benchmark against.

This is not a gap list to be resolved with more paragraphs. It's a gap list that gets resolved by building the buildable layers, measuring them, and only then finding out whether the unsolved layers are tractable at all — which is the argument for phasing that this version of the document was explicitly asked not to make. That request doesn't make the dependency graph go away; it just means the dependency graph is now written down instead of hidden.
