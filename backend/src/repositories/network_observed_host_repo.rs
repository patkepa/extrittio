use chrono::NaiveDateTime;
use diesel::PgConnection;
use diesel::prelude::*;

use crate::db::models::{NetworkObservedHost, NewNetworkObservedHost};
use crate::db::schema::network_observed_hosts;
use crate::services::device_connections::ObservedNetworkHost;

pub fn list_recent_for_analyzers(
    conn: &mut PgConnection,
    tenant_id: &str,
    analyzer_device_ids: &[String],
    cutoff: NaiveDateTime,
) -> Result<Vec<NetworkObservedHost>, diesel::result::Error> {
    if analyzer_device_ids.is_empty() {
        return Ok(Vec::new());
    }

    network_observed_hosts::table
        .filter(network_observed_hosts::tenant_id.eq(tenant_id))
        .filter(network_observed_hosts::analyzer_device_id.eq_any(analyzer_device_ids))
        .filter(network_observed_hosts::last_seen_at.ge(cutoff))
        .order((
            network_observed_hosts::analyzer_device_id.asc(),
            network_observed_hosts::status.asc(),
            network_observed_hosts::last_seen_at.desc(),
        ))
        .select(NetworkObservedHost::as_select())
        .load(conn)
}

pub fn replace_active_scan(
    conn: &mut PgConnection,
    tenant_id: &str,
    analyzer_device_id: &str,
    hosts: &[ObservedNetworkHost],
    seen_at: NaiveDateTime,
) -> Result<(), diesel::result::Error> {
    let active_host_keys: Vec<String> = hosts.iter().map(|host| host.host_key.clone()).collect();

    for host in hosts {
        let new_host = NewNetworkObservedHost {
            tenant_id: tenant_id.to_string(),
            analyzer_device_id: analyzer_device_id.to_string(),
            host_key: host.host_key.clone(),
            label: host.label.clone(),
            address: host.address.clone(),
            device_type: host.device_type.clone(),
            source: host.source.clone(),
            status: "active".to_string(),
            first_seen_at: seen_at,
            last_seen_at: seen_at,
            updated_at: seen_at,
        };

        diesel::insert_into(network_observed_hosts::table)
            .values(&new_host)
            .on_conflict((
                network_observed_hosts::analyzer_device_id,
                network_observed_hosts::host_key,
            ))
            .do_update()
            .set((
                network_observed_hosts::label.eq(&host.label),
                network_observed_hosts::address.eq(&host.address),
                network_observed_hosts::device_type.eq(&host.device_type),
                network_observed_hosts::source.eq(&host.source),
                network_observed_hosts::status.eq("active"),
                network_observed_hosts::last_seen_at.eq(seen_at),
                network_observed_hosts::updated_at.eq(seen_at),
            ))
            .execute(conn)?;
    }

    if active_host_keys.is_empty() {
        diesel::update(
            network_observed_hosts::table
                .filter(network_observed_hosts::tenant_id.eq(tenant_id))
                .filter(network_observed_hosts::analyzer_device_id.eq(analyzer_device_id))
                .filter(network_observed_hosts::status.ne("inactive")),
        )
        .set((
            network_observed_hosts::status.eq("inactive"),
            network_observed_hosts::updated_at.eq(seen_at),
        ))
        .execute(conn)?;
    } else {
        diesel::update(
            network_observed_hosts::table
                .filter(network_observed_hosts::tenant_id.eq(tenant_id))
                .filter(network_observed_hosts::analyzer_device_id.eq(analyzer_device_id))
                .filter(network_observed_hosts::status.ne("inactive"))
                .filter(network_observed_hosts::host_key.ne_all(active_host_keys)),
        )
        .set((
            network_observed_hosts::status.eq("inactive"),
            network_observed_hosts::updated_at.eq(seen_at),
        ))
        .execute(conn)?;
    }

    Ok(())
}

pub fn delete_older_than(
    conn: &mut PgConnection,
    cutoff: NaiveDateTime,
) -> Result<usize, diesel::result::Error> {
    diesel::delete(
        network_observed_hosts::table.filter(network_observed_hosts::last_seen_at.lt(cutoff)),
    )
    .execute(conn)
}
