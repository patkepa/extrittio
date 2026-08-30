# Extrittio Analytics Architecture

**Status:** Blueprint-driven explorer implemented; dashboards and generic rollups proposed
**Date:** 2026-08-30  
**Scope:** Tenant-scoped fleet and device telemetry analytics in the web console

## Decision summary

Build Analytics as a native Extrittio domain and a new **Analytics** route in the
frontend's **General** navigation group. The experience should feel like a
focused Grafana explorer and dashboard builder, but it should not embed Grafana
or expose arbitrary SQL.

The architecture should:

1. use device-blueprint stream fields as the source of metric identity, type,
   unit, permitted aggregates, and presentation hints;
2. persist typed metric points and mergeable per-device rollups alongside the
   canonical raw event;
3. resolve device type, fleet, and explicit-device scopes on the server under
   the authenticated tenant;
4. execute bounded aggregation and downsampling in the database, returning
   chart-ready series rather than raw telemetry rows;
5. support PostgreSQL and Turso through one analytics repository port;
6. deliver an explorer first, then saved multi-panel dashboards without
   changing the query contract.

The first release runs against typed `device_metric_samples` projected from
validated device events. The metric catalog comes from the latest published
blueprint revisions, so adding a numeric stream field does not require an
Analytics backend or frontend change. Fixed telemetry columns remain a legacy
ingestion model and are not the Analytics metric registry.

## Product outcome

An operator can:

- select a device type, one or more fleets, or explicit devices;
- select a compatible metric such as temperature;
- view every device as its own series;
- view a combined fleet mean, minimum/maximum envelope, or both;
- compare latest, average, minimum, maximum, sample count, and data coverage;
- choose a time range, bucket size, refresh interval, and chart style;
- save the result as a reusable panel and arrange panels on a dashboard;
- share a dashboard with other authorized users in the same tenant.

The initial release is an operational analytics tool, not a general business
intelligence system. Arbitrary SQL, cross-tenant data, joins to external data,
user-supplied scripts, and an unrestricted formula language are out of scope.

## Existing foundation and gaps

Extrittio already has useful building blocks:

- validated device events project every declared stream field into typed,
  tenant-scoped `device_metric_samples` rows;
- raw legacy telemetry remains tenant-scoped and monthly partitioned in
  PostgreSQL;
- Turso exposes the same telemetry repository behavior;
- devices already reference a device type and optional fleet;
- the frontend already uses uPlot and TanStack Query;
- `telemetry.read`, `devices.read`, `device_types.read`, and `fleets.read`
  permissions already exist.

The important gaps are:

- generic blueprint metrics do not yet have tiered rollups;
- custom metadata is stored as untyped JSON and interpreted in React;
- metric profiles, labels, units, and chart hints are hard-coded in
  `telemetry-profiles.ts`;
- current telemetry APIs are single-device list endpoints and can return raw
  records, but cannot express a fleet aggregation safely;
- current rollups contain average/minimum/maximum but not the sums needed to
  merge buckets without statistical errors.

The in-progress device-contract model closes the semantic gap. Its stream
fields already define a stable stream plus field path, value type, label, unit,
allowed aggregates, retention, and chart hints. Analytics should consume that
model rather than create a separate metric registry.

## System shape

```text
Analytics page
    |
    | catalog, query, dashboard APIs
    v
Analytics route -> service/query planner -> AnalyticsRepository
                                           |              |
                                           v              v
                                   PostgreSQL adapter   Turso adapter
                                           |              |
                        +------------------+--------------+
                        |                  |              |
                  metric latest       metric points   metric rollups
                        ^                  ^              ^
                        |                  |              |
                 typed projection <- validated event -> rollup worker
                                           |
                                           v
                                     canonical raw event
```

Handlers own HTTP parsing and responses. The analytics service owns
authorization, scope resolution, compatibility checks, query planning, point
budgets, and result semantics. Repositories own database-specific query and
aggregation details. PostgreSQL Diesel work must remain behind the existing
blocking executor boundary.

