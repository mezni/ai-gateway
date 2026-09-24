# AI Gateway API

## 1. Overview

The AI Gateway exposes a unified HTTP API for applications that need to interact with multiple Large Language Model (LLM) providers.

The gateway hides provider-specific APIs behind a normalized interface.

```text
Client Application
       |
       | HTTP/JSON
       v
+----------------------+
|      AI Gateway      |
|                      |
| Authentication       |
| Authorization        |
| Rate Limiting        |
| Model Routing        |
| Policy Enforcement   |
| Provider Abstraction |
| Usage Tracking       |
+----------+-----------+
           |
     +-----+-----+-----+
     |           |     |
     v           v     v
 Provider A  Provider B Provider C
```

The API is designed to resemble common chat-completion APIs while keeping the gateway internally provider-independent.
