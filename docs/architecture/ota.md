# Firmware deployment protocol

OTA commands are created only by the firmware deployment endpoints. The `ota`
key is reserved in the desired-shadow API, including attempts to set it to null.
Every command includes `deployment_id`, `firmware_update_id`, `firmware_version`,
`firmware_url`, and a mandatory 64-character SHA-256 digest. SDKs reject missing
identity/integrity fields and oversized values instead of silently truncating them.

Uploaded objects use `/api/v1/ota-downloads/{grant}`. The grant is a signed,
purpose-specific JWT scoped to the tenant and firmware object, valid for seven
days. It is a bearer credential: keep URLs out of public logs and messages. It
does not grant user API access. Regular user downloads retain session/permission
checks. Retrigger deployment to refresh a grant for a device offline longer than
seven days. External HTTPS artifacts retain their registered URLs.

Deployment states progress through `pending`, `downloading`, `verifying`,
`installing`, `rebooting`, and `success` or `failed`. Reports must identify the
exact deployment and firmware. Backward transitions, unknown states, and updates
to terminal deployments are ignored. Only fresh OTA fields from a device report
are processed; unrelated reports cannot replay old status. Starting another
attempt marks an outstanding attempt failed as superseded. A terminal report
clears only its matching desired OTA command in the same database transaction.
PostgreSQL locks the shadow row before both OTA trigger and completion mutations.

## Native Linux, Raspberry Pi, and macOS clients

Installation preserves the old executable as `<executable>.ota-previous` and
atomically writes a synced installation journal and replacement executable.
Version metadata is bound to the installed file digest and survives restarting.
The binary directory must be writable, and the process supervisor must restart
the client on both successful and unsuccessful exits (`Restart=always` for
systemd; the existing LaunchAgent uses KeepAlive). Systemd units also need
`KillMode=process` so the bounded rollback watchdog survives the main client's
restart; the RPi provisioning script sets it. Apply this setting to existing
units and reload systemd before deploying. On macOS the watchdog starts in its
own process group to survive launchd's cleanup of the old main process group.

The old executable starts a separate 120-second watchdog. The new client confirms
the journal after opening its session/subscription and surviving 30 seconds of
normal operation. Only then does it report success. If it never confirms, the
watchdog verifies and restores the previous image. A running candidate observes
that rollback and exits so its supervisor can restart the previous version.
Terminal reports remain in the journal for retransmission after another restart.
Do not delete the journal or previous image while an update is pending.

## ESP32 clients

The C, Rust, and Arduino clients persist attempt identity and target partition in
NVS before selecting the new boot image. After startup and a 30-second health
interval they validate the running partition and report success, or report failure
if the bootloader returned to the previous partition. A 120-second startup
watchdog resets an unconfirmed candidate to invoke bootloader rollback.

ESP-IDF builds must enable `CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE` and provide two
OTA app partitions sized for the firmware. Changing the app alone does not update
an already installed bootloader or partition table; provision those over the
device's normal flashing connection first. Arduino FOTA rejects updates when its
build lacks rollback support. The Rust ESP32-C6 example's existing 4 MB factory
partition layout is not OTA-capable; choose flash capacity and an A/B partition
layout that fit its Zenoh image before enabling deployments.

The rollback APIs and required bootloader setting follow the
[ESP-IDF OTA guide](https://docs.espressif.com/projects/esp-idf/en/v5.3/esp32/api-reference/system/ota.html).

## Upgrade compatibility

Update backend and SDK clients together. Reports from older clients without a
deployment ID cannot complete a new deployment. New firmware may need a further
deployment after its initial bootstrap to establish the new installation journal.
The C status-builder API now requires the deployment ID. No database schema
migration is required; existing deployment IDs identify attempts.