## Metric identity and compatibility

### Identity

A metric is identified by:

```text
blueprint + stream key + field path
```

Labels are presentation and may change without changing identity. The public
query request should use a structured reference rather than concatenate or put
a JSON pointer into a URL path:

```json
{
  "blueprint_id": "cold-room-sensor",
  "stream_key": "environment",
  "field_path": "/temperature"
}
```

Legacy telemetry can receive generated compatibility blueprints in a follow-up
adapter; Analytics itself does not special-case legacy field names.

### Comparison across revisions or blueprints

Two fields can be compared only when they have:

- the same semantic key, or an explicitly selected common field;
- compatible scalar types;
- the same unit in the first release;
- the requested aggregate in both field definitions.

Do not infer that identically named labels are compatible. Unit conversion can
be added later through a bounded platform-owned conversion catalog. Until then,
mixed units should produce a validation error with the incompatible devices
listed.

The catalog API reports both selected-device coverage and compatibility, so the
UI can show, for example, `temperature · 18/20 devices` before executing the
query.

## Query model

### Scope

The query scope supports:

- one or more device type or blueprint IDs;
- one or more fleet IDs;
- explicit device IDs;
- optionally, device status.

Values within a category are ORed; populated categories are ANDed. For example,
two fleets plus one device type means devices in either fleet that also have
that type. The service resolves the final device set under `tenant_id`; the
client never supplies a trusted tenant or an already-authorized device set.

For the first release, type and fleet membership use the devices' current
membership even when querying historical data. The UI and API response must
label this behavior. Event-time membership requires a membership history or
dimension snapshot and should be a later explicit product decision.

### Series modes

The first release should support:

| Mode | Result |
| --- | --- |
| `per_device` | one line per selected device |
| `fleet_mean` | one equal-device-weighted mean line |
| `mean_and_range` | fleet mean plus minimum/maximum envelope |
| `latest_ranking` | latest value per device, sorted for comparison |

`per_device` and `mean_and_range` directly cover “combined or separately.” A
later release can split by fleet, device type, firmware, or another approved
dimension.

### Aggregation semantics

Aggregation names must have documented mathematical meaning:

- per-device bucket average is `sum(values) / sample_count`;
- the default fleet mean is the average of reporting devices' bucket averages,
  so a sensor publishing every second does not outweigh one publishing every
  minute;
- an optional advanced sample-weighted mean is
  `sum(device sums) / sum(device sample counts)`;
- fleet minimum and maximum operate across all reporting values;
- coverage is `reporting devices / selected compatible devices` per bucket;
- missing data remains null by default;
- carry-forward is allowed only for compatible gauge metrics and must have a
  maximum staleness window.

The response should always say which weighting and fill policy was used.

### Time and resolution

Store and bucket in UTC; timezone affects display only. The planner chooses the
coarsest available resolution that remains within the requested point budget.
An explicit bucket may be requested if it is compatible with the source data.

Initial allowed buckets:

```text
raw, 1 minute, 5 minutes, 15 minutes, 1 hour, 6 hours, 1 day
```

The MVP can support raw plus 1-hour data because those sources already exist.
Generic metric storage should add tiered rollups, starting with 5-minute,
1-hour, and 1-day resolutions. The rollup worker reprocesses a trailing window
so late or out-of-order events converge, while queries merge the current
partial bucket from raw points.

Suggested defaults, subject to load testing:

- no more than 50 output series per panel;
- no more than 2,000 points per series;
- no more than 100,000 total points per response;
- no more than 20 panels per dashboard refresh wave;
- reject or automatically coarsen queries that exceed the budget;
- return the effective resolution and a warning whenever it differs from the
  requested resolution.

These are server limits, not merely frontend checks.

## Persistence

### Generic metric points

Follow the device-blueprint architecture's typed projection:

