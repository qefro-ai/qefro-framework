# Restaurant

Built-in Qefro application. Runtime source: `examples/restaurant`.

Entities, workflows, permissions, and the operations dashboard are registered from that crate. Framework core contains no restaurant business rules.

```bash
qefro app install restaurant
qefro migrate --app restaurant
qefro dev --app restaurant
```
