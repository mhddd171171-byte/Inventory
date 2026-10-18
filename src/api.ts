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
  const result=await invoke<T>("api", { action, payload });
  if(["save_product","stock_entry","stock_out","restore"].includes(action)) window.dispatchEvent(new Event("inventory-data-safety"));
  return result;
}
