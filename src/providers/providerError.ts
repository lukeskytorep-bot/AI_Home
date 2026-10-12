export type ProviderFailurePhase = "before_dispatch" | "awaiting_headers" | "reading_body" | "parsing_body" | "validating_response";

export type ProviderFailureCode =
  | "connect"
  | "timeout"
  | "request_send"
  | "response_body_read"
  | "response_body_decode"
  | "response_body_too_large"
  | "invalid_provider_json"
  | "empty_assistant_response"
  | "http_status"
  | "provider_error"
  | "cancelled"
  | "configuration"
  | "unknown";

export interface ProviderCallErrorDetails {
  code: ProviderFailureCode;
  message: string;
  phase: ProviderFailurePhase;
  httpStatus?: number;
  providerErrorType?: string;
  providerCode?: string;
  retryAfterMs?: number;
  providerRequestId?: string;
  semanticOutputStarted?: boolean;
}

export class ProviderCallError extends Error {
  readonly details: ProviderCallErrorDetails;

  constructor(details: ProviderCallErrorDetails, options?: ErrorOptions) {
    super(details.message, options);
    this.name = "ProviderCallError";
    this.details = details;
  }
}

export function providerErrorDetails(cause: unknown): ProviderCallErrorDetails | undefined {
  if (cause instanceof ProviderCallError) return cause.details;
  if (!cause || typeof cause !== "object") return undefined;
  const candidate = cause as Record<string, unknown>;
  const nested = candidate.details && typeof candidate.details === "object"
    ? candidate.details as Record<string, unknown>
    : candidate;
  if (typeof nested.code !== "string" || typeof nested.message !== "string" || typeof nested.phase !== "string") return undefined;
  return {
    code: nested.code as ProviderFailureCode,
    message: nested.message,
    phase: nested.phase as ProviderFailurePhase,
    ...(typeof nested.httpStatus === "number" ? { httpStatus: nested.httpStatus } : {}),
    ...(typeof nested.providerErrorType === "string" ? { providerErrorType: nested.providerErrorType } : {}),
    ...(typeof nested.providerCode === "string" ? { providerCode: nested.providerCode } : {}),
    ...(typeof nested.retryAfterMs === "number" ? { retryAfterMs: nested.retryAfterMs } : {}),
    ...(typeof nested.providerRequestId === "string" ? { providerRequestId: nested.providerRequestId } : {}),
    ...(typeof nested.semanticOutputStarted === "boolean" ? { semanticOutputStarted: nested.semanticOutputStarted } : {}),
  };
}

export function normalizeProviderCallError(cause: unknown): ProviderCallError {
  if (cause instanceof ProviderCallError) return cause;
  const direct = providerErrorDetails(cause);
  if (direct) return new ProviderCallError(direct, { cause });

  if (typeof cause === "string") {
    try {
      const parsed = JSON.parse(cause) as unknown;
      const parsedDetails = providerErrorDetails(parsed);
      if (parsedDetails) return new ProviderCallError(parsedDetails, { cause });
    } catch {
      // A legacy string error is normalized below and remains available to the
      // compatibility classifier for one release cycle.
    }
  }
  const message = cause instanceof Error ? cause.message : String(cause);
  return new ProviderCallError({ code: "unknown", message, phase: "awaiting_headers" }, { cause });
}

