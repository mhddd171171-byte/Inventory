import type { SaleLine } from './types';
/** Read-only presentation of original sales and dated return settlements. */
export function financialLines(lines:SaleLine[]):SaleLine[]{
 const result:SaleLine[]=[];
 for(const l of lines){
  if(!['personal','damage'].includes(l.purpose??'sale'))result.push(l);
  for(const r of l.returns??[]){
   if(!r.resolved_at)continue;
   const net=r.extra_baisa-r.refund_baisa;
   result.push({...l,id:`return:${r.id}`,purpose:'return_settlement',created_at:r.resolved_at,quantity:0,unit_price_baisa:null,revenue_baisa:net,cost_baisa:0,profit_baisa:net,returns:[],returned_quantity:0,product_name:`${l.product_name} — ${r.resolution} settlement (${r.quantity} returned)`});
  }
 }
 return result;
}
