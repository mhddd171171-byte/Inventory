import { invoke, isTauri } from "@tauri-apps/api/core";
export const isDesktop = isTauri();
export async function call<T = unknown>(
  action: string,
  payload: Record<string, unknown> = {},
): Promise<T> {
  if (!isDesktop)
    throw new Error(
      "Open Inventory Manager from the desktop application. The local database is available through its native backend.",
    );
  return invoke<T>("api", { action, payload });
}
