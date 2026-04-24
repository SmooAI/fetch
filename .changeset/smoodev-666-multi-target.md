---
'@smooai/fetch': patch
---

SMOODEV-666: Multi-target the SmooAI.Fetch NuGet package to `net8.0;net9.0;net10.0` so consumers on every current .NET LTS + STS release get a native `lib/` folder match. Polly v8, Microsoft.Extensions.Http, and Microsoft.Extensions.Http.Polly all resolve cleanly on all three TFMs — no per-TFM conditionals needed.
