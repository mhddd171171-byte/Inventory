import { useState } from 'react';
import type { FormEvent } from 'react';
import type { DamagedReturn, Product, SaleLine } from './types';
import { call } from './api';
import { dateTime, matchesProduct, money, numericReference, positiveQuantity, toBaisa } from './utils';
import { Dialog, Field, Notice, errorMessage } from './ui';

export default function DamagedReturnEditor({line,products,pending,saved,close}:{line:SaleLine;products:Product[];pending?:DamagedReturn;saved:()=>Promise<void>;close:()=>void}) {
 const remaining=line.quantity-(line.returned_quantity??0);
 const [qty,setQty]=useState(String(pending?.quantity??Math.min(1,remaining))),[resolution,setResolution]=useState<'refund'|'replacement'|'pending'>('refund');
 const [refund,setRefund]=useState(line.unit_price_baisa==null?'':((line.unit_price_baisa*(pending?.quantity??1))/1000).toFixed(3)),[extra,setExtra]=useState('0.000');
 const [query,setQuery]=useState(''),[replacement,setReplacement]=useState<Product|null>(null),[note,setNote]=useState(pending?.note??''),[password,setPassword]=useState(''),[busy,setBusy]=useState(false),[error,setError]=useState(''),[result,setResult]=useState('');
 const matches=query.trim()?products.filter(p=>matchesProduct(p,query)):[];
 async function submit(e:FormEvent){e.preventDefault();setBusy(true);setError('');try{
  const quantity=positiveQuantity(qty);
  if(quantity>(pending?.quantity??remaining))throw new Error('Returned quantity exceeds the units eligible for return.');
  await call('damaged_return',{password,line_id:line.id,return_id:pending?.id??null,quantity,resolution,note:note.trim(),refund_baisa:resolution==='pending'?0:toBaisa(refund),extra_baisa:resolution==='replacement'?toBaisa(extra):0,replacement_item_code:resolution==='replacement'?numericReference(replacement?.item_code??''):null});
  setPassword('');await saved();setResult('Damaged return saved. Original CR preserved and a recovery backup created.');
 }catch(e){setError(errorMessage(e));}finally{setBusy(false);}}
 return <Dialog title={pending?'Resolve pending damaged return':'Record damaged return'} close={()=>{if(!busy)close();}}><form className="dialog-body" onSubmit={submit}>
  <Notice text={error}/><Notice text={result} kind="success"/>
  <section className="return-original"><strong>CR {line.cr_number} · {line.product_name}</strong><p>Item Code {line.item_code} · Sold {dateTime(line.created_at)}</p><p>{line.quantity} issued · {line.returned_quantity??0} already returned · Original unit price {money(line.unit_price_baisa)}</p></section>
  {!result&&<><div className="form-grid"><Field label="Returned quantity *"><input required inputMode="numeric" value={qty} readOnly={!!pending} disabled={busy} onChange={e=>{setQty(e.target.value);if(line.unit_price_baisa!=null&&/^\d+$/.test(e.target.value))setRefund((line.unit_price_baisa*Number(e.target.value)/1000).toFixed(3));}}/></Field><Field label="Resolution *"><select aria-label="Resolution *" value={resolution} disabled={busy} onChange={e=>{const next=e.target.value as typeof resolution;setResolution(next);setExtra('0.000');setRefund(next==='refund'&&line.unit_price_baisa!=null?(line.unit_price_baisa*Number(qty)/1000).toFixed(3):'0.000');}}><option value="refund">Refund customer</option><option value="replacement">Give replacement</option>{!pending&&<option value="pending">Pending resolution</option>}</select></Field></div>
  {resolution==='replacement'&&<section className="edit-search"><Field label="Search replacement by code, name or model"><input type="search" dir="auto" value={query} disabled={busy} maxLength={200} onKeyDown={e=>{if(e.key==='Enter')e.preventDefault();}} onChange={e=>{setQuery(e.target.value);setReplacement(null);}}/></Field>{query.trim()&&<div className="edit-search-results">{matches.map(p=><button type="button" className="btn edit-search-result" key={p.id} disabled={busy||p.quantity<Number(qty)} onClick={()=>{setReplacement(p);setQuery('');}}><span>{p.item_code} · {p.name}<small>{p.model_description}</small></span><span>{p.quantity} available</span></button>)}{!matches.length&&<p>No matching item. Try a partial name or model.</p>}</div>}{replacement&&<p role="status">Replacement: {replacement.item_code} · {replacement.name} · {replacement.quantity} available</p>}</section>}
  {resolution!=='pending'&&<div className="form-grid"><Field label={resolution==='refund'?'Refund amount (OMR) *':'Partial refund (OMR) *'}><input required inputMode="decimal" value={refund} disabled={busy} onChange={e=>setRefund(e.target.value)}/></Field>{resolution==='replacement'&&<Field label="Additional payment (OMR) *"><input required inputMode="decimal" value={extra} disabled={busy} onChange={e=>setExtra(e.target.value)}/></Field>}</div>}
  <Field label="Reason / fault *"><textarea required maxLength={2000} rows={2} dir="auto" value={note} disabled={busy} onChange={e=>setNote(e.target.value)}/></Field>
  <p className="form-note">The original sale and CR stay in history. The defective unit does not return to available stock and is not deducted again. {resolution==='replacement'?'A replacement deducts its quantity once, linked to the same CR. Enter only a payment difference, if any.':resolution==='pending'?'No refund is recorded until this return is resolved.':'The refund reduces net sales on the date it is recorded.'}</p>
  <Field label="Edit password *"><input required type="password" autoComplete="off" maxLength={64} value={password} disabled={busy} onChange={e=>setPassword(e.target.value)}/></Field></>}
  <div className="dialog-actions"><button type="button" className="btn" disabled={busy} onClick={close}>{result?'Close':'Cancel'}</button>{!result&&<button className="btn primary" disabled={busy}>{busy?'Saving…':'Save damaged return'}</button>}</div>
 </form></Dialog>;
}