```text
device_metric_points
  tenant_id
  device_id
  contract_revision_id
  stream_key
  field_path
  occurred_at
  value_type
  numeric_value
  string_value
  boolean_value
  json_value
  event_id
  schema_key
```

A check constraint permits exactly one typed value column. Raw validated events
remain the replay/debugging source; metric points are the query and rule source.
Analytics v1 should query numeric gauge fields only. String/boolean `last` and
`count` panels can follow later.

For PostgreSQL, partition metric points by event time once volume justifies it.
Candidate indexes, to be confirmed with realistic `EXPLAIN (ANALYZE, BUFFERS)`
benchmarks, are:

```text
(tenant_id, device_id, stream_key, field_path, occurred_at DESC)
(tenant_id, stream_key, field_path, occurred_at DESC, device_id)
  INCLUDE (numeric_value)
```

Equality columns precede the time range. Do not add both indexes automatically
without measuring ingestion and storage cost. Turso uses equivalent composite
indexes with integer UTC timestamps and bounded retention.

### Mergeable rollups

Introduce a generic rollup table rather than extending the fixed rollup with a
column for every new field:

```text
device_metric_rollups
  tenant_id
  device_id
  stream_key
  field_path
  resolution_seconds
  bucket_start
  sample_count
  numeric_sum
  numeric_min
  numeric_max
  numeric_sum_squares
  first_value
  first_at
  last_value
  last_at
  updated_at

primary key
  (tenant_id, device_id, stream_key, field_path,
   resolution_seconds, bucket_start)
```

`sample_count`, `numeric_sum`, minimum, and maximum allow exact merging of
averages/minimums/maximums across time buckets and devices. Sum of squares makes
standard deviation possible without raw data. First/last timestamps ensure
correct merging of first/last values.

Do not average stored averages. The existing
`telemetry_rollups_hourly.avg_*` fields are sufficient for an equal-device
fleet mean for a single bucket, but they cannot produce every statistically
correct re-aggregation. The generic rollup is the long-term model.

Exact arbitrary percentiles are not mergeable from these columns. Add a
portable bounded histogram or quantile sketch only when percentile panels are
actually prioritized; do not make a PostgreSQL-only extension mandatory for a
feature that must also run on Turso.

### Latest values

`device_metric_latest` serves stat and ranking panels without scanning point
history. Its key is `(tenant_id, device_id, stream_key, field_path)`, and it is
updated transactionally with accepted points only when the incoming event is
newer than the stored value.

### Saved dashboards

Persist dashboard definitions, never query results:

```text
analytics_dashboards
  tenant_id
  id
  name
  description
  owner_user_id
  definition_json
  schema_version
  version
  created_at
  updated_at
```

`definition_json` contains the bounded grid layout, global time/refresh
settings, panel query specifications, and visualization options. The service
validates it against a versioned server-owned schema. `version` provides
optimistic concurrency, and a dashboard update writes one atomic document.
Keep panel count, title length, layout size, queries per panel, and JSON size
bounded.

If independent panel permissions or very large dashboards become necessary,
panels can be normalized later without changing the public definition schema.

## API

### Metric catalog

```http
GET /api/v1/analytics/catalog
```

Returns numeric fields from the tenant's latest published blueprint revisions,
with blueprint identity, stream key, field path, label, type, unit, declared
aggregates, presentation precision, and available bucket resolutions.

### Query

Use an idempotent POST because the query is structured and can exceed sensible
URL limits:

```http
POST /api/v1/analytics/query
```

Example request:

```json
{
  "scope": {
    "device_type_ids": [4],
    "fleet_ids": [2],
    "device_ids": []
  },
  "metric": {
    "blueprint_id": "cold-room-sensor",
    "stream_key": "environment",
    "field_path": "/temperature"
  },
  "from": "2026-08-23T00:00:00Z",
  "to": "2026-08-30T00:00:00Z",
  "bucket_seconds": null,
  "mode": "mean_and_range",
  "weighting": "equal_device",
  "max_points_per_series": 1200
}
```

