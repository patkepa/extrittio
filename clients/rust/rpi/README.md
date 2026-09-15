# Raspberry Pi contract client

Publish a blueprint with the `system` stream and optional numeric fields
`cpuTemperature`, `cpuUsagePercent`, `memoryUsagePercent`, `load1m`, `load5m`,
and `load15m`. Measurements unavailable on the host are omitted.

On the Pi, supply a compiled Pi binary and a bearer token in a private file:

```sh
./provision.sh --backend-url https://hub.example.test \
  --token-file /secure/path/token --blueprint-revision-id PUBLISHED_REVISION_ID \
  --device-name workshop-pi --binary ./extrittio-rpi
```

The backend must allow bearer authentication and the token must authorize device
creation and contract reads. Use HTTPS when sending credentials across a network.
The script requires Bash, curl, jq, sudo and systemd. It creates one device,
downloads its contract, checks identity/hash/schema locally, then installs the
binary and private contract file under `/opt/extrittio` and enables the service.
It does not create device types, infer endpoints, retry device creation or
overwrite an existing installation. Retain the printed device ID if a later
step fails; inspect that device before retrying to avoid duplicate registrations.

For manual provisioning, download the device contract response and run:

```sh
./extrittio-rpi --contract device-contract.json --check-contract
./extrittio-rpi --contract device-contract.json
```

The first command validates without opening a network session. The service uses
the contract endpoint and heartbeat policy. Deployments requiring client TLS
credentials should use manual provisioning and configure the supported
`--ca-cert`, `--client-cert`, and `--client-key` options in their service unit;
the installation script does not distribute TLS credentials.
