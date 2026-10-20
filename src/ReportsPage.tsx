import { useState } from 'react';
import { Download } from 'lucide-react';
import { call } from './api';
import type { Snapshot } from './types';
import type { ReportType } from './reports';
import { currentMonth, monthLabel } from './utils';
import { Field, Notice, errorMessage } from './ui';
const choices:{type:ReportType;title:string;description:string}[] = [
  {type:'monthly_sales',title:'Monthly Sales Report',description:'CR-linked product lines, actual prices, FIFO cost and gross profit.'},
  {type:'current_inventory',title:'Current Inventory Report',description:'Product codes, available quantities, purchase value and stock status.'},
  {type:'monthly_inventory_summary',title:'Monthly Inventory Summary',description:'Monthly units sold, current stock remaining, revenue, cost and profit.'},
];
export default function ReportsPage({editInventory,editSales}:{editInventory:()=>void;editSales:()=>void}) {
  const [type,setType] = useState<ReportType>('monthly_sales'), [month,setMonth] = useState(currentMonth()), [error,setError] = useState(''), [result,setResult] = useState(''), [busy,setBusy] = useState(false);
  async function exportReport(excel:boolean) {
    setError('');setResult('');setBusy(true);
    try {const snapshot = await call<Snapshot>('snapshot',{month});const reports = await import('./reports');const bytes = excel ? await reports.buildExcelReport(type,snapshot,{month}) : await reports.buildReport(type,snapshot,{month});const extension = excel ? 'xlsx' : 'pdf';const saved = await call<{path?:string;cancelled?:boolean}>(excel ? 'save_excel' : 'save_pdf',{bytes:Array.from(bytes),suggested_name:`${type}-${type === 'current_inventory' ? new Date().toISOString().slice(0,10) : month}.${extension}`});if(saved.path) setResult(`Report saved to ${saved.path}`);}
    catch(e) {setError(errorMessage(e));} finally {setBusy(false);}
  }
  return <div className="report-grid"><div className="panel padded"><h2>Choose a report</h2><div className="report-options">{choices.map(r => <label className={`report-option ${r.type === type ? 'chosen' : ''}`} key={r.type}><input type="radio" name="report" checked={r.type === type} onChange={() => setType(r.type)} disabled={busy}/><div><strong>{r.title}</strong><small>{r.description}</small></div></label>)}</div></div><div className="panel padded report-settings"><h2>Report options</h2><p>To correct a report, edit its saved source records. New exports use the corrected values.</p><div className="button-row"><button className="btn small" onClick={editInventory}>Edit inventory records</button><button className="btn small" onClick={editSales}>Edit sales history</button></div><Notice text={error}/><Notice text={result} kind="success"/><Field label="Reporting Month"><input type="month" required value={month} onChange={e => setMonth(e.target.value)} disabled={busy || type === 'current_inventory'}/></Field><p className="help-note">{type === 'current_inventory' ? 'Current inventory shows the available balance at generation time.' : `Sales and financial totals cover ${monthLabel(month)}. Remaining quantity is the current available stock.`}</p><div className="button-row"><button className="btn primary" disabled={busy} onClick={() => void exportReport(false)}><Download size={16}/>{busy ? 'Creating report…' : 'Export PDF'}</button><button className="btn" disabled={busy} onClick={() => void exportReport(true)}><Download size={16}/>Export Excel (.xlsx)</button></div><p className="form-note">Choose a local save location. Reports use your recorded data and OMR with three decimal places.</p></div></div>;
}
