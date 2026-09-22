# Browser-only interactive demo plan

## Goal

Publish the complete Extrittio operations console as a shareable, browser-only
demo. A visitor can navigate every current screen, inspect realistic sample
devices, and try the main workflows without credentials, a running Extrittio
server, physical devices, or shared mutable data. The demo must clearly label
simulated actions and offer a one-click reset.

This is a distinct build of `apps/frontend`, not a replacement for the normal
frontend. Production continues to use the real `/api/v1` backend and session
cookie.

## Decisions

1. **Reuse the real frontend.** Add a build-time `EXTRITTIO_DEMO` flag. Keep
   the existing components, routes, queries, and API types. Give the demo a
   visible banner, a short introduction, and `Reset demo` control.
2. **Run a simulated API in the browser.** Put the demo implementation behind
   the shared Axios client in `src/api/client.ts`, so feature code retains its
   current API calls. A custom adapter can answer `/api/v1` requests from a
   typed in-browser state store. Handle the one direct logout `fetch` in
   `src/stores/auth-store.ts` through the same demo-aware session boundary.
   Reject and log any unimplemented demo endpoint during development; never
   silently fall through to a live server.
3. **Use coherent fixture data.** Seed several device types and health states,
   each tied to a blueprint, fleet, telemetry series, location where
   applicable, shadow, command history, logs, and relevant alerts. Derive
   dashboard totals, filters, graphs, and analytics from this state rather
   than maintaining unrelated hard-coded responses.
4. **Persist locally per visitor.** Save supported changes in versioned
   browser storage; migrate or reset on schema changes. `Reset demo` restores
   the seed, clears query caches, and returns to the dashboard. Use a fixed
   relative timeline so sample readings stay recent at each visit. No demo
   state or entered data goes to Extrittio infrastructure.
5. **Ship static assets only.** Build with a configurable Vite base path and
   static-host-compatible routing (for example `HashRouter`, or a host with
   an SPA fallback). Publish the bundle, including any later Wasm, to a
   static-file host such as GitHub Pages. A shareable URL still requires a
   static host; no application server or database is required.

## Visitor experience and behavior

| Surface | Demo behavior |
| --- | --- |
| Entry, login, profile, navigation | Enter as a clearly named demo user with permissions needed to expose every route. Preserve the real login flow in production. Show demo status and reset from every screen. |
| Dashboard, devices, blueprints, fleets | Lists, detail pages, search, filters, pagination, creation, edits, bulk fleet changes, and deletion operate on one state. Device detail tabs show consistent contract, telemetry, config, shadow, command, location, alert, and log data. |
| Analytics, fleet graph, map | Charts and aggregates reflect the seeded devices and supported edits. Seed locations and zones. Map tiles are an external asset dependency unless replaced with bundled tiles or an offline backdrop. |
| Rules, alerts, activity | Create and edit rules; toggle them; acknowledge, resolve, and reactivate alerts. Record simulated actions in activity history. Rule execution may initially be a documented deterministic simulation. |
| Commands, restarts, firmware, OTA | Accept UI actions and show a simulated lifecycle and result. Never imply that a real device received a command or update. File uploads remain local to the browser; bound accepted size. |
| OpenThread scanner, mesh, settings | Supply a consistent sample network, scan results, topology, and editable demo configuration. Label scans and network operations as simulated. |
| Users, roles, certificates, API keys | Keep screens navigable and forms testable with local-only sample records. Never create usable credentials, export a real private key, or display a secret that looks valid for production. |
| Help and remaining settings | Load without server requests or dead navigation; explain where the production product differs from the demo. |

All current routes in `src/app/routes.tsx`, including nested settings and
OpenThread routes, are in scope. `Full UI` means every route renders and its
visible controls either work against demo state or explicitly explain a
simulated or unavailable hardware outcome. It does not mean running the entire
Rust backend in the browser.

## Implementation sequence

### 1. Inventory and contract

- Enumerate requests from `src/api`, `src/features/*/api`, and the direct auth
  logout call. Record method, path, query parameters, response shape, and UI
  action that triggers each request. Use `api/openapi.json` and generated
  `src/types/openapi.ts` as the response contract.
- Build a route-by-route behavior matrix from `src/app/routes.tsx` and nested
  settings/OpenThread routes. Mark every button or form as local mutation,
  simulated device effect, read-only data, or an external link.
- Define fixture invariants: IDs and references resolve; counts equal lists;
  charts reflect telemetry; filters and pagination are stable; status and
  timestamps agree. Choose a small, varied sample fleet rather than a large
  random dataset.

### 2. Demo runtime and first journey

- Introduce the demo build flag, demo session user, browser-state schema,
  reset control, and API adapter. Keep production authentication and API
  transport untouched when the flag is off.
- Implement auth, dashboard, devices, device details, blueprints, fleets,
  telemetry, shadows, commands, alerts, and logs first. Make the journey
  from dashboard to a device, command, and resulting activity believable.
- Add contract checks for every handled endpoint and a development assertion
  that unhandled `/api/v1` calls fail visibly.

### 3. Complete every screen

- Implement analytics queries, map/zones, graph data, rules, firmware and OTA,
  OpenThread, users/roles, certificates, API keys, server metrics, and any
  remaining endpoint from the inventory.
- Reuse state transitions across screens. For example, changing a device's
  fleet updates its list row, fleet graph, filters, and analytics; resolving
  an alert updates the dashboard and activity history.
- Add concise UI copy beside simulated hardware/security actions. Provide
  clear local-only outcomes instead of generic success toasts.

### 4. Static packaging and verification

- Produce a demo-only Vite bundle with correct asset paths and deep-link
  behavior on the chosen static host. Ensure its requests never target the
  production API. Keep demo assets separate from the embedded Edge UI build.
- Run typecheck, lint, unit tests, and bundle build. Add focused tests for
  fixture invariants, API responses, mutations, reset, and production/demo
  separation. Browser-test every route at desktop and mobile widths, plus
  the main create/edit/delete, alert, command, and reset journeys.
- Test a deployed preview through a fresh browser profile, including reload,
  direct link, offline behavior after assets load, and two independent
  visitors. Publish only after the preview is reviewable.

## Usability testing without an application backend

- Give testers a short task list: find an offline device, inspect its history,
  move it to a fleet, acknowledge an alert, create a rule, and try a command.
  Ask what they expected at each step and what they could not find.
- For moderated sessions, use screen sharing and take notes. A static page
  alone cannot tell us where remote visitors clicked or what confused them.
  If unmoderated feedback is needed, add an explicit link to an external form.
  Analytics or session replay would be an additional third-party service and
  should be a separate decision with clear visitor notice.
- Start each session with `Reset demo`; use the same fixture version for
  comparable observations. Record the build version and task outcomes.

## Acceptance criteria

- A fresh visitor opens one URL and reaches the complete console without
  signing in or starting a backend.
- Every current route and nested tab renders with plausible, related sample
  data; no blank screen, unexpected 401, or unhandled API request remains.
- Key mutations change related screens coherently, survive reload for that
  visitor, and reset completely on demand.
- Hardware, network, credential, and firmware actions visibly state their
  simulated result. No demo action can affect a real device or account.
- The production build still uses the normal session and API, with no demo
  user or fixture data included in its behavior.
- A static-host preview passes the browser checks above and can be used for
  moderated user testing.

## Later option: real rule evaluation in Wasm

`crates/rule-engine` has no HTTP, persistence, or transport dependencies, so
it is a reasonable candidate for a later Wasm module. Add it only if testing
shows that visitors need to exercise rule semantics more deeply than the
deterministic demo simulator allows. It does not change the static hosting or
browser-state design.
