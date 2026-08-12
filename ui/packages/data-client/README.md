# @extrittio/data-client

Backend-agnostic Axios client factory with injectable token lookup and unauthorized handling.

## Exports

- `createApiClient`
- `CreateApiClientOptions`

## Usage

```ts
import { createApiClient } from "@extrittio/data-client";

export const api = createApiClient({
  baseUrl: "/api/v1",
  getToken: () => localStorage.getItem("token"),
  onUnauthorized: () => {
    window.location.href = "/login";
  },
});
```
