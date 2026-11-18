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
  if(action==='restore'&&!window.confirm('This replaces the shared ONLINE database for ALL connected devices. A recovery backup is created first. Continue?'))return {cancelled:true} as T;
  if(action==='restore')payload={...payload,confirm_shared_restore:true};
  const result=await invoke<T>("api", { action, payload });
  if(["save_product","stock_entry","stock_out","edit_record","delete_record","damaged_return","warranty","restore"].includes(action)) window.dispatchEvent(new Event("inventory-data-safety"));
  return result;
}
