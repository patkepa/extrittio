# Apple Watch App Product Design

- **Status:** Proposed
- **Decision date:** 2026-09-02
- **Scope:** A native watchOS companion for Extrittio operators
- **Companion architecture:** [Apple Watch App Architecture and Implementation Plan](watch-app-architecture-plan.md)

## 1. Product decision

Build the watch app as an operator companion, not a miniature copy of the iOS
app. It should answer three questions in a few seconds:

1. Is the system healthy?
2. What needs attention now?
3. Can I safely acknowledge the issue or inspect the affected device?

The watch owns fast monitoring and triage. The iPhone continues to own login,
server setup, provisioning, configuration, rules, firmware, administration,
and other work that requires dense information or careful editing.

This follows Apple's guidance that watch experiences should be glanceable,
focused, shallow, and useful independently. See
[Designing for watchOS](https://developer.apple.com/design/human-interface-guidelines/designing-for-watchos).

## 2. Users and jobs

The primary user is an authenticated Extrittio operator who already configured
the iOS app and may be moving through a home, workshop, facility, or field site.

| Job | Desired outcome | Time target |
| --- | --- | --- |
| Check fleet health | Understand online, offline, and alert state | One wrist raise |
| Triage an alert | See severity, message, device, value, and age | Under 15 seconds |
| Acknowledge an alert | Signal ownership without opening the phone | Two deliberate taps |
| Inspect a device | See status, last contact, fleet, firmware, and key metrics | Under 20 seconds |
| Continue on iPhone | Offer the matching detail through Handoff for complex work | One watch tap |

## 3. Release scope

### First release

- iPhone-led setup and revocation.
- Permission-aware fleet health overview.
- Active and acknowledged alert list.
- Alert detail and acknowledgement.
- Attention-first device list and device summary.
- Cached, timestamped offline state.
- Health and alert-count complications.
- Deep links from complications into the relevant watch screen.

### Later releases

- Direct APNs alert notifications and notification acknowledgement.
- Configurable complications for a fleet or favorite device.
- A confirmed device restart for users with `commands.send`.
- Contract-defined latest metrics and compact trends.
- App Shortcuts for health and alert queries.

### Explicitly iPhone-only

- Username and password entry, tenant selection, and server editing.
- Device provisioning or deletion.
- Shadow and configuration editing.
- Arbitrary command parameters.
- Alert resolution or reactivation.
- Firmware and OTA operations.
- Rule, user, role, certificate, API-key, and Thread administration.

Acknowledgement is intentionally the only first-release mutation. It is a
reversible operational handoff. Resolution asserts that the condition is
finished and should stay in the richer iPhone flow.

## 4. Experience principles

1. **Attention before inventory.** Critical alerts and offline devices sort
   ahead of healthy devices.
2. **Freshness is part of every fact.** Cached content always carries a visible
   age; the app never presents stale data as live.
3. **One primary action per screen.** Alert detail offers Acknowledge. Device
   detail offers Continue on iPhone.
4. **Color reinforces meaning.** Critical, warning, healthy, and informational
   states also use labels and SF Symbols.
5. **No queued mutations.** When the server and iPhone are unavailable, the app
   remains useful for reading but disables actions.
6. **Permission changes fail closed.** Sections and actions disappear when the
   current credential no longer grants their required permission.

## 5. Information architecture

```text
Overview
├── Alerts
│   └── Alert detail
├── Devices
│   └── Device detail
└── Connection
    └── Continue setup on iPhone
```

Use a single `NavigationStack`. The Overview is one vertically scrolling root
screen driven by the Digital Crown. There is no miniature tab bar and no route
deeper than two pushes from the root.

External routes land directly on an alert or device and preserve a short Back
path to Overview:

- `extrittio://watch/overview`
- `extrittio://watch/alerts/{id}`
- `extrittio://watch/devices/{id}`

## 6. Screen specifications

### 6.1 Overview

The first visible region contains:

- a status label: **Healthy**, **Needs attention**, **Critical**, or
  **Unavailable**;
- online devices as `online / total`;
- critical and total active alert counts;
- the snapshot age when older than two minutes.

Below it, a **Needs Attention** section shows at most three items. The order is
critical alerts, warning alerts, offline devices, then other warning devices.
Each row opens the corresponding detail.

The bottom of the screen contains large Alerts and Devices navigation rows.
Sections are independent: a user with `devices.read` but not `alerts.read` still
gets device health, while a user with only `alerts.read` still gets alert state.

### 6.2 Alerts

Default to active alerts, ordered by severity and then newest first. A compact
menu can switch between Active and Acknowledged. Do not reproduce the iOS app's
full filter bar.

Each row contains:

- severity icon and label;
- a two-line message;
- device name when available, otherwise device ID;
- relative age;
- status when the alert is acknowledged.

Empty state copy distinguishes **No active alerts** from **No permission to
view alerts** and **Alerts unavailable**.

### 6.3 Alert detail

Show severity, full message, device, triggered value when present, created time,
and current status. The device row navigates to Device detail when the user has
`devices.read`.

For an active alert and a user with `alerts.manage`, show one full-width
**Acknowledge** button. The tap presents a short confirmation containing the
device and alert message. While the request is in flight, keep the action
visible with progress and prevent a second submission. A successful response
updates local state immediately and uses a success haptic.

If the response is lost, fetch the canonical alert. Treat an already
acknowledged state as success. Never queue acknowledgement while offline.

### 6.4 Devices

The initial group is **Needs Attention**, followed by **All Devices**. Sort
offline and warning devices before online devices, then by name. A simple
Online/Offline menu is enough for the first release; omit text search until
there is evidence that watch users need it.

Each row contains the device name, status label, fleet name when present, and
relative last contact. The status must remain understandable without color.

### 6.5 Device detail

Show:

- name and status;
- device type and fleet;
- last contact and firmware;
- up to three key metrics selected by contract presentation metadata;
- active alert count;
- **Continue on iPhone**.

If generic contract metrics are not yet available, omit the metric section
instead of presenting legacy temperature, humidity, and battery fields as if
they applied to every device.

### 6.6 Connection and setup states

| State | Presentation | Available behavior |
| --- | --- | --- |
| Watch not paired | “Finish setup on iPhone” | Retry Watch Connectivity |
| iPhone signed out | “Sign in on iPhone” | Cached read-only content |
| Credential expired | “Reconnect to refresh access” | Attempt direct renewal, then iPhone |
| Server unreachable | Cached content with timestamp | Retry and Continue on iPhone |
| No relevant permissions | “Your Extrittio role has no watch access” | Open account on iPhone |

Do not ask for an Extrittio password on Apple Watch.

## 7. Complications

Use WidgetKit and deep-link every complication to Overview or Alerts. The first
release supports:

| Family | Content | Stale behavior |
| --- | --- | --- |
| Accessory circular | Health glyph plus critical count | Dim tint and retain last count |
| Accessory rectangular | `12/14 online` and `2 active alerts` | Add an age label when space permits |
| Accessory inline | `Extrittio: Healthy` or `2 critical alerts` | Prefix with `Last:` when stale |

Complications are snapshots, not real-time monitors. WidgetKit controls the
actual refresh schedule and applies a budget; the app must refresh on launch
and show age rather than promise exact intervals. See
[Developing a WidgetKit strategy](https://developer.apple.com/documentation/widgetkit/developing-a-widgetkit-strategy)
and
[Creating accessory widgets and watch complications](https://developer.apple.com/documentation/widgetkit/creating-accessory-widgets-and-watch-complications).

## 8. Notifications

Direct watch notifications are a later release because the current Extrittio
backend has no APNs registration or delivery path.

When implemented, only critical and user-selected warning alerts should notify
by default. The long look contains the severity, device, message, and triggered
value, with **Acknowledge** as the first nondestructive action. Backend
authorization remains authoritative even if an old notification still exposes
the action.

Apple Watch can invoke the first nondestructive notification action with Double
Tap, so Acknowledge must remain first and destructive operations must never use
that position. See
[Adding actions to notifications on watchOS](https://developer.apple.com/documentation/watchos-apps/adding-actions-to-notifications-on-watchos).

## 9. Visual language

The watch app is recognizably Extrittio without reproducing phone-sized cards:

- use the system black background and native watchOS typography;
- retain the iOS app's semantic tones: green healthy, orange warning, red
  critical/offline, blue informational;
- pair every tone with an SF Symbol and text;
- use material or glass treatment only for the primary health region and
  prominent controls;
- use native lists, buttons, navigation titles, and confirmation surfaces;
- avoid charts in the first release; a value and trend label are more legible
  at a glance.

The Continue action publishes a small `NSUserActivity` for Handoff. It cannot
force-launch the iPhone app; the user accepts the activity on the iPhone. The
iOS app restores the matching route when it receives the activity. See
[Implementing Handoff in Your App](https://developer.apple.com/documentation/foundation/implementing-handoff-in-your-app).

The existing iOS `Spacing`, status semantics, severity semantics, and fleet
health classification should move into shared platform-neutral values where
appropriate. UIKit haptic helpers and iPhone-specific view modifiers must not
be shared with watchOS.

## 10. Accessibility and ergonomics

- Support Dynamic Type without truncating the primary status or action.
- Give interactive rows and buttons a minimum 44-point hit region.
- Provide VoiceOver labels that include severity, status, device, and age in a
  useful reading order.
- Never encode health, alert severity, or freshness by color alone.
- Respect Reduce Motion and avoid continuous status pulsing on the watch.
- Keep acknowledgement reachable within two deliberate taps from an alert row.
- Localize visible strings and use locale-aware dates and numbers.

## 11. Product acceptance criteria

The first release is ready when:

1. A configured user can install the watch app and finish setup without typing
   server details or a password on the watch.
2. Overview reaches useful content in one foreground refresh and still renders
   a timestamped snapshot without network access.
3. Every visible section and action matches the user's server permissions.
4. An authorized user can acknowledge an active alert, and an ambiguous network
   result reconciles against server state.
5. No offline, repeated, or accidental tap can queue a mutation.
6. Complications render all supported families and deep-link correctly.
7. VoiceOver, Dynamic Type, stale, empty, loading, unauthorized, and offline
   states have dedicated tests or previews.
8. Complex and destructive operations consistently hand off to the iPhone.
