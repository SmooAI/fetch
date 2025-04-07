# @smooai/fetch

## 1.4.0

### Minor Changes

- 88f6e41: Fix issue with pre-using response body and update prettier plugins.

## 1.3.0

### Minor Changes

- 937a5cd: Changed FetchBuilder to take the schema in the constructor to fix type inference.

### Patch Changes

- 937a5cd: Updated all vite dependencies.

## 1.2.1

### Patch Changes

- 081e6ff: Fix package description.

## 1.2.0

### Minor Changes

- 7cbaa0b: Add lifecycle hooks to fetch implementation and update README

    - Introduced lifecycle hooks: pre-request, post-response success, and post-response error, allowing for enhanced request and response handling.
    - Updated README with detailed descriptions of lifecycle hooks and examples demonstrating their usage.
    - Refactored fetch implementation to integrate hooks, improving flexibility and error handling capabilities.

### Patch Changes

- 7cbaa0b: Enhance README and fetch implementation with new options

    - Added detailed section on opinionated defaults for the fetch function, including retry configuration, timeout settings, and rate limit retry options.
    - Updated examples to demonstrate usage of new options in fetch requests.
    - Introduced `RequestInitWithOptions` type to support additional options in fetch requests, within the same fetch argument footprint.
    - Improved error handling and response type inference in the fetch implementation.

    This update aims to provide better guidance for users and enhance the flexibility of the fetch functionality.

## 1.1.0

### Minor Changes

- 07df8fe: Enhance fetch functionality with schema validation

    - Enhanced fetch implementation with a FetchBuilder class for better configuration options, including schema validation, retry, and rate limiting.
    - Improved error handling and logging capabilities in the fetch module.
    - Updated README to reflect new features and usage examples.

## 1.0.7

### Patch Changes

- 3503fdb: Fix index export via @smooai/utils update.

## 1.0.6

### Patch Changes

- 4277a0f: Fix package file selection."

## 1.0.5

### Patch Changes

- 4d45f19: Fix npm publishing.

## 1.0.4

### Patch Changes

- 300d106: Fixed package.json for publishing.

## 1.0.3

### Patch Changes

- 8ceaebc: Updating @smooai/fetch to be its own package.

## 1.0.2

### Patch Changes

- 44fd23b: Fix publish for Github releases.

## 1.0.1

### Patch Changes

- 52c9eb1: Initial check-in.
