# Field permissions

Entity RBAC still gates create/read/update/delete/list. Field permissions then hide or reject individual fields.

Levels:

| Level | Meaning |
| --- | --- |
| 0 | Normal (anyone who passed entity RBAC) |
| 1 | Restricted |
| 2 | Sensitive |
| 3 | Highly sensitive |

```yaml
fields:
  salary:
    permission_level: 2
```

Roles receive a numeric grant. A grant of level 2 can read/write fields at level ≤ 2 on that entity.

```rust
FieldLevelGrant::new("HR", "Employee", 2)
FieldLevelGrant::new("Manager", "Employee", 1).read_only()
```

## Enforcement

Read: `EntityService` strips unauthorized fields before the response. The frontend never receives them.

Write: unauthorized keys in PATCH/POST return 403. Admin bypasses field levels after entity access.

Do not treat React hiding as authorization.
