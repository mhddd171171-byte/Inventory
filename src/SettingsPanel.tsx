import { useEffect, useState } from 'react';
import BackupSafety from './BackupSafety';
import DriveBackup from './DriveBackup';
import { call } from './api';
import type { AssistantStatus } from './types';
import { aiLabel } from './AssistantPanel';
import { Notice, errorMessage } from './ui';

export type Preferences = {ai_enabled:boolean;theme:'system'|'light'|'dark';ai_pause_reason:string};
export function applyTheme(theme:Preferences['theme']) {
  const dark=theme==='dark'||theme==='system'&&window.matchMedia('(prefers-color-scheme: dark)').matches;
  document.documentElement.dataset.theme=dark?'dark':'light';
}
export default function SettingsPanel({status,onStatus,onImport}:{status:AssistantStatus|null;onStatus:(s:AssistantStatus)=>void;onImport:()=>void}) {
  const [preferences,setPreferences]=useState<Preferences|null>(null),[error,setError]=useState(''),[busy,setBusy]=useState(false);
  useEffect(()=>{let live=true;call<Preferences>('settings_get').then(p=>{if(live)setPreferences(p);}).catch(e=>{if(live)setError(errorMessage(e));});return()=>{live=false;};},[]);
  async function setAI(enabled:boolean) {
    setBusy(true);setError('');
    try {const next=await call<AssistantStatus>('assistant_set_enabled',{enabled});onStatus(next);setPreferences(p=>p?{...p,ai_enabled:enabled}:p);}
    catch(e){setError(errorMessage(e));}finally{setBusy(false);}
  }
  async function setTheme(theme:Preferences['theme']) {
    setBusy(true);setError('');
    try {await call('settings_theme',{theme});applyTheme(theme);window.dispatchEvent(new CustomEvent('inventory-theme',{detail:theme}));setPreferences(p=>p?{...p,theme}:p);}
    catch(e){setError(errorMessage(e));}finally{setBusy(false);}
  }
  return <div className="dialog-body settings-stack"><Notice text={error}/><section className="preference-section"><h2>Import / restore database</h2><p>Load an Inventory Manager .db backup, including one copied from another computer. This replaces all current records; it does not merge them. The application validates the file and saves a recoverable copy of your current database first.</p><p>Excel and CSV files cannot be restored here.</p><button className="btn primary" onClick={onImport}>Import database (.db)</button></section><section className="preference-section"><h2>Local AI and voice</h2><p>Turn AI off to free memory and keep using inventory, search and reports. This choice is saved for the next launch.</p><label className="ai-toggle"><input type="checkbox" role="switch" aria-label="Enable local AI and voice" checked={status?.enabled??preferences?.ai_enabled??false} disabled={busy||!preferences} onChange={e=>void setAI(e.target.checked)}/><strong>Enable local AI and voice</strong></label><p role="status">{aiLabel(status)}</p><p>{status?.message}</p><p>On this computer, an AI startup attempt stops after 20 minutes or an earlier failure. It stays off until you enable it or select Retry AI.</p>{status?.retry_available&&<button className="btn" disabled={busy} onClick={()=>void setAI(true)}>Retry AI</button>}</section><section className="preference-section"><h2>Appearance</h2><label className="field"><span>Theme</span><select aria-label="Theme" value={preferences?.theme??'system'} disabled={!preferences||busy} onChange={e=>void setTheme(e.target.value as Preferences['theme'])}><option value="system">Follow system</option><option value="light">Light</option><option value="dark">Dark</option></select></label></section><BackupSafety/><DriveBackup/><section className="preference-section"><h2>Your data</h2><p>Your working database stays on this computer. Optional Google Drive backup uploads a separate copy only after you connect it. Turning AI off does not change your products, stock or sales. Use Backup &amp; restore in the sidebar to save a copy of your database.</p></section></div>;
}
