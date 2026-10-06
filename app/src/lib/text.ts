/** Match Rust str::trim (Unicode White_Space), rather than ECMAScript's whitespace set. */
export function rustTrim(value: string): string {
  return value.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, "");
}