Response shape:

```json
{
  "metric": {
    "blueprint_id": "cold-room-sensor",
    "stream_key": "environment",
    "field_path": "/temperature",
    "key": "environment./temperature",
    "label": "Temperature",
    "unit": "Cel",
    "value_type": "float64"
  },
  "effective": {
    "from": "...",
    "to": "...",
    "bucket_seconds": 3600,
    "source": "blueprint_metric_samples",
    "weighting": "equal_device",
    "fill": "none"
  },
  "scope": {
    "selected_devices": 20,
    "compatible_devices": 18
  },
  "series": [
    {
      "id": "fleet-mean",
      "label": "Fleet mean",
      "kind": "mean",
      "device_id": null,
      "points": [{"timestamp_ms": 1788048000000, "value": 21.4}],
      "stats": {
        "minimum": 18.2,
        "maximum": 25.1,
        "average": 21.7,
        "latest": 21.4,
        "sample_count": 18
      }
    }
  ],
  "coverage": [{
    "timestamp_ms": 1788048000000,
    "reporting_devices": 17,
    "selected_devices": 18
  }],
  "warnings": []
}
```

Epoch milliseconds keep chart payloads compact. Null values remain explicit.
The API should return a validation error with a suggested bucket when a query
cannot fit within cost limits.

### Dashboards

```http
GET    /api/v1/analytics/dashboards
POST   /api/v1/analytics/dashboards
GET    /api/v1/analytics/dashboards/{id}
PATCH  /api/v1/analytics/dashboards/{id}
DELETE /api/v1/analytics/dashboards/{id}
```

Dashboard reads require analytics/telemetry access. Creation, update, and
deletion require a management permission and emit audit events.

Every contract-affecting API change must regenerate `api/openapi.json` and
`apps/frontend/src/types/openapi.ts` through the documented generator.

## Backend domain design

Add a vertical domain under `crates/backend/src/domains/analytics/`:

```text
analytics.rs       HTTP requests/responses and routes
analytics_service.rs
repository.rs      AnalyticsRepository port
types.rs           backend-neutral query plan/results
```

Add PostgreSQL and Turso adapters under their existing persistence trees. The
domain service performs this sequence:

1. require the read permission;
2. validate time range, aggregate, fill, and point budgets;
3. resolve the scope to tenant-owned devices;
4. resolve the metric against assigned blueprint revisions or the legacy
   catalog;
5. reject or explicitly exclude incompatible devices;
6. choose raw/latest/rollup source and resolution;
7. execute an already-aggregated repository query;
8. attach coverage, source, effective resolution, and warnings;
9. record query duration and result-size metrics.

The repository must aggregate in SQL and stream/load only bounded result rows.
It must not load all raw telemetry into Rust for grouping. It must join devices
using both tenant and device identity and retain `tenant_id` in every grouping
key.

The existing telemetry repository remains the device-detail data API. Analytics
gets a separate port because its scope resolution, cost controls, and result
types are materially different.

## Frontend design

Add the lazy route `/analytics` with icon `chart` or `timeline-line-chart`,
navigation group `General`, and required read permissions. Place the feature in:

```text
apps/frontend/src/features/analytics/
  api/
  queries/
  model/
  components/
  page/
```

The page has three conceptual regions:

1. **Scope and time toolbar** — device type, fleet, devices, range, refresh;
2. **Query builder** — metric, aggregation, separate/combined mode, bucket;
3. **Canvas** — chart, legend, coverage, summary stats, and panel actions.

Use the current uPlot wrapper for time series. Add chart renderers behind a
small internal interface so stat, ranking table, and later heatmap panels share
the same query result. Avoid putting metric-specific logic into renderers.

