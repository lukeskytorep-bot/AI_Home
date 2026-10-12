import { describe, expect, it } from "vitest";
import { ProviderCallError } from "../../src/providers/providerError";
import { isRetryableProviderError, isStandardRetryHttpStatus, providerRetryAllowance, providerRetryCategory, providerRetryDelayMs, STANDARD_RETRY_HTTP_STATUSES } from "../../src/providers/retry";

describe("provider retry policy", () => {
  it("centralizes the complete standard HTTP retry status policy, including 524 and 529", () => {
    expect(STANDARD_RETRY_HTTP_STATUSES).toEqual([408, 425, 429, 500, 502, 503, 504, 524, 529]);
    for (const status of STANDARD_RETRY_HTTP_STATUSES) expect(isStandardRetryHttpStatus(status)).toBe(true);
    for (const status of [400, 401, 403, 404, 409, 422, 501]) expect(isStandardRetryHttpStatus(status)).toBe(false);

    for (const status of [524, 529]) {
      expect(providerRetryCategory(new ProviderCallError({ code: "http_status", message: `status ${status}`, phase: "reading_body", httpStatus: status }))).toBe("standard");
      expect(providerRetryCategory(new Error(`provider request failed (${status}): transient`))).toBe("standard");
      expect(providerRetryCategory(new Error(`provider error payload code=${status} type=upstream_unavailable: retry`))).toBe("standard");
    }
  });
  it("automatically retries only explicit not-yet-processed and throttling responses", () => {
    expect(isRetryableProviderError(new Error("network connection reset"))).toBe(true);
    expect(isRetryableProviderError(new Error("provider request failed (429): rate limit"))).toBe(true);
    expect(isRetryableProviderError(new Error("provider request failed (425): too early"))).toBe(true);
    expect(isRetryableProviderError(new Error("provider request failed (503): unavailable"))).toBe(true);
  });

  it("keeps the existing transient retry window open before the first semantic chunk", () => {
    const error = new ProviderCallError({
      code: "timeout",
      message: "first-event timeout before semantic output",
      phase: "reading_body",
    });
    expect(providerRetryCategory(error)).toBe("standard");
    expect(providerRetryAllowance(error, 2)).toBe(2);
  });

  it("never retries a stream that exceeded a local bounded-buffer guard", () => {
    for (const semanticOutputStarted of [undefined, true]) {
      const error = new ProviderCallError({
        code: "response_body_too_large",
        message: "provider streaming response exceeded the maximum accumulated data size",
        phase: "reading_body",
        ...(semanticOutputStarted ? { semanticOutputStarted } : {}),
      });
      expect(providerRetryCategory(error)).toBe("never");
      expect(providerRetryAllowance(error, 5)).toBe(0);
    }
  });

  it("never retries a streaming failure after the first semantic chunk", () => {
    const error = new ProviderCallError({
      code: "timeout",
      message: "idle timeout after partial output",
      phase: "reading_body",
      semanticOutputStarted: true,
    });
    expect(providerRetryCategory(error)).toBe("never");
    expect(providerRetryAllowance(error, 5)).toBe(0);
  });

  it("limits ambiguous transport recovery to one retry", () => {
    const error = new Error("error decoding response body");
    expect(providerRetryCategory(error)).toBe("single_recovery");
    expect(providerRetryAllowance(error, 5)).toBe(1);
    expect(0).toBeLessThan(providerRetryAllowance(error, 5));
    expect(1).not.toBeLessThan(providerRetryAllowance(error, 5));
    expect(providerRetryCategory(new Error("provider returned reasoning without a final assistant response [finish-reason=length]"))).toBe("never");
    expect(providerRetryCategory(new Error("provider returned an incomplete assistant response [finish-reason=max_tokens]"))).toBe("never");
  });

  it("retries transient gateway statuses according to the configured count", () => {
    const error = new Error("provider request failed (504): gateway timeout");
    expect(providerRetryAllowance(error, 3)).toBe(3);
    expect(2).toBeLessThan(providerRetryAllowance(error, 3));
    expect(3).not.toBeLessThan(providerRetryAllowance(error, 3));
    expect(providerRetryAllowance(new Error("provider error payload code=503 type=upstream_unavailable: try again"), 2)).toBe(2);
  });

  it("does not retry safety, configuration, or credential failures", () => {
    for (const message of ["content_filter", "context length exceeded", "invalid model id", "API key invalid", "provider request failed (401): unauthorized"]) {
      expect(providerRetryCategory(new Error(message))).toBe("never");
      expect(providerRetryAllowance(new Error(message), 5)).toBe(0);
    }
  });

  it("does not retry permanent authentication and request errors", () => {
    expect(isRetryableProviderError(new Error("provider request failed (401): unauthorized"))).toBe(false);
    expect(isRetryableProviderError(new Error("invalid model id"))).toBe(false);
  });

  it("uses bounded full-jitter exponential backoff", () => {
    expect([0, 1, 2, 8].map((attempt) => providerRetryDelayMs(attempt, undefined, () => 1))).toEqual([500, 1000, 2000, 8000]);
    expect(providerRetryDelayMs(2, undefined, () => 0.25)).toBe(500);
    expect(providerRetryDelayMs(0, new Error("provider request failed (429) [retry-after-ms=4500]: wait"))).toBe(4500);
    expect(providerRetryDelayMs(0, new Error("provider request failed (429) [retry-after-ms=999999]: wait"))).toBe(30_000);
    expect(providerRetryDelayMs(0, new ProviderCallError({ code: "http_status", message: "retry now", phase: "reading_body", httpStatus: 429, retryAfterMs: 0 }))).toBe(0);
  });
});
