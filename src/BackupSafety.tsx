import { useEffect, useState } from 'react';
import { call } from './api';
import { Notice, errorMessage } from './ui';
type Safety={version:string;database_directory?:string;directory:string;additional_directory:string|null;latest_backup:string|null;backup_count:number;warning:string;recovery_pending:boolean;database_was_missing:boolean};
function useSafety(enabled=true) {
  const [status,setStatus]=useState<Safety|null>(null),[error,setError]=useState('');
  useEffect(()=>{if(!enabled)return;let live=true;const update=()=>{void call<Safety>('data_safety_status').then(s=>{if(live)setStatus(s);}).catch(e=>{if(live)setError(errorMessage(e));});};update();const timer=setInterval(update,5000);window.addEventListener('inventory-data-safety',update);return()=>{live=false;clearInterval(timer);window.removeEventListener('inventory-data-safety',update);};},[enabled]);
  return {status,error,setStatus,setError};
}
export function DataSafetyNotice({enabled}:{enabled:boolean}) {
  const {status,error}=useSafety(enabled);
  if(!enabled)return null;
  const message=status?.recovery_pending?'Earlier backups were found, but the live database was missing. Open Backup & restore before entering new records. Your earlier backups have been preserved.':status?.database_was_missing?'A new local database was opened. If you used this app before, check your previous data and backups before entering new records.':'';
  return <><Notice text={message}/><Notice text={status?.warning||error}/></>;
}
export default function BackupSafety() {
  const {status,error,setStatus,setError}=useSafety();const [busy,setBusy]=useState(false);
  async function act(action:string) {setBusy(true);setError('');try{const next=await call<Safety>(action);if(action!=='backup_open_folder')setStatus(next);window.dispatchEvent(new Event('inventory-data-safety'));}catch(e){setError(errorMessage(e));}finally{setBusy(false);}}
  return <section className="preference-section"><h2>Automatic backups</h2><p>Version {status?.version||'…'}</p><Notice text={error||status?.warning||''}/><p>Every saved product change, Stock Entry and Stock Out is copied automatically to a separate backup file. Recent versions and daily history are kept.</p>{status?.database_directory&&<p><strong>Inventory data:</strong> {status.database_directory}</p>}<p><strong>Backup folder:</strong> {status?.directory||'Loading…'}</p><p><strong>Latest backup:</strong> {status?.latest_backup?.split(/[\\/]/).at(-1)||'No automatic backup yet'}</p>{status?.additional_directory&&<p><strong>Additional copy:</strong> {status.additional_directory}</p>}<div className="dialog-actions"><button className="btn" disabled={busy} onClick={()=>void act('backup_open_folder')}>Open backup folder</button><button className="btn" disabled={busy||status?.recovery_pending} onClick={()=>void act('backup_now')}>Back up now</button><button className="btn" disabled={busy} onClick={()=>void act('backup_choose_folder')}>Add USB / backup folder</button></div><p>Use Choose backup to restore in Backup &amp; restore to recover a .db file. Restore replaces current records. Keep an additional copy on another drive to protect against loss of this computer or its disk.</p></section>;
}
