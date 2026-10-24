import { expect, test } from "bun:test";
import { NowPlayingError, fromNative } from "../src/errors";

test("native rejections keep their code", () => {
  const error = fromNative(new Error("SESSION_NOT_FOUND: Gone."));
  expect(error).toBeInstanceOf(NowPlayingError);
  expect(error.code).toBe("SESSION_NOT_FOUND");
  expect(error.message).toBe("Gone.");
});

test("unknown rejections become PLATFORM_ERROR with the full message", () => {
  for (const message of ["boom", "NOT_A_CODE: x", ": x"]) {
    const error = fromNative(new Error(message));
    expect(error.code).toBe("PLATFORM_ERROR");
    expect(error.message).toBe(message);
  }
});
