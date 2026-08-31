# Security Policy

## Reporting a vulnerability

Email **security@qefro.ai** with a description of the issue, affected versions, and steps to reproduce. Do not file public GitHub issues for undisclosed vulnerabilities.

Please include:

- Qefro version or commit
- Deployment shape (single process vs API + worker)
- Whether the report involves tenant isolation, authentication, or XSS

Do not attach live credentials, production dumps, or exploit payloads that contain real tenant data.

## Supported versions

| Version | Supported |
| --- | --- |
| `main` / current 1.3.x | Yes |
| Older tagged releases | Security fixes are backported only for actively maintained tags |

## Response

We aim to acknowledge reports within a few business days and to ship a fix or mitigation for confirmed issues as soon as we can reasonably do so. We will not discuss exploit internals in public until a fix is available.

## Scope

The security boundary is **authentication → tenant context → RBAC / RowPolicy → EntityService**. UI, SDK, CLI, Studio, automation, import, export, and workers are untrusted entry points.

See [docs/security.md](docs/security.md) and [docs/security-audit.md](docs/security-audit.md).
