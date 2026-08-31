# Dependency policy

Keep the crate graph small. EntityService, JWT auth, and PostgreSQL remain the runtime — do not add a second auth, ORM, or permission engine to “fix” a CVE.

Before adding a **direct** dependency, check:

| Question | Why |
| --- | --- |
| Is it maintained? | Last release, known owner, not abandoned |
| License | Compatible with MIT |
| Security history | Advisories, whether vulns were in optional features |
| Runtime exposure | Does production traffic execute this code, or is it build-only / test-only? |

Prefer:

- Feature-gating unused optional crates (`default-features = false`)
- Pinning a known-good version when a safe upgrade is not available
- Documenting residual risk in [security-audit.md](security-audit.md) rather than pretending a transitive advisory disappeared

`cargo audit` runs in CI as a non-blocking job. New findings should be triaged (upgrade, pin, or document) before they become an accepted residual.
