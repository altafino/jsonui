import pkg from "../../package.json";

/** Shown in About. Kept in step with package.json, Cargo.toml and tauri.conf.json. */
export const APP_VERSION = pkg.version;

export const AUTHOR = "Stefan";

export const SITE_LABEL = "altafino.com";

export const TAGLINE = "Edit JSON, YAML and TOML settings as forms. Every save makes a backup first.";
