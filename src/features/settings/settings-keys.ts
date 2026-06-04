/**
 * Known setting keys.
 *
 * Centralised in a sibling module (not SettingsPage.tsx) so the page, the tests
 * and any future caller agree on the wire names — and so the page file only
 * exports its component (keeps React Fast Refresh working). Values are always
 * strings (the backend store is a TEXT key/value map).
 */

export const SETTING_KEYS = {
  locale: "locale",
  anthropicApiKey: "anthropic_api_key",
  anthropicKeyInKeychain: "anthropic_api_key_in_keychain",
  cloudAiEnabled: "cloud_ai_enabled",
  cloudBackupEnabled: "cloud_backup_enabled",
  telemetryEnabled: "telemetry_enabled",
} as const;
