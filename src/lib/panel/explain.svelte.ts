// Novice mode's settings as the drawer's tabs share them (P6, adr.rg.023): the Settings
// tab changes them, the Diff tab reads them to know whether a hunk gets an Explain
// button. Module state, loaded from the core on first use.
import { invoke } from "@tauri-apps/api/core";
import type { ExplainView } from "./types";

export const explain = $state<{ view: ExplainView | null; error: string | null }>({
  view: null,
  error: null,
});

export async function loadExplain(): Promise<ExplainView | null> {
  try {
    explain.view = await invoke<ExplainView>("explain_settings");
    explain.error = null;
  } catch (e) {
    explain.error = e instanceof Error ? e.message : String(e);
  }
  return explain.view;
}

/** A backend is chosen and complete: hunks get an Explain button. */
export function explainReady(): boolean {
  return !!explain.view && explain.view.config.backend !== "off" && !!explain.view.label;
}
