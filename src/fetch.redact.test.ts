import { describe, expect, it } from 'vitest';
import { redactFormCredentials } from './fetch';

// SMOODEV-2716: url-encoded request bodies are logged as opaque strings, so an
// OAuth token-exchange body leaked `client_secret` to CloudWatch. These pin the
// redaction that closes that gap.
describe('redactFormCredentials', () => {
    it('redacts client_secret / client_id in a token-exchange body, preserving base64 = padding', () => {
        const body = 'grant_type=client_credentials&client_id=abc-123&client_secret=sk_EL%2BPixKPBBe%3D';
        const out = redactFormCredentials(body);
        expect(out).toContain('grant_type=client_credentials');
        expect(out).toContain('client_secret=[REDACTED]');
        expect(out).toContain('client_id=[REDACTED]');
        expect(out).not.toContain('sk_EL');
    });

    it('leaves a form body with no sensitive params unchanged', () => {
        const body = 'grant_type=client_credentials&scope=read';
        expect(redactFormCredentials(body)).toBe(body);
    });

    it('does not corrupt a JSON string body (not form-encoded)', () => {
        const body = '{"client_secret":"sk_x","a":1}';
        expect(redactFormCredentials(body)).toBe(body);
    });
});