Keep editable query state separate from the committed query. Apply changes on
an explicit Run action or a short debounce, canonicalize the request before
using it as a TanStack Query key, cancel superseded requests, and retain prior
data while refreshing. The URL should encode explorer state so an unsaved view
can be linked or restored; saved dashboards refer to server IDs.

The UI must make the following visible rather than hiding them in tooltips:

- compatible devices versus selected devices;
- current bucket and whether data came from raw points or rollups;
- missing/stale series;
- automatic coarsening;
- equal-device versus sample-weighted mean;
- truncation caused by series or point limits.

## Authorization and tenant isolation

For the explorer MVP, require `telemetry.read` plus the permissions needed to
discover the selected scopes. Introduce:

```text
analytics.read
analytics.manage
```

when saved dashboards are added. Seed these into built-in roles and add them to
frontend permission types. `analytics.read` controls dashboards/catalog/query;
`analytics.manage` controls create/update/delete and implies read.

Security invariants:

- ignore any client-provided tenant identifier;
- scope every device, metric, point, rollup, and dashboard query by the request
  context tenant;
- validate explicit device IDs instead of interpolating them into SQL;
- expose no arbitrary SQL, JSONPath, regex, or executable expression;
- bound queries before database execution;
- audit dashboard mutations and exports;
- prevent dashboard definitions from referencing unauthorized devices even if
  a user guesses an ID from another tenant.

## Performance and operations

The planner and worker should expose metrics for:

- analytics query count, latency, failures, and cancellations;
- chosen source and resolution;
- selected devices, series, and returned points;
- rejected/auto-coarsened queries;
- rollup lag by resolution;
- rollup rows upserted, worker duration, and consecutive failures;
- server-cache hit ratio if a cache is introduced.

Start without a server result cache. TanStack Query handles short-lived browser
reuse, while rollup indexes and bounded queries handle the primary load. Add a
small tenant-aware cache only after measurements show repeated dashboard
queries are a bottleneck; include the latest data watermark in invalidation.

Before release, generate realistic data for small, medium, and upper-bound
fleets and record `EXPLAIN (ANALYZE, BUFFERS)` for each query shape. Validate
both foreground latency and telemetry-ingestion impact before finalizing
indexes. On Turso/edge, use stricter default series and point budgets if soak
tests show foreground writes are affected.

## Delivery phases

### Phase 0 — freeze semantics and API types

- Treat the device blueprint stream field as the future metric catalog.
- Define scope intersection, metric compatibility, weighting, missing-data,
  membership, and time-bucket semantics.
- Add backend-neutral analytics request/result types and repository contract.
- Add cost-limit configuration with conservative defaults.

**Exit:** the same fixture produces identical expected query semantics in a
pure service-level test for PostgreSQL and Turso adapters.

### Phase 1 — blueprint-driven explorer

- Add `/analytics` to General navigation.
- Catalog arbitrary `float64` and `int64` fields from the latest published
  blueprint revisions, including labels, units, aggregates, and precision.
- Query typed `device_metric_samples` by blueprint, stream key, and field path.
- Support device-type, fleet, and explicit-device scopes.
- Support per-device, fleet mean, mean/range, and latest ranking modes.
- Return coverage, effective resolution, and warnings.
- Reuse uPlot and add scope/query controls; persist explorer state in the URL.

**Exit:** an operator can publish a blueprint with a new numeric field and
compare compatible devices separately or as a correctly weighted aggregate
without changing Analytics code.

### Phase 2 — tiered generic rollups

- Adapt legacy v1 telemetry into generated legacy blueprint fields.
- Add 5-minute, 1-hour, and 1-day rollups with trailing-window repair.
- Backfill generic temperature/humidity/battery rollups from retained raw data
  and translate older fixed hourly rollups where exact semantics permit.
- Replace hard-coded frontend telemetry profiles with blueprint presentation.

**Exit:** a new numeric device metric becomes queryable and chartable without a
backend migration or frontend code change.

### Phase 3 — saved dashboards

