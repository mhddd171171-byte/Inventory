import { createContext, useContext, useState } from 'react';
import type { FormEvent } from 'react';
import type { Product, SaleLine, StockEntry } from './types';
import { call } from './api';
import { matchesProduct, numericReference, positiveQuantity, toBaisa } from './utils';
import { Dialog, Field, Notice, errorMessage } from './ui';

export type EditTarget = {kind:'product';record:Product}|{kind:'entry';record:StockEntry}|{kind:'sale';record:SaleLine};
export const RecordEditContext=createContext<((target:EditTarget)=>void)|null>(null);
export const useRecordEditor=()=>useContext(RecordEditContext);
export function DataIssues({issues}:{issues?:string[]}) {
 return issues?.length ? <span className="data-warning" role="note" title={issues.join('\n')} aria-label={`Needs review: ${issues.join(' ')}`}>⚠ Needs review<small>{issues.join(' ')}</small></span> : null;
}
function localDate(value:string){const date=new Date(value);if(Number.isNaN(date.getTime()))return '';const local=new Date(date.getTime()-date.getTimezoneOffset()*60000);return local.toISOString().slice(0,19);}
export default function RecordEditor({target,products,saved,close}:{target:EditTarget;products:Product[];saved:()=>Promise<void>;close:()=>void}) {
 const {kind,record}=target;
 const [confirmDelete,setConfirmDelete]=useState(false);
 async function remove(){
  setBusy(true);setError('');
  try{const response=await call<{pre_delete_backup:string}>('delete_record',{kind,id:record.id,password,confirmed:true});setPassword('');await saved();setResult(`Record deleted. Recovery backup: ${response.pre_delete_backup}`);setConfirmDelete(false);}
  catch(e){setError(errorMessage(e));}finally{setBusy(false);}
 }
 const openEditor=useRecordEditor();
 const [search,setSearch]=useState(''),[switchProduct,setSwitchProduct]=useState<Product|null>(null);
 const matches=search.trim()?products.filter(p=>matchesProduct(p,search)):[];
 const [code,setCode]=useState(record.item_code),[name,setName]=useState(kind==='product'?record.name:''),[model,setModel]=useState(kind==='product'?record.model_description:'');
 const [quantity,setQuantity]=useState(kind==='product'?'':String(record.quantity));
 const originalPrice=kind==='entry'?record.unit_cost_baisa:kind==='sale'?record.unit_price_baisa:null;
 const [price,setPrice]=useState(originalPrice==null?'':(originalPrice/1000).toFixed(3)),[cr,setCr]=useState(kind==='sale'?record.cr_number??'':'');
 const [date,setDate]=useState(localDate(record.created_at)),[password,setPassword]=useState(''),[busy,setBusy]=useState(false),[error,setError]=useState(''),[result,setResult]=useState('');
 async function submit(event:FormEvent){
  event.preventDefault();setBusy(true);setError('');setResult('');
  try{
   const changes:Record<string,unknown>={item_code:numericReference(code)};
   if(kind==='product'){changes.name=name.trim();changes.model_description=model.trim();}
   else{changes.quantity=positiveQuantity(quantity);changes.created_at=new Date(date).toISOString();if(kind==='entry')changes.unit_cost_baisa=toBaisa(price);else{changes.unit_price_baisa=toBaisa(price);changes.cr_number=numericReference(cr,'CR Number');}}
   const response=await call<{pre_edit_backup:string}>('edit_record',{kind,id:record.id,password,changes});
   setPassword('');await saved();setResult(`Correction saved. Recovery backup: ${response.pre_edit_backup}`);
  }catch(e){setError(errorMessage(e));}finally{setBusy(false);}
 }
 return <Dialog title={kind==='product'?'Edit product':kind==='entry'?'Edit stock entry':'Edit Stock Out'} close={()=>{if(!busy)close();}}><form className="dialog-body" onSubmit={submit}>
  <Notice text={error}/><Notice text={result} kind="success"/>
  <DataIssues issues={record.data_issues}/>
  <p>A recovery backup is saved before each correction or deletion. Stock, buying costs, profit and reports are recalculated together.</p>
  {!result&&<><section className="edit-search" aria-label="Search inventory inside editor">
   <Field label="Search item by code, name or model"><input type="search" maxLength={200} dir="auto" value={search} placeholder="Item Code, Samsung screen, device model…" disabled={busy} onKeyDown={e=>{if(e.key==='Enter')e.preventDefault();}} onChange={e=>{setSearch(e.target.value);setSwitchProduct(null);}}/></Field>
   {search.trim()&&<><p className="help-note" role="status">{matches.length?`${matches.length} matching items${matches.length>8?' — showing the first 8; refine your search':''}`:'No matching item. Try a partial name or model.'}</p><div className="edit-search-results">{matches.slice(0,8).map(p=><button key={p.id} type="button" className="btn edit-search-result" disabled={busy||kind==='product'&&p.id===record.id} onClick={()=>{if(kind==='product')setSwitchProduct(p);else{setCode(p.item_code);setSearch('');}}}><span dir="auto"><strong>{p.item_code} · {p.name}</strong><small>{p.model_description}</small></span><span>{kind==='product'?p.id===record.id?'Currently editing':'Edit this product':'Use this item'}</span></button>)}</div></>}
   {switchProduct&&<div className="notice migration" role="status"><p>Open Item Code {switchProduct.item_code} for editing? Unsaved changes in this window will be discarded. No database records change until you enter the password and save.</p><div className="button-row"><button type="button" className="btn" disabled={busy} onClick={()=>{openEditor?.({kind:'product',record:switchProduct});}}>Open selected product</button><button type="button" className="btn" onClick={()=>setSwitchProduct(null)}>Keep editing this record</button></div></div>}
   {kind!=='product'&&<p className="help-note">Selecting an item changes this record's Item Code. Quantity, price and date stay as entered until you save.</p>}
  </section><div className="form-grid">
   <Field label="Item Code *"><input required inputMode="numeric" maxLength={30} value={code} onChange={e=>setCode(e.target.value)} disabled={busy}/></Field>
   {kind==='product'?<><Field label="Product Name *"><input required maxLength={200} value={name} dir="auto" onChange={e=>setName(e.target.value)} disabled={busy}/></Field><Field label="Model / Description *" wide><textarea required maxLength={2000} value={model} dir="auto" onChange={e=>setModel(e.target.value)} disabled={busy}/></Field><p className="form-note">To correct quantity or buying price, edit the purchase batch in Stock Entry. Historical sales keep their recorded names unless you edit the Stock Out record.</p></>:<>
    <Field label="Quantity *"><input required inputMode="numeric" value={quantity} onChange={e=>setQuantity(e.target.value)} disabled={busy}/></Field>
    <Field label={kind==='entry'?'Buying Price per Unit (OMR) *':'Selling Price per Unit (OMR) *'}><input required inputMode="decimal" placeholder="Unknown — enter actual price" value={price} onChange={e=>setPrice(e.target.value)} disabled={busy}/></Field>
    {kind==='sale'&&<Field label="Customer CR Number *"><input required inputMode="numeric" maxLength={30} value={cr} onChange={e=>setCr(e.target.value)} disabled={busy}/></Field>}
    <Field label="Date and time *"><input required type="datetime-local" step="1" value={date} onChange={e=>setDate(e.target.value)} disabled={busy}/></Field>
   </>}
   <Field label="Edit password *" wide><input required type="password" autoComplete="off" maxLength={64} value={password} onChange={e=>setPassword(e.target.value)} disabled={busy}/></Field>
  </div>
  {kind==='sale'&&<p className="help-note">CR number and date apply to every line in this Stock Out transaction. Quantity, item and selling price apply to this line.</p>}
  <p className="help-note">Corrections that leave insufficient stock or invalid records will be rejected. Missing prices must be entered as their actual values; zero means the actual price was zero.</p></>}
  {!result&&confirmDelete&&<div className="notice migration" role="alert"><strong>Delete this saved record?</strong><p>Item Code {record.item_code} · {kind==='product'?record.name:record.product_name}</p><p>{kind==='sale'?'Only this Stock Out line will be deleted. Its quantity returns to stock; other lines in the CR stay.':kind==='entry'?'This purchase batch will be removed. Deletion is rejected if recorded sales would have insufficient stock.':'Only this product will be deleted. Products with stock or sales history must have those records removed first.'} Reports and stock totals will update. Unsaved edits will be discarded. A recovery backup is created first.</p><div className="button-row"><button type="button" className="btn" disabled={busy||!password} onClick={remove}>{busy?'Deleting…':'Confirm delete'}</button><button type="button" className="btn" disabled={busy} onClick={()=>setConfirmDelete(false)}>Keep record</button></div><p>Enter the password above to confirm.</p></div>}
  <div className="dialog-actions">{!result&&<button type="button" className="btn" disabled={busy} onClick={()=>setConfirmDelete(true)}>Delete record</button>}<button type="button" className="btn" disabled={busy} onClick={close}>{result?'Close':'Cancel'}</button>{!result&&!confirmDelete&&<button className="btn primary" disabled={busy}>{busy?'Saving…':'Save correction'}</button>}</div>
 </form></Dialog>;
}
