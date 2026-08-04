use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleRecord {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleDetails {
    pub role: RoleRecord,
    pub permissions: Vec<String>,
    pub user_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRoleRecord {
    pub name: String,
    pub description: Option<String>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateRoleRecord {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub permissions: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateRoleOutcome {
    Updated(RoleDetails),
    NotFound,
    SystemRole,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteRoleOutcome {
    Deleted,
    NotFound,
    SystemRole,
    InUse { name: String, user_count: i64 },
}
