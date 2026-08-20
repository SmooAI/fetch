import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { isSensitiveKey, redactBody, redactHeaders, redactQueryString, redactUrl } from './redact';

// SMOODEV-2716. The cases live in spec/redaction-corpus.json, shared with the
// Rust suite — do not inline them here, or the two implementations drift.
const corpusPath = join(dirname(fileURLToPath(import.meta.url)), '..', 'spec', 'redaction-corpus.json');

interface Case<TInput, TExpected> {
    name: string;
    input: TInput;
    expected: TExpected;
}

const corpus = JSON.parse(readFileSync(corpusPath, 'utf8')) as {
    redactedWith: string;
    sensitiveKeys: { contains: string[]; equals: string[] };
    cases: {
        headers: Case<Record<string, string>, Record<string, string>>[];
        query: Case<string, string>[];
        url: Case<string, string>[];
        form: Case<string, string>[];
        json: Case<string, string>[];
    };
};

describe('redaction corpus', () => {
    // Positive control: a corpus that failed to load reads as "all tests passed".
    it('loaded every group', () => {
        for (const [group, cases] of Object.entries(corpus.cases)) {
            expect(cases.length, `group ${group} is empty`).toBeGreaterThan(0);
        }
        expect(corpus.redactedWith).toBe('[REDACTED]');
    });

    it.each(corpus.cases.headers)('headers: $name', ({ input, expected }) => {
        expect(redactHeaders(input)).toEqual(expected);
    });

    it.each(corpus.cases.query)('query: $name', ({ input, expected }) => {
        expect(redactQueryString(input)).toBe(expected);
    });

    it.each(corpus.cases.url)('url: $name', ({ input, expected }) => {
        expect(redactUrl(input)).toBe(expected);
    });

    it.each(corpus.cases.form)('form: $name', ({ input, expected }) => {
        expect(redactBody(input)).toBe(expected);
    });

    it.each(corpus.cases.json)('json: $name', ({ input, expected }) => {
        expect(redactBody(input)).toBe(expected);
    });

    // The key list is the security contract; keeping it in the corpus means the
    // Rust side matches on the same roots.
    it.each(corpus.sensitiveKeys.contains)('treats a key containing %s as sensitive', (part) => {
        expect(isSensitiveKey(`x_${part}_value`)).toBe(true);
    });

    it.each(corpus.sensitiveKeys.equals)('treats the exact key %s as sensitive', (key) => {
        expect(isSensitiveKey(key)).toBe(true);
    });
});

describe('redactBody', () => {
    it('redacts an object body without stringifying secrets first', () => {
        expect(redactBody({ client_secret: 'sk_x', a: 1 })).toBe('{"client_secret":"[REDACTED]","a":1}');
    });

    it('returns undefined for an absent body', () => {
        expect(redactBody(undefined)).toBeUndefined();
        expect(redactBody(null)).toBeUndefined();
        expect(redactBody('')).toBeUndefined();
    });

    it('leaves an unparseable JSON-ish string alone rather than mangling it', () => {
        expect(redactBody('{not json')).toBe('{not json');
    });
});

describe('isSensitiveKey', () => {
    it('does not redact identifiers that merely resemble credential names', () => {
        for (const safe of ['country_code', 'zipcode', 'user_id', 'client_id', 'grant_type', 'scope', 'state']) {
            expect(isSensitiveKey(safe), safe).toBe(false);
        }
    });
});
