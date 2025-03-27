---
'@smooai/fetch': patch
---

Enhance README and fetch implementation with new options

- Added detailed section on opinionated defaults for the fetch function, including retry configuration, timeout settings, and rate limit retry options.
- Updated examples to demonstrate usage of new options in fetch requests.
- Introduced `RequestInitWithOptions` type to support additional options in fetch requests, within the same fetch argument footprint.
- Improved error handling and response type inference in the fetch implementation.

This update aims to provide better guidance for users and enhance the flexibility of the fetch functionality.
