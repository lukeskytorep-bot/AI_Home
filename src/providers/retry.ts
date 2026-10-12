import { providerErrorDetails } from "./providerError";

export type ProviderRetryCategory = "standard" | "single_recovery" | "never";

export const STANDARD_RETRY_HTTP_STATUSES = [408, 425, 429, 500, 502, 503, 504, 524, 529] as const;
const STANDARD_RETRY_HTTP_STATUS_SET = new Set<number>(STANDARD_RETRY_HTTP_STATUSES);
const MAX_RETRY_AFTER_MS = 30_000;

export function isStandardRetryHttpStatus(status: number | undefined): boolean {
  return typeof status === "number" && STANDARD_RETRY_HTTP_STATUS_SET.has(status);
}

export function providerRetryCategory(cause: unknown): ProviderRetryCategory {
  // A completed executor operation must never be retried by a caller. This is
  // also a compatibility guard while old controller loops are removed.
  if (cause instanceof Error && cause.name === "ProviderExecutionError") return "never";
  if (cause instanceof DOMException && cause.name === "AbortError") return "never";
  const details = providerErrorDetails(cause);
  if (details) {
    if (details.semanticOutputStarted) return "never";
    if (details.code === "cancelled" || details.code === "configuration" || details.code === "response_body_too_large") return "never";
    if (/^(rate_limit_exceeded|provider_overloaded|provider_unavailable|timeout|server)$/i.test(details.providerErrorType ?? "")) return "standard";
    if (["response_body_read", "response_body_decode", "invalid_provider_json", "empty_assistant_response"].includes(details.code)) return "single_recovery";
    if (["connect", "timeout", "request_send"].includes(details.code)) return "standard";
    if (details.code === "http_status") return isStandardRetryHttpStatus(details.httpStatus) ? "standard" : "never";
    if (details.code === "provider_error") return "never";
  }
  const message = (cause instanceof Error ? cause.message : String(cause)).toLowerCase();
  const status = message.match(/\((\d{3})\)/)?.[1];
  if (status) {
    const code = Number(status);
    if (isStandardRetryHttpStatus(code)) return "standard";
    return "never";
  }
  const payloadCode = Number(message.match(/\bcode=(\d{3})\b/)?.[1]);
  if (Number.isFinite(payloadCode)) return isStandardRetryHttpStatus(payloadCode) ? "standard" : "never";
  if (/content[_ -]?filter|safety|blocked|block reason|prompt feedback|context length|context window|maximum context|invalid model|model not found|unauthorized|forbidden|api key|credential|permission|route mismatch|cancelled|canceled|user stop|cost limit/.test(message)) return "never";
  if (/decod(?:e|ing).*response body|response body.*(?:incomplete|closed|read)|invalid provider json|empty (?:assistant|provider) response|connection reset|connection closed|unexpected eof/.test(message)) return "single_recovery";
  if (/timed? out|timeout|network error|error sending request|connect error|dns error|temporarily unavailable|overloaded/.test(message)) return "standard";
  return "never";
}

export function providerRetryAllowance(cause: unknown, configuredRetries: number): number {
  const configured = Math.max(0, Math.min(Math.floor(configuredRetries), 5));
  if (configured === 0) return 0;
  const category = providerRetryCategory(cause);
  return category === "standard" ? configured : category === "single_recovery" ? 1 : 0;
}

export function isRetryableProviderError(cause: unknown): boolean {
  return providerRetryCategory(cause) !== "never";
}

export function providerRetryDelayMs(failedAttempt: number, cause?: unknown, random: () => number = Math.random): number {
  const details = providerErrorDetails(cause);
  if (typeof details?.retryAfterMs === "number" && details.retryAfterMs >= 0) return Math.min(MAX_RETRY_AFTER_MS, details.retryAfterMs);
  const message = cause instanceof Error ? cause.message : String(cause ?? "");
  const retryAfter = Number(message.match(/\[retry-after-ms=(\d+)\]/)?.[1]);
  if (Number.isFinite(retryAfter) && retryAfter >= 0) return Math.min(MAX_RETRY_AFTER_MS, retryAfter);
  const ceiling = Math.min(8_000, 500 * 2 ** Math.max(0, failedAttempt));
  return Math.floor(Math.max(0, Math.min(1, random())) * ceiling);
}
