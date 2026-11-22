import {normalize as baseNormalize,numericReference} from './utils';
export const searchText=(value:string)=>baseNormalize(value).replace(/ة/g,'ه');
export type LimitedProduct={id:string;item_code:string;name:string;model_description:string;quantity:number};
export type LimitedLine={id:string;product_id:string;created_at:string;cr_number:string|null;product_name:string;item_code:string;model_description:string;quantity:number;unit_price_baisa:number|null;purpose:string;original_cr:string};
export type LimitedEntry={id:string;product_id:string;item_code:string;product_name:string;created_at:string;quantity:number;remaining_quantity:number};
export type Connection={configured:boolean;connected:boolean;url?:string;warning:string;updated_at:string;read_only:true;version:string};
export type LimitedSnapshot={products:LimitedProduct[];sale_lines:LimitedLine[];entries:LimitedEntry[];updated_at:string;connection:Connection};
export type Filters={query:string;cr:string;itemCode:string;from:string;to:string};
export const emptyFilters:Filters={query:'',cr:'',itemCode:'',from:'',to:''};
export const stockStatus=(quantity:number)=>quantity===0?'Out of Stock':quantity===1?'Critical Stock':quantity===2?'Low Stock':'In Stock';
export function filterLimited(snapshot:LimitedSnapshot,input:Filters){
 const cr=input.cr.trim()?numericReference(input.cr,'CR number'):'',code=input.itemCode.trim()?numericReference(input.itemCode):'';
 const terms=searchText(input.query).trim().split(/\s+/).filter(Boolean);if(input.query.length>200)throw new Error('Search text must be at most 200 characters.');
 for(const value of [input.from,input.to])if(value&&(!/^\d{4}-\d{2}-\d{2}$/.test(value)||!Number.isFinite(Date.parse(value+'T12:00:00Z'))||new Date(value+'T12:00:00Z').toISOString().slice(0,10)!==value))throw new Error('Choose a valid date.');
 if(input.from&&input.to&&input.from>input.to)throw new Error('Start date must be before or equal to end date.');
 const matches=(...parts:(string|null)[])=>{const text=searchText(parts.filter(Boolean).join(' '));return terms.every(term=>text.includes(term));};
 const range=(stamp:string)=>{const parts=new Intl.DateTimeFormat('en-CA',{timeZone:'Asia/Muscat',year:'numeric',month:'2-digit',day:'2-digit'}).formatToParts(new Date(stamp));const day=['year','month','day'].map(k=>parts.find(p=>p.type===k)?.value??'').join('-');return (!input.from||day>=input.from)&&(!input.to||day<=input.to);};
 const sales=snapshot.sale_lines.filter(l=>!['personal','damage'].includes(l.purpose)&&(!cr||l.cr_number===cr||l.original_cr===cr)&&(!code||l.item_code===code)&&range(l.created_at)&&matches(l.cr_number,l.original_cr,l.product_name,l.item_code,l.model_description));
 const byId=new Map(snapshot.products.map(p=>[p.id,p]));
 const entries=cr?[]:snapshot.entries.filter(e=>(!code||e.item_code===code)&&range(e.created_at)&&matches(e.item_code,e.product_name,byId.get(e.product_id)?.model_description??''));
 const linked=new Set([...sales,...entries].map(l=>l.product_id));
 const products=snapshot.products.filter(p=>(!code||p.item_code===code)&&(cr||input.from||input.to?linked.has(p.id):matches(p.item_code,p.name,p.model_description)));
 return {sales,products,entries};
}
