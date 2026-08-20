/**
 * Credential redaction for anything this client hands to a logger.
 *
 * SMOODEV-2716: an OAuth `client_credentials` token exchange leaked its
 * `client_secret` to CloudWatch in plaintext. The original diagnosis blamed the
 * url-encoded request body alone, but `@smooai/logger` performs no redaction of
 * any kind — so the `Authorization` header, the raw query string and the full
 * URL in the log message were leaking too, on every request, at debug level.
 *
 * Everything here is log-only. The request actually sent on the wire is never
 * touched.
 */

export const REDACTED = '[REDACTED]';

// A key is sensitive when its normalized form CONTAINS one of these.
const SENSITIVE_PARTS = [
    'secret',
    'password',
    'passwd',
    'token',
    'apikey',
    'authorization',
    'credential',
    'privatekey',
    'assertion',
    'cookie',
    'session',
    'signature',
];

// ...or EQUALS one of these. Too short or too common to substring-match without
// shredding unrelated fields (`code` would hit `country_code`, `zipcode`, ...).
const SENSITIVE_EXACT = new Set(['auth', 'code', 'pwd', 'sig']);

// `X-Api-Key`, `x_api_key` and `apiKey` are the same key as far as a leak is
// concerned, so separators and case are removed before matching.
function normalizeKey(key: string): string {
    return key.toLowerCase().replace(/[-_.]/g, '');
}

export function isSensitiveKey(key: string): boolean {
    const k = normalizeKey(key);
    return SENSITIVE_EXACT.has(k) || SENSITIVE_PARTS.some((part) => k.includes(part));
}

/** Redact sensitive entries of a header map. Returns a copy. */
export function redactHeaders(headers: Record<string, string>): Record<string, string> {
    const out: Record<string, string> = {};
    for (const [key, value] of Object.entries(headers)) {
        out[key] = isSensitiveKey(key) ? REDACTED : value;
    }
    return out;
}

// Shared by the query string and the url-encoded body: both are `a=1&b=2`.
// Splits each pair on the FIRST `=` so a base64 value keeps its `=` padding.
function redactPairs(pairs: string): string {
    return pairs
        .split('&')
        .map((pair) => {
            const eq = pair.indexOf('=');
            if (eq === -1) return pair;
            const key = pair.slice(0, eq);
            return isSensitiveKey(key) ? `${key}=${REDACTED}` : pair;
        })
        .join('&');
}

/** Redact sensitive params from a `?a=1&b=2` query string (leading `?` optional). */
export function redactQueryString(search: string): string {
    if (!search) return search;
    return search.startsWith('?') ? `?${redactPairs(search.slice(1))}` : redactPairs(search);
}

// scheme://user:password@host — the password half of URL userinfo.
const URL_USERINFO_PASSWORD = /^([a-z][a-z0-9+.-]*:\/\/[^/?#@]*:)[^/?#@]*@/i;

/**
 * Redact credentials from a URL string: userinfo password and query params.
 * Purely textual, so relative URLs and non-parseable strings work unchanged and
 * nothing gets normalized behind the caller's back.
 */
export function redactUrl(url: string): string {
    const hash = url.indexOf('#');
    const base = hash === -1 ? url : url.slice(0, hash);
    const fragment = hash === -1 ? '' : url.slice(hash);

    const q = base.indexOf('?');
    const withQuery = q === -1 ? base : `${base.slice(0, q)}${redactQueryString(base.slice(q))}`;

    return `${withQuery.replace(URL_USERINFO_PASSWORD, `$1${REDACTED}@`)}${fragment}`;
}

// JSON.stringify does the traversal (and keeps today's throw-on-cycle behavior),
// so the replacer only has to decide one key at a time.
function redactingReplacer(key: string, value: unknown): unknown {
    return key && isSensitiveKey(key) ? REDACTED : value;
}

/**
 * Redact a request/response body for logging. Handles objects, JSON strings and
 * url-encoded strings. Anything else — HTML, prose, an upstream error page — is
 * returned untouched rather than mangled by a form parser it isn't.
 */
export function redactBody(body: unknown): string | undefined {
    if (body === undefined || body === null || body === '') return undefined;
    if (typeof body === 'object') return JSON.stringify(body, redactingReplacer);
    if (typeof body !== 'string') return String(body);

    const trimmed = body.trimStart();
    if (trimmed.startsWith('{') || trimmed.startsWith('[')) {
        try {
            return JSON.stringify(JSON.parse(body), redactingReplacer);
        } catch {
            return body;
        }
    }

    // A url-encoded form body is percent-encoded, so it never contains raw
    // whitespace. Whitespace means this is free text — leave it alone.
    if (!body.includes('=') || /\s/.test(body)) return body;
    return redactPairs(body);
}
