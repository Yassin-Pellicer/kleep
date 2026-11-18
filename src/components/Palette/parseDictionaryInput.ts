/**
 * Returns the term to look up when the input uses a quoted form:
 *
 *   "hello world"          -> hello world
 *   en:"hola mundo"        -> en:hola mundo
 *   'some phrase'          -> some phrase
 *
 * The prefix before the colon is preserved as part of the term. Returns null
 * for ordinary input.
 */
export function parseDictionaryInput(input: string): string | null {
  const trimmed = input.trim();
  if (!trimmed) return null;

  // prefix:"quoted text" — keep the prefix, drop the quote delimiters.
  const prefixed = trimmed.match(/^(.*?):\s*(["'])(.*)\2$/s);
  if (prefixed) {
    const prefix = prefixed[1].trim();
    const term = prefixed[3].trim();
    if (!term) return null;
    return prefix ? `${prefix}:${term}` : term;
  }

  // "quoted text"
  const whole = trimmed.match(/^(["'])(.*)\1$/s);
  if (whole) {
    const term = whole[2].trim();
    return term.length > 0 ? term : null;
  }

  return null;
}