- Add dashboard storage, optimistic concurrency, permissions, and audit events.
- Add a responsive grid editor, panel duplication, titles/descriptions, global
  time range, refresh, and view/edit modes.
- Support time-series, stat, and latest-ranking panels.
- Add import/export of the versioned safe dashboard definition.

**Exit:** users can create, revisit, edit, and share tenant-scoped multi-panel
dashboards.

### Phase 4 — advanced analytics

Prioritize from observed use rather than implement everything at once:

- standard-deviation and deviation-from-fleet panels;
- distributions, histograms, and percentile sketches;
- comparisons between time windows;
- counter `rate` and `increase` semantics;
- safe formulas represented as a validated expression AST;
- CSV export through an asynchronous bounded job;
- annotations from alerts, OTA deployments, and commands;
- historical membership dimensions;
- anomaly detection built on stored aggregates, with explicit confidence and
  coverage.

Alerting remains owned by the rules engine. Analytics may help users create a
rule from a panel, but dashboards must not become a second alert evaluator.

## Testing strategy

### Domain and API

- tenant isolation for every scope type and guessed cross-tenant IDs;
- permission combinations for catalog, query, and dashboard mutations;
- scope OR/AND behavior;
- metric compatibility and unit mismatch rejection;
- equal-device and sample-weighted mean fixtures with unequal sample rates;
- min/max/range and coverage with missing devices;
- raw/rollup boundary and current partial bucket merging;
- late-event trailing-window repair;
- automatic bucket selection and hard point/series caps;
- invalid times, future times, zero-width ranges, and very large ranges;
- stable OpenAPI request and response schemas;
- PostgreSQL/Turso parity tests using identical fixtures.

### Frontend

- route and command-palette visibility by permission;
- scope selection and incompatible metric states;
- URL serialization and restoration;
- query cancellation and refresh behavior;
- null gaps, stale data, coverage, and warning rendering;
- chart legend toggling and accessible data-table fallback;
- responsive canvas and keyboard-operable panel controls;
- dashboard optimistic-concurrency conflict handling.

### Load and failure

- maximum supported devices, series, points, panels, and concurrent refreshes;
- worker catch-up after downtime and idempotent reprocessing;
- retention racing with rollup creation;
- database timeout/cancellation and client disconnect;
- rollup lag with continued ingestion;
- PostgreSQL partition pruning and Turso foreground-write latency.

## Acceptance criteria for the first product release

The initial Analytics release is complete when:

1. Analytics appears in General navigation only for authorized users.
2. A tenant can filter by device type, fleet, and explicit devices.
3. Any numeric field declared by a published blueprint can be graphed per
   device or combined without application code changes.
4. Fleet means are mathematically documented and tested with unequal reporting
   rates.
5. Every response reports coverage, effective resolution, and warnings.
6. Queries are tenant-scoped and enforced by server-side cost limits.
7. PostgreSQL and Turso return equivalent results for supported inputs.
8. Queries use bounded database-side buckets and never transfer raw samples to
   the browser; tiered rollups remain a follow-up for large historical ranges.
9. Metrics and query latency expose enough information to tune the feature.
10. The API is ready for saved panels without a breaking query redesign.

## Decisions to confirm

The recommended defaults are:

1. **Native engine, not embedded Grafana.** This keeps Extrittio tenancy,
   permissions, backend portability, and device semantics authoritative.
2. **Equal-device fleet mean.** It avoids high-frequency devices dominating a
   fleet comparison; expose sample-weighted mean as an advanced option.
3. **Current membership for v1.** Clearly label it and add historical
   dimensions only when that reporting requirement is confirmed.
4. **No implicit unit conversion.** Reject mixed units until a reviewed
   conversion catalog exists.
5. **Explorer before dashboard editor.** Prove query correctness and cost before
   multiplying concurrent panels.
6. **Generic custom metrics only through typed blueprint projection.** Do not
   build a second analytics system over arbitrary JSON.
