import { useState } from 'react';
import { Download, Search } from 'lucide-react';
import { call } from './api';
import type { Product, Snapshot } from './types';
import { emptySearch, searchSnapshot, searchSuggestions, type SearchFilters } from './search';
import { currentMonth, dateTime, money } from './utils';
import { Empty, Field, Notice, ProductTable, SalesTable, StockEntriesTable, errorMessage } from './ui';

export default function SearchPage({snapshot,select}:{snapshot:Snapshot;select:(product:Product)=>void}) {
  const [filters,setFilters]=useState<SearchFilters>({...emptySearch}),[busy,setBusy]=useState(false),[error,setError]=useState(''),[saved,setSaved]=useState('');
  const set=(key:keyof SearchFilters,value:string)=>{setFilters(f=>({...f,[key]:value}));setSaved('');};
  let validation='',results;try {results=searchSnapshot(snapshot,filters);} catch(e) {validation=errorMessage(e);}
  const suggestions=searchSuggestions(snapshot);
  async function exportResults(excel:boolean) {
    setBusy(true);setError('');setSaved('');
    try {
      const fresh=await call<Snapshot>('snapshot',{month:currentMonth()});
      const matches=searchSnapshot(fresh,filters),reports=await import('./reports');
      const model=reports.createSearchReportModel(matches),bytes=excel ? await reports.renderExcel(model) : await reports.buildModelPdf(model);
      const suffix=[matches.filters.cr&&`cr-${matches.filters.cr}`,matches.filters.itemCode&&`item-${matches.filters.itemCode}`].filter(Boolean).join('-')||'results';
      const result=await call<{path?:string;cancelled?:boolean}>(excel?'save_excel':'save_pdf',{bytes:Array.from(bytes),suggested_name:`search-${suffix}.${excel?'xlsx':'pdf'}`});
      if(result.path) setSaved(`Search results saved to ${result.path}`);
    } catch(e) {setError(errorMessage(e));} finally {setBusy(false);}
  }
  return <div className="settings-stack">
    <div className="panel padded"><div className="panel-title"><div><h2><Search size={18}/> Search your records</h2><p>Find which items were sold under a CR, or trace an item's stock and sales history.</p></div></div>
      <div className="search-fields"><Field label="Name, model or any reference"><input disabled={busy} maxLength={200} list="search-names" value={filters.query} onChange={e=>set('query',e.target.value)} placeholder="e.g. Samsung S3, screen, بطارية" dir="auto"/></Field>
        <Field label="Exact CR number"><input disabled={busy} inputMode="numeric" maxLength={35} list="search-crs" value={filters.cr} onChange={e=>set('cr',e.target.value)} placeholder="Customer CR"/></Field>
        <Field label="Exact Item Code"><input disabled={busy} inputMode="numeric" maxLength={35} list="search-codes" value={filters.itemCode} onChange={e=>set('itemCode',e.target.value)} placeholder="Product code"/></Field>
        <Field label="From date"><input disabled={busy} type="date" value={filters.from} onChange={e=>set('from',e.target.value)}/></Field><Field label="To date"><input disabled={busy} type="date" value={filters.to} onChange={e=>set('to',e.target.value)}/></Field></div>
      <datalist id="search-crs">{suggestions.cr.map(v=><option key={v} value={v}/>)}</datalist><datalist id="search-codes">{suggestions.codes.map(v=><option key={v} value={v}/>)}</datalist><datalist id="search-names">{suggestions.names.map(v=><option key={v} value={v}/>)}</datalist>
      <p className="help-note">Search by CR, Item Code, partial product name, device model or Arabic description. Combine CR + Item Code to check a specific part sold to that CR. Date filters use your computer's local dates. Suggestions come from saved records. Tables show up to 100 rows per section; exports include every matching row.</p>
      <div className="button-row"><button className="btn" disabled={busy} onClick={()=>{setFilters({...emptySearch});setSaved('');}}>Clear filters</button><button className="btn primary" disabled={busy||!!validation} onClick={()=>void exportResults(false)}><Download size={16}/>{busy?'Creating report…':'Export search PDF'}</button><button className="btn" disabled={busy||!!validation} onClick={()=>void exportResults(true)}><Download size={16}/>Export search Excel</button></div>
      <Notice text={validation||error}/><Notice text={saved} kind="success"/>
    </div>
    {results&&<><div className="panel"><div className="panel-title"><div><h2>Items sold · {results.sales.length} lines</h2><p>Selling price is what the customer paid per unit. Total buying cost is what you paid for the sold units, using the oldest batches first (FIFO).</p></div></div>{results.sales.length?<SalesTable lines={results.sales.slice(0,100)}/>:<Empty title="No matching sales" text="No saved sale line matches all these filters."/>}{results.sales.length>100&&<div className="table-footer">Showing the first 100 lines. Exports include all {results.sales.length} matching lines.</div>}</div>
      <div className="panel"><div className="panel-title"><div><h2>Matching products · {results.products.length}</h2><p>Available quantities and purchase values are current balances.</p></div></div>{results.products.length?<ProductTable products={results.products.slice(0,100)} select={select}/>:<Empty title="No matching products" text="Try a partial name or model; an Item Code is not required."/>}</div>
      {!results.filters.cr&&<div className="panel"><div className="panel-title"><div><h2>Stock entries · {results.entries.length}</h2><p>Original buying price, total amount paid and quantity remaining for each batch.</p></div></div>{results.entries.length?<StockEntriesTable entries={results.entries.slice(0,100)}/>:<Empty title="No matching stock entries" text="Incoming deliveries do not have customer CR numbers."/>}</div>}</>}
  </div>;
}
