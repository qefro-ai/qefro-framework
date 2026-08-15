# Studio permissions

The permission matrix is `PermissionGrant` metadata from the same `PermissionRegistry` used by `EntityService`. Publishing replaces grants for one entity in a live overlay. Admin bypass is unchanged.

Tenant Staff cannot open Studio. Editing the matrix requires `studio.manage_permissions`.

Business operations are listed separately from CRUD. Seeing an entity in Studio does not grant the caller those operations. Rust handlers stay source-managed; Studio only displays their `OperationDef` (name, roles, permission key).
