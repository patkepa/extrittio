use diesel::result::{DatabaseErrorKind, Error};
use extrittio_backend_core::{ConstraintName, PersistenceError};

pub(crate) fn map_diesel_error(error: Error) -> PersistenceError {
    match error {
        Error::NotFound => PersistenceError::NotFound,
        Error::DatabaseError(kind, information) => {
            let constraint = semantic_constraint(information.constraint_name());
            match kind {
                DatabaseErrorKind::UniqueViolation => {
                    PersistenceError::UniqueViolation { constraint }
                }
                DatabaseErrorKind::ForeignKeyViolation => {
                    PersistenceError::ForeignKeyViolation { constraint }
                }
                DatabaseErrorKind::CheckViolation => {
                    PersistenceError::CheckViolation { constraint }
                }
                DatabaseErrorKind::UnableToSendCommand | DatabaseErrorKind::ClosedConnection => {
                    PersistenceError::Unavailable(information.message().to_owned())
                }
                DatabaseErrorKind::SerializationFailure => {
                    PersistenceError::Busy { retry_after: None }
                }
                _ => PersistenceError::Internal(information.message().to_owned()),
            }
        }
        other => PersistenceError::Internal(other.to_string()),
    }
}

/// Do not leak backend-generated constraint identifiers into the core contract.
/// Known schema constraints collapse to stable, business-facing identifiers;
/// unknown constraints retain only their semantic category.
fn semantic_constraint(database_name: Option<&str>) -> ConstraintName {
    let name = match database_name {
        Some("zones_pkey" | "zones_tenant_id_id_key") => "zones.id",
        Some("zones_tenant_name_unique") => "zones.tenant_name",
        Some("zones_tenant_id_fkey") => "zones.tenant_id",
        Some("roles_pkey" | "roles_tenant_id_id_key") => "roles.id",
        Some("roles_tenant_id_name_key") => "roles.tenant_name",
        Some("rule_conditions_zone_id_fkey" | "rule_conditions_tenant_zone_fk") => {
            "rule_conditions.zone_id"
        }
        _ => "database.constraint",
    };
    ConstraintName::new(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postgres_constraint_names_map_to_stable_names() {
        assert_eq!(semantic_constraint(Some("zones_pkey")).as_str(), "zones.id");
        assert_eq!(
            semantic_constraint(Some("zones_tenant_name_unique")).as_str(),
            "zones.tenant_name"
        );
        assert_eq!(
            semantic_constraint(Some("roles_tenant_id_name_key")).as_str(),
            "roles.tenant_name"
        );
        assert_eq!(
            semantic_constraint(Some("zones_tenant_id_fkey")).as_str(),
            "zones.tenant_id"
        );
        assert_eq!(
            semantic_constraint(Some("rule_conditions_tenant_zone_fk")).as_str(),
            "rule_conditions.zone_id"
        );
        assert_eq!(
            semantic_constraint(Some("generated_constraint_42")).as_str(),
            "database.constraint"
        );
        assert_eq!(semantic_constraint(None).as_str(), "database.constraint");
    }

    #[test]
    fn diesel_not_found_maps_to_the_stable_taxonomy() {
        assert!(matches!(
            map_diesel_error(Error::NotFound),
            PersistenceError::NotFound
        ));
    }
}
