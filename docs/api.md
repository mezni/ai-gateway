# AI Gateway API

## 1. Overview

The AI Gateway exposes a unified HTTP API for applications that need to interact with multiple Large Language Model (LLM) providers.

The gateway hides provider-specific APIs behind a normalized interface.

```mermaid
flowchart TD
    Client[Client Application] -->|HTTP/JSON| GW[AI Gateway<br/>Authentication<br/>Authorization<br/>Rate Limiting<br/>Model Routing<br/>Policy Enforcement<br/>Provider Abstraction<br/>Usage Tracking]
    GW --> PA[Provider A]
    GW --> PB[Provider B]
    GW --> PC[Provider C]
```

The API is designed to resemble common chat-completion APIs while keeping the gateway internally provider-independent.
