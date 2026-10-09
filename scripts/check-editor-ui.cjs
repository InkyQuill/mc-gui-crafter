// Browser-mock interaction regressions. Requires Playwright and a running Vite server.
const {chromium}=require('playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await chromium.launch({headless:true,executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE});
 const page=await browser.newPage({viewport:{width:1280,height:900}});
 await page.goto(process.env.MCGUI_TEST_URL || 'http://127.0.0.1:49325');
 await page.evaluate(async()=>{const {project}=await import('/src/lib/stores/project.svelte.ts');await project.newProject('Review checks',176,166,'forge');for (const e of [...project.elements]) await project.removeElement(e.id);await project.addElement('slot',20,20);});
 async function endpoints(open, name){
  await open(); const dialog=page.getByRole('dialog');await dialog.waitFor();
  const b=await dialog.boundingBox();const inside={x:b.x+b.width/2,y:b.y+5};
  await page.mouse.move(2,2);await page.mouse.down();await page.mouse.move(inside.x,inside.y);await page.mouse.up();
  assert.equal(await dialog.count(),1,`${name}: outside-inside closed`);
  await page.mouse.move(inside.x,inside.y);await page.mouse.down();await page.mouse.move(2,2);await page.mouse.up();
  assert.equal(await dialog.count(),1,`${name}: inside-outside closed`);
  await page.mouse.click(2,2);await dialog.waitFor({state:'detached'}); console.log('PASS endpoints:',name);
 }
 await endpoints(()=>page.getByRole('button',{name:'New',exact:true}).click(),'new');
 await endpoints(()=>page.getByRole('button',{name:'Export',exact:true}).click(),'export');
 await endpoints(()=>page.locator('button[title*="Preferences"]').click(),'preferences');
 await endpoints(()=>page.locator('button[title*="shortcut" i]').click(),'shortcuts');
 await page.getByText('Add slot grid',{exact:true}).click();
 for (const [label,valid] of [['Cols','9'],['Rows','3'],['Step','18']]) {
   const count=await page.evaluate(async()=>(await import('/src/lib/stores/project.svelte.ts')).project.elements.length);
   await page.locator('.slot-grid-tool').getByLabel(label,{exact:true}).fill('');
   await page.locator('.slot-grid-add').click();
   assert.equal(await page.evaluate(async()=>(await import('/src/lib/stores/project.svelte.ts')).project.elements.length),count);
   await page.locator('.slot-grid-tool').getByLabel(label,{exact:true}).fill(valid);
 }
 console.log('PASS cleared grid inputs do not create elements');
 await page.getByRole('button',{name:'Assets',exact:true}).click();
 const opener=page.getByRole('button',{name:'+ Load Texture Pack',exact:true});await opener.click();
 const pack=page.getByRole('dialog',{name:'Minecraft Style'});await pack.waitFor();
 assert(await pack.evaluate(e=>e.contains(document.activeElement)),'focus not inside');
 for(let i=0;i<25;i++){await page.keyboard.press('Tab');assert(await pack.evaluate(e=>e.contains(document.activeElement)),'Tab escaped');}
 for(let i=0;i<25;i++){await page.keyboard.press('Shift+Tab');assert(await pack.evaluate(e=>e.contains(document.activeElement)),'Shift Tab escaped');}
 await page.keyboard.press('Escape');await pack.waitFor({state:'detached'});assert(await opener.evaluate(e=>e===document.activeElement));console.log('PASS texture-pack keyboard focus');
 await page.evaluate(async()=>{
   const {project,assetDataUrls}=await import('/src/lib/stores/project.svelte.ts');
   const api=await import('/src/lib/api.ts');const canvas=document.createElement('canvas');canvas.width=16;canvas.height=16;
   const data=canvas.toDataURL();const asset=await api.assetImport('review.png',project.activeProjectId,data);
   await project.syncFromBackend();assetDataUrls.set(asset.name,data);
 });
 await endpoints(()=>page.locator('.asset-item').filter({hasText:'review'}).locator('button[title="Click to edit"]').click(),'pixel editor');
 await page.getByRole('button',{name:'Layers',exact:true}).click();
 const row=page.locator('.layer-item').first();await row.click({button:'right',position:{x:5,y:5}});
 const menu=page.getByRole('menu');await menu.waitFor();assert(await menu.evaluate(e=>e.contains(document.activeElement)));
 assert.equal(await page.locator(':focus').innerText(),'Hide');
 await page.keyboard.press('Escape');
 await row.dispatchEvent('contextmenu',{clientX:1278,clientY:898});await menu.waitFor();await page.waitForTimeout(50);
 const box=await menu.boundingBox();assert(box.x>=0 && box.y>=0 && box.x+box.width<=1280 && box.y+box.height<=900);console.log('PASS single-element menu focus and bounds');
 await page.keyboard.press('Escape');
 await browser.close();
})().catch(e=>{console.error(e);process.exit(1)});
