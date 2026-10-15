// Test-only IPC fixture; never imported by the production entry point.
import { mockIPC } from '@tauri-apps/api/mocks';
import type { Product, Snapshot, StockEntry, Sale, SaleLine } from '../src/types';
import { currentMonth } from '../src/utils';
const state={calls:[] as {action:string;payload:any}[],products:[] as Product[],entries:[] as StockEntry[],sales:[] as Sale[],lines:[] as SaleLine[],failNext:'',ready:false,voiceActive:false,voiceQueue:['How many screens do we have?','How many did we sell this month?'],interruptNext:null as {text:string;language:string}|null,savedPdfs:[] as number[][],savedExcels:[] as number[][]};
const macosControls=new URLSearchParams(location.search).has('mac');
const preferences=JSON.parse(sessionStorage.getItem('mac-preferences')||'{"ai_enabled":true,"theme":"system","ai_pause_reason":""}');
Object.assign(state,{preferences,macosControls});
Object.assign(window,{isTauri:true,__uiTest:state});
const now=()=>new Date().toISOString();
const sum=(rows:any[],key:string)=>rows.reduce((n,r)=>n+(r[key]??0),0);
function snapshot(month=currentMonth()):Snapshot {
  for(const p of state.products){const entries=state.entries.filter(e=>e.product_id===p.id);p.quantity=sum(entries,'remaining_quantity');p.inventory_value_baisa=entries.reduce((n,e)=>n+e.remaining_quantity*(e.unit_cost_baisa??0),0);p.stock_status=p.quantity===0?'out':p.quantity===1?'critical':p.quantity===2?'low':'normal';}
  const monthly=state.lines.filter(l=>l.created_at.startsWith(month));
  return structuredClone({month,products:state.products,entries:state.entries,sales:state.sales,sale_lines:state.lines,monthly_sales_lines:monthly,monthly_summary:state.products.map(p=>{const lines=monthly.filter(l=>l.product_id===p.id);return {product_id:p.id,item_code:p.item_code,product_name:p.name,quantity_sold:sum(lines,'quantity'),quantity_remaining:p.quantity,cost_baisa:sum(lines,'cost_baisa'),revenue_baisa:sum(lines,'revenue_baisa'),profit_baisa:sum(lines,'profit_baisa')};}),low_stock:state.products.filter(p=>p.quantity<=2),summary:{total_products:state.products.length,total_units:sum(state.products,'quantity'),inventory_value_baisa:sum(state.products,'inventory_value_baisa'),known_inventory_value_baisa:sum(state.products,'inventory_value_baisa'),unknown_cost_units:0,monthly_units_sold:sum(monthly,'quantity'),monthly_sales_baisa:sum(monthly,'revenue_baisa'),known_monthly_sales_baisa:sum(monthly,'revenue_baisa'),monthly_cost_baisa:sum(monthly,'cost_baisa'),monthly_profit_baisa:sum(monthly,'profit_baisa'),unknown_sales_lines:0}});
}
let cancelListen:(value:any)=>void=()=>{};
mockIPC(async(command,args)=>{
  if(command!=='api')throw new Error(`Unexpected command ${command}`);
  const {action,payload}=args as {action:string;payload:any};state.calls.push({action,payload:structuredClone(payload)});
  if(state.failNext===action){state.failNext='';throw new Error('Deliberate test backend rejection');}
  if(action==='snapshot')return snapshot(payload.month);
  if(action==='status')return {ready:true,schema_version:2};
  if(action==='assistant_status')return {configured:true,enabled:preferences.ai_enabled,macos_controls:macosControls,retry_available:!preferences.ai_enabled&&['failed','timed_out'].includes(preferences.ai_pause_reason),startup_timeout_seconds:1200,running:preferences.ai_enabled,ready:state.ready&&preferences.ai_enabled,state:preferences.ai_enabled?(state.ready?'ready':'starting'):(preferences.ai_pause_reason==='manual'?'disabled':preferences.ai_pause_reason),message:preferences.ai_enabled?'Starting local AI':'AI is off. Inventory remains available.',voice_configured:true,voice_ready:state.voiceActive,voice:{configured:true,ready:state.voiceActive,active:state.voiceActive,message:'Voice ready'}};
  if(action==='settings_get')return structuredClone(preferences);
  if(action==='settings_theme'){preferences.theme=payload.theme;sessionStorage.setItem('mac-preferences',JSON.stringify(preferences));return {theme:preferences.theme};}
  if(action==='assistant_set_enabled'){preferences.ai_enabled=payload.enabled;preferences.ai_pause_reason=payload.enabled?'':'manual';state.ready=false;state.voiceActive=false;sessionStorage.setItem('mac-preferences',JSON.stringify(preferences));return {configured:true,enabled:preferences.ai_enabled,macos_controls:macosControls,retry_available:false,running:preferences.ai_enabled,ready:false,state:payload.enabled?'starting':'disabled',message:payload.enabled?'Starting AI':'AI is off',voice_configured:true,voice_ready:false};}
  if(action==='save_product'){
    const old=state.products.find(p=>p.id===payload.product.id);
    if(state.products.some(p=>p.item_code===payload.product.item_code&&p!==old))throw new Error('This Item Code already belongs to another product.');
    const product={...payload.product,id:old?.id??crypto.randomUUID(),quantity:old?.quantity??0,inventory_value_baisa:old?.inventory_value_baisa??0,unknown_cost_units:0,stock_status:'out',created_at:old?.created_at??now(),updated_at:now()} as Product;
    if(old)Object.assign(old,product);else state.products.push(product);return product;
  }
  if(action==='stock_entry'){
    let product=state.products.find(p=>p.item_code===payload.item_code);
    if(!product){product={id:crypto.randomUUID(),item_code:payload.item_code,name:payload.name,model_description:payload.model_description,quantity:0,inventory_value_baisa:0,unknown_cost_units:0,stock_status:'out',created_at:now(),updated_at:now()};state.products.push(product);}
    const entry={id:crypto.randomUUID(),product_id:product.id,item_code:product.item_code,product_name:product.name,quantity:payload.quantity,remaining_quantity:payload.quantity,unit_cost_baisa:payload.unit_cost_baisa,created_at:now()};state.entries.push(entry);snapshot();return entry;
  }
  if(action==='stock_out'){
    const saleId=crypto.randomUUID(),saleLines:SaleLine[]=[];
    for(const line of payload.lines){const p=state.products.find(p=>p.item_code===line.item_code);if(!p||p.quantity<line.quantity)throw new Error('Insufficient stock.');let remaining=line.quantity,cost=0;for(const e of state.entries.filter(e=>e.product_id===p.id)){const take=Math.min(remaining,e.remaining_quantity);e.remaining_quantity-=take;remaining-=take;cost+=take*(e.unit_cost_baisa??0);if(!remaining)break;}const revenue=line.quantity*line.unit_price_baisa;saleLines.push({...line,id:crypto.randomUUID(),sale_id:saleId,product_id:p.id,product_name:p.name,model_description:p.model_description,cr_number:payload.cr_number,cost_baisa:cost,revenue_baisa:revenue,profit_baisa:revenue-cost,created_at:now()});}
    const sale={id:saleId,cr_number:payload.cr_number,created_at:now(),total_quantity:sum(saleLines,'quantity'),revenue_baisa:sum(saleLines,'revenue_baisa'),cost_baisa:sum(saleLines,'cost_baisa'),profit_baisa:sum(saleLines,'profit_baisa')};state.sales.unshift(sale);state.lines.unshift(...saleLines);snapshot();return sale;
  }
  if(action==='save_pdf'){state.savedPdfs.push(payload.bytes);return {path:'C:/TestOnly/report.pdf'};}
  if(action==='save_excel'){state.savedExcels.push(payload.bytes);return {path:'C:/TestOnly/report.xlsx'};}
  if(action==='assistant_new_conversation')return {ok:true};
  if(action==='assistant_ask'){if(payload.question==='Hi')return {text:'Long greeting details',brief_text:'Hi! How can I help with your inventory?',intent:'greeting'};if(payload.question==='Bye')return {text:'Bye! Talk to you soon.',intent:'farewell'};return {text:`Fixture reply to: ${payload.question}`};}
  if(action==='voice_start'){state.voiceActive=true;return {configured:true,ready:true,active:true,message:'Voice ready'};}
  if(action==='voice_status')return {configured:true,ready:state.voiceActive,active:state.voiceActive,message:'Voice ready'};
  if(action==='voice_listen'){const text=state.voiceQueue.shift();if(text){await new Promise(r=>setTimeout(r,100));return {text,language:'en'};}return new Promise(resolve=>{cancelListen=resolve;});}
  if(action==='voice_speak'){await new Promise(r=>setTimeout(r,100));if(state.interruptNext){const transcript=state.interruptNext;state.interruptNext=null;return {spoken:false,interrupted:true,transcript};}return {spoken:true};}
  if(action==='voice_stop'){state.voiceActive=false;cancelListen({no_speech:true,text:''});return {ok:true};}
  if(action==='backup')return {path:'C:/TestOnly/backup.db'};
  if(action==='restore')return {cancelled:true};
  throw new Error(`Unimplemented fixture action ${action}`);
});
await import('../src/main');
