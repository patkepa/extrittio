# Native contract runtime

Linux and Raspberry Pi clients require `--contract PATH`, containing the JSON
response from `GET /api/v1/devices/{id}/contract`. Provision the device from a
published blueprint revision before starting the client.

```sh
extrittio-client --contract /path/to/device-contract.json
extrittio-rpi --contract /path/to/device-contract.json
```

The runtime verifies the contract hash and device identity, takes heartbeat
timing from the contract, and validates every sample against its declared stream
schema before publishing. `--connect` can override the contract transport endpoint;
`--ca-cert`, `--client-cert` and `--client-key` provide deployment TLS credentials.
An explicit or embedded device ID must match the contract.

Implement `EventSource::sample` to return a `ContractEvent` with a stream key and
JSON payload. The runtime supplies envelope identity, timestamp and route. There
is no fixed telemetry message or publication fallback.

The Linux sample emits the `environment` stream with `temperature`, `humidity`
and `batteryLevel`. The Raspberry Pi sample emits the `system` stream with
`cpuTemperature`, `cpuUsagePercent`, `memoryUsagePercent`, `load1m`, `load5m` and
`load15m`. These are example payloads that must be declared in their blueprints,
not platform-reserved fields. Raspberry Pi measurements may be absent when a
sensor is unavailable, so their schema properties should be optional. CPU usage
also requires two successful snapshots.

The Raspberry Pi provisioning script creates a device from a published revision,
downloads its contract and validates it with `--check-contract` before installing
the service. See `../rpi/README.md` for authentication and installation details.
