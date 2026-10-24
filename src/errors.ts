export type NowPlayingErrorCode =
  | "INVALID_ARGUMENT"
  | "NO_SESSION"
  | "SESSION_NOT_FOUND"
  | "UNSUPPORTED_OPERATION"
  | "UNSUPPORTED_PLATFORM"
  | "BACKEND_UNAVAILABLE"
  | "PLATFORM_ERROR"
  | "INVALID_DATA";

const codes = new Set<string>([
  "INVALID_ARGUMENT",
  "NO_SESSION",
  "SESSION_NOT_FOUND",
  "UNSUPPORTED_OPERATION",
  "UNSUPPORTED_PLATFORM",
  "BACKEND_UNAVAILABLE",
  "PLATFORM_ERROR",
  "INVALID_DATA",
]);

export class NowPlayingError extends Error {
  readonly code: NowPlayingErrorCode;
  constructor(code: NowPlayingErrorCode, message: string) {
    super(message);
    this.name = "NowPlayingError";
    this.code = code;
  }
}

/** The addon rejects with `CODE: message`. */
export function fromNative(error: unknown): NowPlayingError {
  const message = error instanceof Error ? error.message : String(error);
  const separator = message.indexOf(": ");
  const code = message.slice(0, separator);
  return separator > 0 && codes.has(code)
    ? new NowPlayingError(
        code as NowPlayingErrorCode,
        message.slice(separator + 2),
      )
    : new NowPlayingError("PLATFORM_ERROR", message);
}
