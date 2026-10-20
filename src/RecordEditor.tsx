import { createContext, useContext, useState } from 'react';
import type { FormEvent } from 'react';
import type { Product, SaleLine, StockEntry } from './types';
import { call } from './api';
import { numericReference, positiveQuantity, toBaisa } from './utils';
import { Dialog, Field, Notice, errorMessage } from './ui';

export type EditTarget = {kind:'product';record:Product}|{kind:'entry';record:StockEntry}|{kind:'sale';record:SaleLine};
export const RecordEditContext=createContext<((target:EditTarget)=>void)|null>(null);
export const useRecordEditor=()=>useContext(RecordEditContext);
export function DataIssues({issues}:{issues?:string[]}) {
 return issues?.length ? <span className="data-warning" role="note" title={issues.join('\n')} aria-label={`Needs review: ${issues.join(' ')}`}>⚠ Needs review<small>{issues.join(' ')}</small></span> : null;
}
function localDate(value:string){const date=new Date(value);if(Number.isNaN(date.getTime()))return '';const local=new Date(date.getTime()-date.getTimezoneOffset()*60000);return local.toISOString().slice(0,19);}
export default function RecordEditor({target,saved,close}:{target:EditTarget;saved:()=>Promise<void>;close:()=>void}) {
 const {kind,record}=target;
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
  <p>A recovery backup is saved before each correction. Stock, buying costs, profit and reports are recalculated together.</p>
  {!result&&<><div className="form-grid">
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
  <div className="dialog-actions"><button type="button" className="btn" disabled={busy} onClick={close}>{result?'Close':'Cancel'}</button>{!result&&<button className="btn primary" disabled={busy}>{busy?'Saving…':'Save correction'}</button>}</div>
 </form></Dialog>;
}
