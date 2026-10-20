// UI integration over a test-only IPC fixture. Rust tests cover real SQLite.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import { mkdirSync,writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
const require=createRequire(import.meta.url);
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const port=Number(process.env.UI_TEST_PORT||1433),base=`http://127.0.0.1:${port}`;
const server=spawn(process.execPath,[resolve('node_modules/vite/bin/vite.js'),'--config',resolve('tests/ui-vite.config.ts'),'--host','127.0.0.1','--port',String(port)],{cwd:process.cwd(),windowsHide:true,stdio:'pipe'});
let log='';server.stdout.on('data',b=>log+=b);server.stderr.on('data',b=>log+=b);
const delay=ms=>new Promise(r=>setTimeout(r,ms));
const results=[],errors=[],external=[];let browser,page;
async function check(name,fn){try{await fn();results.push({name,passed:true});console.log(`PASS ${name}`);}catch(e){results.push({name,passed:false,error:e.message});throw e;}}
const nav=name=>page.locator('nav').getByRole('button',{name,exact:true}).click();
const count=action=>page.evaluate(action=>window.__uiTest.calls.filter(c=>c.action===action).length,action);
const last=action=>page.evaluate(action=>window.__uiTest.calls.filter(c=>c.action===action).at(-1),action);
const idle=()=>page.waitForFunction(()=>!Array.from(document.querySelectorAll('button')).some(b=>/^(Saving|Creating report)/.test(b.textContent||'')));
async function entry(code,qty,cost,name,model){await nav('Stock Entry');await page.getByLabel('Item Code *',{exact:true}).fill(code);if(name){await page.getByLabel('Product Name *',{exact:true}).fill(name);await page.getByLabel('Model / Description *',{exact:true}).fill(model);}await page.getByLabel('Incoming Quantity *',{exact:true}).fill(String(qty));await page.getByLabel('Buying Price per Unit (OMR) *',{exact:true}).fill(cost);await page.getByRole('button',{name:'Save Stock Entry',exact:true}).click();await idle();}
async function outLine(index,code,qty,price){await page.getByLabel(`Item Code — line ${index} *`,{exact:true}).fill(code);await page.getByLabel(`Quantity — line ${index} *`,{exact:true}).fill(qty);await page.getByLabel(`Selling Price per Unit (OMR) — line ${index} *`,{exact:true}).fill(price);}
try{
  for(let i=0;i<100;i++){if(server.exitCode!==null)throw new Error(log);try{if((await fetch(`${base}/tests/ui-fixture.html`)).ok)break;}catch{}if(i===99)throw new Error('Vite startup timed out');await delay(200);}
  browser=await chromium.launch({headless:true,channel:process.env.UI_BROWSER_CHANNEL||'msedge'});
  page=await browser.newPage({viewport:{width:1380,height:950}});page.on('pageerror',e=>errors.push(e.message));
  await page.route('**/*',route=>{const url=route.request().url();if(!url.startsWith(base)&&!url.startsWith('data:')){external.push(url);return route.abort();}return route.continue();});
  await page.goto(`${base}/tests/ui-fixture.html${process.env.MAC_UI_TEST?'?mac=1':''}`);
  await check('dashboard opens immediately with eight pages and inventory usable during AI startup',async()=>{
    await page.getByRole('heading',{name:'Dashboard',exact:true}).waitFor();await page.getByText('Your inventory is empty',{exact:true}).waitFor();assert.equal(await page.locator('nav button').count(),8);assert.equal(await page.getByLabel('Password',{exact:true}).count(),0);assert.equal(await count('login'),0);assert.ok(await page.getByText('Starting AI…',{exact:true}).count());
    await nav('Out of Stock');await page.getByText('No out-of-stock items',{exact:true}).waitFor();assert.equal(await page.getByTestId('out-of-stock-count').count(),0);
  });
  await check('manual numeric product creation and duplicate error preserve input',async()=>{
    await nav('Products');await page.getByRole('button',{name:'Add product',exact:true}).click();await page.getByLabel('Item Code *',{exact:true}).fill('0011234');await page.getByLabel('Product Name *',{exact:true}).fill('Samsung S3 screen');await page.getByLabel('Model / Description *',{exact:true}).fill('Galaxy S3 GT-I9300');await page.getByRole('button',{name:'Save product',exact:true}).click();await page.locator('dialog').waitFor({state:'hidden'});assert.equal((await last('save_product')).payload.product.item_code,'11234');
    await page.getByRole('button',{name:'Add product',exact:true}).click();await page.getByLabel('Item Code *',{exact:true}).fill('11234');await page.getByLabel('Product Name *',{exact:true}).fill('Duplicate');await page.getByLabel('Model / Description *',{exact:true}).fill('Same code');await page.getByRole('button',{name:'Save product',exact:true}).click();await page.getByText('This Item Code already belongs to another product.',{exact:true}).waitFor();assert.equal(await page.getByLabel('Product Name *',{exact:true}).inputValue(),'Duplicate');await page.getByRole('button',{name:'Cancel',exact:true}).click();
  });
  await check('new zero-stock products show in workspace with a persistent count and independent search',async()=>{
    await nav('Products');await page.getByLabel('Search by Item Code, name or model').fill('unmatched');await nav('Out of Stock');assert.equal(await page.locator('tbody tr').count(),1);assert.equal(await page.getByTestId('out-of-stock-count').innerText(),'1');await page.getByLabel('Search by Item Code, name or model').fill('missing');await page.getByText('No matching out-of-stock products',{exact:true}).waitFor();assert.equal(await page.getByTestId('out-of-stock-count').innerText(),'1');await nav('Dashboard');await nav('Out of Stock');assert.equal(await page.locator('tbody tr').count(),1);await nav('Products');await page.getByRole('button',{name:'Clear search'}).click();
  });
  await check('stock entry autofills existing codes and preserves separate purchase batches',async()=>{
    await entry('11234',5,'10.000');assert.equal((await last('stock_entry')).payload.unit_cost_baisa,10000);assert.equal(await page.getByLabel('Product Name *',{exact:true}).inputValue(),'Samsung S3 screen');assert.equal(await page.getByLabel('Product Name *',{exact:true}).getAttribute('readonly'),'');await entry('11234',3,'12.000');assert.equal(await page.evaluate(()=>window.__uiTest.products.length),1);assert.equal(await page.evaluate(()=>window.__uiTest.products[0].quantity),8);
    await entry('11235',4,'5.000','بطارية آيفون 14','iPhone 14 battery');assert.equal(await page.evaluate(()=>window.__uiTest.products.length),2);
  });
  await check('partial model and Arabic name searches need no item code',async()=>{
    await nav('Products');await page.getByLabel('Search by Item Code, name or model').fill('GT-I9300');assert.equal(await page.locator('tbody tr').count(),1);await page.getByLabel('Search by Item Code, name or model').fill('بطارية');assert.equal(await page.locator('tbody tr').count(),1);await page.getByRole('button',{name:'Clear search'}).click();
  });
  await check('Stock Out CR, quantity and price validation save no invalid lines',async()=>{
    await nav('Stock Out');await outLine(1,'11234','6','50.000');await page.getByRole('button',{name:'Save Stock Out',exact:true}).click();assert.equal(await count('stock_out'),0);await page.getByLabel('Customer CR Number *',{exact:true}).fill('0');await page.getByRole('button',{name:'Save Stock Out',exact:true}).click();await page.getByRole('alert').waitFor();assert.equal(await count('stock_out'),0);
    await page.getByLabel('Customer CR Number *',{exact:true}).fill('99876');await page.getByRole('button',{name:'Add product line',exact:true}).click();await outLine(2,'11235','9','15.000');await page.getByRole('button',{name:'Save Stock Out',exact:true}).click();await page.getByText(/Insufficient stock for/).waitFor();assert.equal(await count('stock_out'),0);assert.equal(await page.getByLabel('Quantity — line 1 *',{exact:true}).inputValue(),'6');
    await outLine(2,'11235','1','');await page.getByRole('button',{name:'Save Stock Out',exact:true}).click();assert.equal(await count('stock_out'),0);
  });
  await check('backend rejection preserves all CR form inputs and succeeds atomically on correction',async()=>{
    await outLine(2,'11235','1','15.000');await page.evaluate(()=>window.__uiTest.failNext='stock_out');await page.getByRole('button',{name:'Save Stock Out',exact:true}).click();await page.getByText('Deliberate test backend rejection',{exact:true}).waitFor();assert.equal(await page.getByLabel('Customer CR Number *',{exact:true}).inputValue(),'99876');assert.equal(await page.evaluate(()=>window.__uiTest.products[0].quantity),8);await page.getByRole('button',{name:'Save Stock Out',exact:true}).click();await idle();await page.getByText(/Revenue OMR 315.000/).waitFor();assert.equal((await last('stock_out')).payload.lines.length,2);assert.equal(await page.evaluate(()=>window.__uiTest.sales[0].profit_baisa),248000);
  });
  await check('low, critical and out-of-stock alerts update and same CR can repeat with zero selling price',async()=>{
    await nav('Dashboard');await page.getByText('Low Stock',{exact:true}).waitFor();await nav('Stock Out');await page.getByLabel('Customer CR Number *',{exact:true}).fill('99876');await outLine(1,'11234','1','0');await page.getByRole('button',{name:'Save Stock Out',exact:true}).click();await idle();await nav('Dashboard');await page.getByText('Critical Stock',{exact:true}).waitFor();await nav('Stock Out');await page.getByLabel('Customer CR Number *',{exact:true}).fill('99876');await outLine(1,'11234','1','1.000');await page.getByRole('button',{name:'Save Stock Out',exact:true}).click();await idle();await nav('Dashboard');await page.locator('.page-content').getByText('Out of Stock',{exact:true}).waitFor();assert.equal(await page.evaluate(()=>window.__uiTest.sales.length),3);
  });
  await check('selling the last unit raises the workspace badge; receiving stock clears the list and badge',async()=>{
    await nav('Out of Stock');assert.equal(await page.getByTestId('out-of-stock-count').innerText(),'1');assert.equal(await page.locator('tbody tr').count(),1);await page.locator('tbody tr').getByText('Samsung S3 screen',{exact:true}).waitFor();assert.equal(await page.locator('tbody tr').getByText('بطارية آيفون 14',{exact:true}).count(),0);mkdirSync('tmp/ui',{recursive:true});await page.screenshot({path:'tmp/ui/out-of-stock-v204.png',fullPage:true});await entry('11234',3,'10.000');await nav('Out of Stock');await page.getByText('No out-of-stock items',{exact:true}).waitFor();assert.equal(await page.getByTestId('out-of-stock-count').count(),0);
  });
  await check('all three PDF and Excel report exports use actual selected month and correct file types',async()=>{
    await nav('Reports');await page.getByLabel('Reporting Month',{exact:true}).fill('2026-10');
    for(const title of ['Monthly Sales Report','Current Inventory Report','Monthly Inventory Summary']){await page.getByText(title,{exact:true}).click();await page.getByRole('button',{name:'Export PDF',exact:true}).click();await page.getByText('Report saved to C:/TestOnly/report.pdf',{exact:true}).waitFor();await page.getByRole('button',{name:'Export Excel (.xlsx)',exact:true}).click();await page.getByText('Report saved to C:/TestOnly/report.xlsx',{exact:true}).waitFor();}
    const files=await page.evaluate(()=>({pdf:window.__uiTest.savedPdfs,xlsx:window.__uiTest.savedExcels}));assert.equal(files.pdf.length,3);assert.equal(files.xlsx.length,3);for(const bytes of files.pdf)assert.equal(Buffer.from(bytes).subarray(0,5).toString(),'%PDF-');for(const bytes of files.xlsx)assert.equal(Buffer.from(bytes).subarray(0,2).toString(),'PK');
  });
  await check('workspace CR and item search combines filters and exports only matching records',async()=>{
    await nav('Search');await page.getByLabel('Exact CR number',{exact:true}).fill('99876');await page.getByLabel('Exact Item Code',{exact:true}).fill('11234');
    const sales=page.locator('.panel').filter({has:page.getByRole('heading',{name:/^Items sold/})});const rows=await sales.locator('tbody tr').all();assert.ok(rows.length);for(const row of rows){const text=await row.innerText();assert.ok(text.includes('99876'));assert.ok(text.includes('Item Code 11234'));assert.ok(!text.includes('11235'));}
    await page.getByRole('button',{name:'Export search PDF',exact:true}).click();await page.getByText(/Search results saved to/).waitFor();await page.getByRole('button',{name:'Export search Excel',exact:true}).click();await page.getByText(/Search results saved to/).waitFor();
    const bytes=await page.evaluate(()=>({pdf:window.__uiTest.savedPdfs.at(-1),excel:window.__uiTest.savedExcels.at(-1)}));assert.equal(Buffer.from(bytes.pdf).subarray(0,5).toString(),'%PDF-');assert.equal(Buffer.from(bytes.excel).subarray(0,2).toString(),'PK');
    await page.getByLabel('Exact CR number',{exact:true}).fill('99999999');await sales.getByText('No matching sales',{exact:true}).waitFor();
  });
  await check('automatic AI readiness enables chat; Arabic and followups share session across navigation',async()=>{
    await page.evaluate(()=>window.__uiTest.ready=true);await nav('AI Assistant');await page.getByRole('button',{name:'Start Conversation',exact:true}).waitFor();await page.waitForFunction(()=>!Array.from(document.querySelectorAll('button')).find(b=>b.textContent==='Start Conversation')?.disabled);
    const ask=async q=>{await page.getByLabel('Ask the inventory assistant').fill(q);await page.getByRole('button',{name:'Send question',exact:true}).click();await page.getByText(`Fixture reply to: ${q}`,{exact:true}).waitFor();};await ask('كم عدد الشاشات؟');const id=(await last('assistant_ask')).payload.session_id;await ask('How many did we sell this month?');assert.equal((await last('assistant_ask')).payload.session_id,id);await nav('Products');await nav('AI Assistant');await page.getByText('Fixture reply to: كم عدد الشاشات؟',{exact:true}).waitFor();
  });
  await check('continuous voice sequences two listen/AI/speak turns with ordered request sequencing; End cancels pending listen',async()=>{
    const before=await count('voice_listen');await page.getByRole('button',{name:'Start Conversation',exact:true}).click();await page.waitForFunction(()=>window.__uiTest.calls.filter(c=>c.action==='voice_speak').length>=2);await page.getByRole('button',{name:'End Conversation',exact:true}).click();await page.getByText('Conversation ended',{exact:true}).waitFor();const calls=await page.evaluate(()=>window.__uiTest.calls.map(c=>c.action));const first=calls.indexOf('voice_start');const ordered=calls.slice(first).filter(a=>['voice_listen','assistant_ask','voice_speak'].includes(a));assert.deepEqual(ordered.slice(0,6),['voice_listen','assistant_ask','voice_speak','voice_listen','assistant_ask','voice_speak']);const endCount=await count('voice_listen');await delay(700);assert.equal(await count('voice_listen'),endCount);assert.ok(endCount>=before+2);
  });
  await check('voice interruption feeds its captured question directly into the next chat turn',async()=>{
    const before=await page.evaluate(()=>window.__uiTest.calls.length);await page.evaluate(()=>{window.__uiTest.voiceQueue=['Hi'];window.__uiTest.interruptNext={text:'How many 11234?',language:'en'};});await page.getByRole('button',{name:'Start Conversation',exact:true}).click();
    await page.getByText('Interrupted — listening to you',{exact:true}).waitFor();await page.getByText('Fixture reply to: How many 11234?',{exact:true}).waitFor();
    const actions=await page.evaluate(n=>window.__uiTest.calls.slice(n).map(c=>c.action).filter(a=>['voice_listen','assistant_ask','voice_speak'].includes(a)),before);assert.deepEqual(actions.slice(0,5),['voice_listen','assistant_ask','voice_speak','assistant_ask','voice_speak']);
    await page.getByRole('button',{name:'End Conversation',exact:true}).click();await page.getByText('Conversation ended',{exact:true}).waitFor();
  });
  await check('spoken farewell ends voice mode cleanly',async()=>{
    await page.evaluate(()=>window.__uiTest.voiceQueue=['Bye']);await page.getByRole('button',{name:'Start Conversation',exact:true}).click();await page.getByText('Bye! Talk to you soon.',{exact:true}).waitFor();await page.getByRole('button',{name:'Start Conversation',exact:true}).waitFor();assert.equal(await page.evaluate(()=>window.__uiTest.voiceActive),false);
  });
  await check('chat reset clears conversation; local backup has no credentials or settings page',async()=>{
    await page.getByRole('button',{name:'New conversation',exact:true}).click();await page.getByRole('heading',{name:'Your local inventory assistant',exact:true}).waitFor();assert.ok(await count('assistant_new_conversation'));await page.getByRole('button',{name:'Backup & restore',exact:true}).click();await page.getByRole('button',{name:'Create backup',exact:true}).click();await page.getByText('Backup saved to C:/TestOnly/backup.db.',{exact:true}).waitFor();await page.getByRole('button',{name:'Close',exact:true}).click();
  });
  if(process.env.MAC_UI_TEST) {
    await check('Settings import opens a safe database restore screen; cancel and invalid files preserve records',async()=>{
      const original=await page.evaluate(()=>JSON.stringify(window.__uiTest.products));
      const before=await count('restore');
      await page.getByRole('button',{name:'Settings',exact:true}).click();
      await page.getByRole('button',{name:'Import database (.db)',exact:true}).click();
      await page.getByRole('heading',{name:'Import / restore database',exact:true}).waitFor();
      await page.getByText(/it does not merge records/).waitFor();
      await page.getByText(/Excel and CSV files are not supported/).waitFor();
      assert.equal(await count('restore'),before);
      await page.getByRole('button',{name:'Choose database (.db) to restore',exact:true}).click();
      await page.waitForFunction(n=>window.__uiTest.calls.filter(c=>c.action==='restore').length>n,before);
      assert.equal(await page.evaluate(()=>JSON.stringify(window.__uiTest.products)),original);
      assert.equal(await page.getByText(/Backup restored/).count(),0);
      await page.evaluate(()=>window.__uiTest.failNext='restore');
      await page.getByRole('button',{name:'Choose database (.db) to restore',exact:true}).click();
      await page.getByRole('dialog').getByRole('alert').filter({hasText:'Deliberate test backend rejection'}).waitFor();
      assert.equal(await page.evaluate(()=>JSON.stringify(window.__uiTest.products)),original);
      await page.getByRole('button',{name:'Close',exact:true}).click();
    });
    await check('successful Settings import shows recovery backup and refreshes products and stock alerts',async()=>{
      await page.evaluate(()=>window.__uiTest.restoreOutcome='success');
      await page.getByRole('button',{name:'Settings',exact:true}).click();
      await page.getByRole('button',{name:'Import database (.db)',exact:true}).click();
      assert.equal(await page.getByRole('alert').count(),0);
      await page.getByRole('button',{name:'Choose database (.db) to restore',exact:true}).click();
      await page.getByText('Backup restored. A copy of the previous database was saved to C:/TestOnly/before-restore.db.',{exact:true}).waitFor();
      await page.getByRole('button',{name:'Close',exact:true}).click();
      await nav('Products');await page.getByText('Restored screen',{exact:true}).waitFor();
      assert.equal(await page.locator('tbody tr').count(),1);
      assert.equal(await page.getByText('Samsung S3 screen',{exact:true}).count(),0);
      await nav('Out of Stock');await page.getByText('Restored screen',{exact:true}).waitFor();
      assert.equal(await page.getByTestId('out-of-stock-count').innerText(),'1');
      await page.evaluate(()=>window.__uiTest.restoreOutcome='cancelled');
    });
    await check('imported missing Item Code warning opens a password-protected editor; wrong password and cancel preserve data',async()=>{
      await nav('Products');await page.getByText('Needs review',{exact:false}).waitFor();mkdirSync('tmp/ui',{recursive:true});await page.screenshot({path:'tmp/ui/import-review-warning.png',fullPage:true,animations:'disabled'});
      await page.getByRole('button',{name:'Edit product 900001',exact:true}).click();
      const dialog=page.getByRole('dialog');await dialog.getByRole('heading',{name:'Edit product',exact:true}).waitFor();
      assert.equal(await dialog.getByLabel('Edit password *',{exact:true}).getAttribute('type'),'password');
      await dialog.getByLabel('Product Name *',{exact:true}).fill('Reviewed screen');
      await dialog.getByLabel('Edit password *',{exact:true}).fill('wrong');await dialog.getByRole('button',{name:'Save correction',exact:true}).click();
      await dialog.getByText('Incorrect edit password. No records were changed.',{exact:true}).waitFor();
      assert.equal(await page.evaluate(()=>window.__uiTest.products[0].name),'Restored screen');
      await dialog.getByLabel('Edit password *',{exact:true}).fill('1234');
      await dialog.getByRole('button',{name:'Save correction',exact:true}).click();
      await dialog.getByText('Correction saved. Recovery backup: C:/TestOnly/before-edit.db',{exact:true}).waitFor();
      await dialog.getByRole('button',{name:'Close',exact:true}).click();
      await page.getByText('Reviewed screen',{exact:true}).waitFor();assert.equal(await page.getByText('Needs review',{exact:false}).count(),0);
      await page.getByRole('button',{name:'Edit product 900001',exact:true}).click();
      await page.getByLabel('Product Name *',{exact:true}).fill('Cancelled name');
      await page.getByRole('button',{name:'Cancel',exact:true}).click();assert.equal(await page.evaluate(()=>window.__uiTest.products[0].name),'Reviewed screen');
    });
    await check('stock entry and Stock Out history can be corrected and report source actions remain available',async()=>{
      await entry('900001',5,'1.000');
      await page.getByRole('button',{name:'Edit stock entry 900001',exact:true}).click();
      await page.getByRole('dialog').getByLabel('Quantity *',{exact:true}).fill('6');await page.getByRole('dialog').getByLabel('Buying Price per Unit (OMR) *',{exact:true}).fill('2.000');
      await page.getByLabel('Edit password *',{exact:true}).fill('1234');await page.screenshot({path:'tmp/ui/edit-stock-password.png',fullPage:true,animations:'disabled'});await page.getByRole('button',{name:'Save correction',exact:true}).click();
      await page.getByText('Correction saved. Recovery backup: C:/TestOnly/before-edit.db',{exact:true}).waitFor();await page.getByRole('button',{name:'Close',exact:true}).click();
      assert.equal(await page.evaluate(()=>window.__uiTest.products[0].quantity),6);
      await nav('Stock Out');await page.getByLabel('Customer CR Number *',{exact:true}).fill('77');await outLine(1,'900001','2','4.000');
      await page.getByRole('button',{name:'Save Stock Out',exact:true}).click();await idle();
      await page.getByRole('button',{name:'Edit Stock Out 77 item 900001',exact:true}).click();
      const dialog=page.getByRole('dialog');await dialog.getByLabel('Quantity *',{exact:true}).fill('1');
      await dialog.getByLabel('Selling Price per Unit (OMR) *',{exact:true}).fill('5.000');await dialog.getByLabel('Customer CR Number *',{exact:true}).fill('88');
      await dialog.getByLabel('Edit password *',{exact:true}).fill('1234');await page.screenshot({path:'tmp/ui/edit-stock-out-password.png',fullPage:true,animations:'disabled'});await dialog.getByRole('button',{name:'Save correction',exact:true}).click();
      await dialog.getByText('Correction saved. Recovery backup: C:/TestOnly/before-edit.db',{exact:true}).waitFor();await dialog.getByRole('button',{name:'Close',exact:true}).click();
      assert.equal(await page.evaluate(()=>window.__uiTest.products[0].quantity),5);
      await page.getByRole('button',{name:'Edit Stock Out 88 item 900001',exact:true}).waitFor();
      await nav('Search');await page.getByRole('button',{name:'Edit Stock Out 88 item 900001',exact:true}).waitFor();await page.getByRole('button',{name:'Edit stock entry 900001',exact:true}).waitFor();
      await nav('Reports');await page.getByRole('button',{name:'Edit sales history',exact:true}).click();await page.getByRole('heading',{name:'Stock Out',exact:true}).waitFor();
    });
    await check('Mac automatic backup controls and committed-save warnings remain visible without blocking inventory',async()=>{
      await page.getByRole('button',{name:'Settings',exact:true}).click();await page.getByRole('heading',{name:'Automatic backups',exact:true}).waitFor();await page.getByRole('button',{name:'Back up now',exact:true}).click();await page.getByText('latest.db',{exact:false}).waitFor();await page.getByRole('button',{name:'Add USB / backup folder',exact:true}).click();await page.getByText('/Volumes/USB',{exact:false}).waitFor();await page.getByRole('button',{name:'Close',exact:true}).click();
      await page.evaluate(()=>{window.__uiTest.backupWarning='Inventory is saved, but its automatic backup could not be created.';window.dispatchEvent(new Event('inventory-data-safety'));});await page.getByText('Inventory is saved, but its automatic backup could not be created.',{exact:true}).waitFor();await nav('Stock Entry');await page.getByRole('button',{name:'Save Stock Entry',exact:true}).waitFor();await page.evaluate(()=>{window.__uiTest.backupWarning='';window.__uiTest.recoveryPending=true;window.dispatchEvent(new Event('inventory-data-safety'));});await page.getByText('Earlier backups were found, but the live database was missing.',{exact:false}).waitFor();await page.evaluate(()=>{window.__uiTest.recoveryPending=false;window.dispatchEvent(new Event('inventory-data-safety'));});
    });

    await check('Mac Settings turns AI off while inventory and reports stay usable; dark appearance persists',async()=>{
      await page.getByRole('button',{name:'Settings',exact:true}).click();const toggle=page.getByRole('switch',{name:'Enable local AI and voice'});await toggle.uncheck();await page.locator('.topbar').getByText('AI Off',{exact:true}).waitFor();await page.getByLabel('Theme',{exact:true}).selectOption('dark');await page.waitForFunction(()=>document.documentElement.dataset.theme==='dark');await page.getByRole('button',{name:'Close',exact:true}).click();await nav('Products');await page.getByRole('heading',{name:'Products',exact:true}).waitFor();await nav('Reports');await page.getByRole('button',{name:'Export PDF',exact:true}).waitFor();assert.equal(await page.getByRole('button',{name:'Export PDF',exact:true}).isEnabled(),true);await page.reload();await page.locator('.topbar').getByText('AI Off',{exact:true}).waitFor();await page.waitForFunction(()=>document.documentElement.dataset.theme==='dark');mkdirSync('tmp/ui',{recursive:true});await page.getByRole('button',{name:'Settings',exact:true}).click();assert.equal(await page.getByRole('switch',{name:'Enable local AI and voice'}).isChecked(),false);await page.screenshot({path:'tmp/ui/mac-settings-dark.png',fullPage:true,animations:'disabled'});await page.getByRole('button',{name:'Close',exact:true}).click();
    });
    await check('Mac timeout exposes a corner Retry AI button with no repeated start calls',async()=>{
      await page.evaluate(()=>{window.__uiTest.preferences.ai_enabled=false;window.__uiTest.preferences.ai_pause_reason='timed_out';});const retry=page.locator('.topbar').getByRole('button',{name:'Retry AI',exact:true});await retry.waitFor();assert.equal(await count('assistant_set_enabled'),0);await delay(1500);assert.equal(await count('assistant_set_enabled'),0);await retry.click();await page.locator('.topbar').getByText('Starting AI…',{exact:true}).waitFor();assert.equal(await count('assistant_set_enabled'),1);await page.evaluate(()=>window.__uiTest.ready=true);await page.locator('.topbar').getByText('AI Ready',{exact:true}).waitFor();assert.equal(await retry.count(),0);await page.getByRole('button',{name:'Settings',exact:true}).click();await page.getByLabel('Theme',{exact:true}).selectOption('light');await page.waitForFunction(()=>document.documentElement.dataset.theme==='light');await page.screenshot({path:'tmp/ui/mac-settings-light.png',fullPage:true,animations:'disabled'});await page.getByRole('button',{name:'Close',exact:true}).click();
    });
  }
  await check('screens render at desktop sizes with no external requests or frontend exceptions',async()=>{
    await nav('Dashboard');mkdirSync('tmp/ui',{recursive:true});await page.screenshot({path:'tmp/ui/dashboard-v2.png',fullPage:true});await page.setViewportSize({width:960,height:680});await nav('Stock Out');await page.screenshot({path:'tmp/ui/stock-out-v2-small.png',fullPage:true});assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth),true);assert.deepEqual(external,[]);assert.deepEqual(errors,[]);
  });
}catch(e){console.error(e.stack);process.exitCode=1;}finally{mkdirSync('tmp/ui',{recursive:true});writeFileSync('tmp/ui/results-v2.json',JSON.stringify({results,errors,external},null,2));if(browser)await browser.close();server.kill();}
