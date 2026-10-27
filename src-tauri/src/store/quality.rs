use super::*;
pub(super) fn annotate(conn:&Connection,products:&mut [Value],entries:&mut [Value],lines:&mut [Value])->Result<Value,String>{
 let generated=query(conn,"SELECT a.record_id FROM legacy_archive a WHERE a.kind='spreadsheet_opening_stock' AND EXISTS(SELECT 1 FROM products p WHERE p.id=a.record_id) AND NOT EXISTS(SELECT 1 FROM migration_info m WHERE m.key='reviewed-item-code:'||a.record_id AND m.value='true')",|r|Ok(json!(r.get::<_,String>(0)?)))?;
 let generated:HashSet<String>=generated.into_iter().filter_map(|v|v.as_str().map(str::to_owned)).collect();
 let mut flagged_products=0;let mut flagged_entries=0;let mut flagged_sales=0;
 for p in products{let mut issues=Vec::new();if generated.contains(p["id"].as_str().unwrap_or("")){issues.push("Item Code was not supplied in the import; an automatic code was assigned. Confirm or correct it with Edit.");}if p["unknown_cost_units"].as_i64().unwrap_or(0)>0{issues.push("Buying price is missing for some stock. Edit the stock entry to record the actual price.");}if !issues.is_empty(){flagged_products+=1;}p["data_issues"]=json!(issues);}
 for e in entries{let issues=if e["unit_cost_baisa"].is_null(){vec!["Buying price is missing; it has not been treated as zero."]}else{vec![]};if !issues.is_empty(){flagged_entries+=1;}e["data_issues"]=json!(issues);}
 for l in lines{let mut issues=Vec::new();if l["cr_number"].is_null()&&l["purpose"]!="personal"&&l["purpose"]!="damage"{issues.push("Customer CR is missing.");}if l["unit_price_baisa"].is_null(){issues.push("Selling price is missing.");}if l["cost_baisa"].is_null(){issues.push("Buying cost is unknown; correct the original stock entry.");}if !issues.is_empty(){flagged_sales+=1;}l["data_issues"]=json!(issues);}
 Ok(json!({"products":flagged_products,"stock_entries":flagged_entries,"sale_lines":flagged_sales,"generated_item_codes":generated.len()}))
}
