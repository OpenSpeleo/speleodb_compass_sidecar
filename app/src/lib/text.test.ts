import { describe, expect, it } from "vitest";
import { rustTrim } from "./text";

describe("Rust whitespace compatibility", () => {
  it("trims the next-line character that JavaScript trim retains", () =>
    expect(rustTrim("\u0085 hello \u0085")).toBe("hello"));
  it("preserves byte-order marks that JavaScript trim removes", () =>
    expect(rustTrim(" \ufeffhello\ufeff ")).toBe("\ufeffhello\ufeff"));
  it("handles the complete Unicode White_Space set and preserves interior whitespace", () => {
    const whitespace =
      "\u0009\u000a\u000b\u000c\u000d\u0020\u0085\u00a0\u1680\u2000\u2001\u2002\u2003\u2004\u2005\u2006\u2007\u2008\u2009\u200a\u2028\u2029\u202f\u205f\u3000";
    expect(rustTrim(`${whitespace}a\u0085b${whitespace}`)).toBe("a\u0085b");
    expect(rustTrim(whitespace)).toBe("");
  });
});
