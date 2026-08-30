# Analytics

Status: the blueprint-driven explorer and its bounded query API are implemented.
Saved dashboards, generic stored rollups, formulas, and external BI integration
are not part of the current feature.

## Data source

Analytics discovers numeric `float64` and `int64` stream fields from each
tenant's latest published blueprint revisions. Accepted contract events are
projected into typed `device_metric_samples` rows in both PostgreSQL and Turso.
The query adapters aggregate those rows in the database and return bounded,
chart-ready buckets.

Fixed legacy telemetry columns are not automatically an analytics catalog. A
metric appears only when it is declared in a published blueprint and compatible
devices have emitted contract events for that stream and field path.

## User experience

The `/analytics` console route lets an operator choose:

- a device type, fleet, and explicit device subset;
- a blueprint metric;
- a one-hour to 30-day time range;
- an automatic or explicit bucket;
- separate device series, fleet mean, or fleet mean with minimum/maximum range;
- equal-device or sample-weighted means.

The result includes the effective bucket, compatible-device count, coverage,
summary statistics, per-device statistics, and chart series. The API also
supports `latest_ranking`, although the current console does not expose that
mode.

## API

```text
GET  /api/v1/analytics/catalog
POST /api/v1/analytics/query
```

The catalog returns the structured metric identity:

```text
blueprint_id + stream_key + field_path
```

Labels are presentation metadata and are not metric identity. The query body
contains the device scope, structured metric selector, RFC 3339 start and end,
optional bucket, series mode, weighting, and optional point limit. The complete
schema is in [`api/openapi.json`](../../api/openapi.json).

Values within one scope category are ORed. Populated categories are combined,
so a fleet filter plus a device-type filter selects devices matching both. The
server resolves the final set under the authenticated tenant; it does not accept
a trusted tenant identifier from the request.

Current fleet and device-type membership is used for historical queries. The
system does not keep event-time membership history.

## Aggregation semantics

- A device bucket average is weighted by its samples.
- `equal_device` gives each reporting device's bucket average equal weight.
- `sample` weights device bucket averages by their sample counts.
- Fleet minimum and maximum use the extrema reported in each device bucket.
- Missing buckets remain missing; there is no carry-forward fill.
- Coverage is the number of reporting compatible devices divided by the number
  of compatible devices in the selected scope.

All storage and bucketing use UTC. Timezone conversion is a presentation
concern.

## Enforced limits

The backend currently enforces:

| Limit | Value |
| --- | ---: |
| Maximum compatible devices | 50 |
| Values in each scope filter | 100 |
| Maximum time range | 366 days |
| Default points per series | 1,200 |
| Hard points per series | 2,000 |
| Total result rows | 100,000 |

Allowed buckets are 60, 300, 900, 3,600, 21,600, and 86,400 seconds. Automatic
selection chooses the smallest allowed bucket that fits the requested point
budget. An explicit bucket that would exceed the budget is rejected with a
suggested minimum.

Analytics requires `telemetry.read`. The console additionally requires the
read permissions used to populate device, device-type, and fleet selectors.
