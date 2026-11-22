/**
 * The app's words, kept with its own translations in
 * `crates/arto/locales/frontend.<code>.json`.
 *
 * Importing `t` from here rather than from `./i18n` is what registers the
 * app's English before the first word is looked up.
 */
import english from "../../crates/arto/locales/frontend.en.json";
import { addFallback } from "./i18n";

addFallback(english);

export { locale, t } from "./i18n";
