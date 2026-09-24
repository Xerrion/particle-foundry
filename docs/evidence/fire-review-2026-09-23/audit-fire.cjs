'use strict';
// Review harness only. Compiled imports are from the unmodified uploaded src(3).zip.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const build = process.env.FIRE_REVIEW_BUILD || path.join(__dirname, 'build');
const M = require(path.join(build, 'materials/index.js'));
const {createWorld} = require(path.join(build, 'simulation/world.js'));
const {createPhysics} = require(path.join(build, 'simulation/physics.js'));
const {createReactions} = require(path.join(build, 'physics/reactions.js'));
const {createElementReactions} = require(path.join(build, 'physics/element-reactions.js'));
const {energyAtTemperature} = require(path.join(build, 'physics/thermal.js'));
const {createBrush} = require(path.join(build, 'tools/brush.js'));
const {CELL_WIDTH_METERS, GRAVITY_M_PER_S2} = require(path.join(build, 'simulation/physical-scale.js'));
const results = [];
function sum(xs) { return xs.reduce((a,b)=>a+b,0); }
function fill(w,id) { for(let i=0;i<w.size;i++) w.setCell(i,id); }
function hot(w,i,t) {w.addExternalEnergy(i, energyAtTemperature(w.grid[i],t,undefined,w.massKg[i])-w.energy[i]);}
function energy(w) {let total=0;for(let i=0;i<w.size;i++) total+=w.energy[i]+1000*w.chemicalEnergyKj[i]+w.massKg[i]*(0.5*(w.velocityX[i]**2+w.velocityY[i]**2)+GRAVITY_M_PER_S2*(w.height-Math.floor(i/w.width)-0.5)*CELL_WIDTH_METERS);return total;}
function test(id,description,fn) {try {results.push({id,description,status:'observed-and-asserted',...fn()});}catch(e){results.push({id,description,status:'harness-failure',error:e.stack});}}
function sealed(n=7){const w=createWorld(n,n,{boundariesEnabled:true,seed:2026});fill(w,M.STONE);return w;}

test('F01','Closed boundary flag does not stop oxygen replenishment or wood combustion',()=>{
 const w=createWorld(7,7,{boundariesEnabled:true,seed:2026});const i=24;w.setCell(i,M.WOOD);hot(w,i,500);w.oxygenKg.fill(0);
 const r=createReactions(w);const ledger={...w.ledger};const chem=w.chemicalEnergyKj[i];r.beginStep();const replenished=sum(w.oxygenKg);const before=sum(w.oxygenKg);r.update(i);
 const releasedJ=(chem-w.chemicalEnergyKj[i])*1000;assert(replenished>0);assert(Math.abs(releasedJ-100)<1e-8);assert.equal(sum(w.oxygenKg),before);assert.deepEqual(w.ledger,ledger);
 return {classification:'bug',boundariesEnabled:w.boundariesEnabled,initialOxygenKg:0,replenishedOxygenKg:replenished,releasedJ,oxygenConsumedDuringBurnKg:before-sum(w.oxygenKg),ledgerDelta:0};
});

test('F02','The closed-boundary oxygen issue occurs through the real full physics tick',()=>{
 const w=createWorld(7,7,{boundariesEnabled:true,seed:2026});w.setCell(24,M.WOOD);hot(w,24,500);w.oxygenKg.fill(0);
 const p=createPhysics(w);const before=sum(w.chemicalEnergyKj);p.step();const releasedJ=(before-sum(w.chemicalEnergyKj))*1000;
 assert(Math.abs(releasedJ-100)<1e-8);return {classification:'integration-confirmation',releasedJ,oxygenAfterKg:sum(w.oxygenKg)};
});

test('F03','Explicit stone enclosure with no oxygen does stop wood combustion',()=>{
 const w=sealed();w.setCell(24,M.WOOD);hot(w,24,500);w.setCell(23,M.EMPTY);w.oxygenKg.fill(0);const r=createReactions(w);const before=w.chemicalEnergyKj[24];r.beginStep();r.update(24);
 assert.equal(w.chemicalEnergyKj[24],before);assert.equal(sum(w.oxygenKg),0);return {classification:'positive-control',releasedJ:0,burning:w.burning[24]};
});

test('F04','Generic combustion depletes an O2 inventory without converting O2 mass into products',()=>{
 const w=sealed();const f=24,o=23;w.setCell(f,M.WOOD);hot(w,f,300);w.setCell(o,M.OXYGEN);const r=createReactions(w);const before={oxygen:w.oxygenKg[o],oxygenMass:w.massKg[o],fuelMass:w.massKg[f],chemical:w.chemicalEnergyKj[f],energy:energy(w)};r.beginStep();r.update(f);
 assert.equal(w.oxygenKg[o],0);assert.equal(w.grid[o],M.OXYGEN);assert.equal(w.massKg[o],before.oxygenMass);assert.equal(w.massKg[f],before.fuelMass);assert(Math.abs(energy(w)-before.energy)<1e-8);
 return {classification:'composition-defect',oxygenConsumedKg:before.oxygen,releasedJ:(before.chemical-w.chemicalEnergyKj[f])*1000,oxygenMaterialStill:'OXYGEN',remainingOxygenCellMassKg:w.massKg[o],fuelMassLostKg:before.fuelMass-w.massKg[f],co2Cells:w.countMaterial(M.CARBON_DIOXIDE),waterOrSteamCells:w.countMaterial(M.WATER)+w.countMaterial(M.STEAM),totalTrackedEnergyErrorJ:energy(w)-before.energy};
});

test('F05','Wood-generated flames relabel ambient air without transporting fuel mass or chemical inventory',()=>{
 const w=createWorld(7,7,{seed:2026});const f=24;w.setCell(f,M.WOOD);hot(w,f,500);const r=createReactions(w);const mass=w.massKg[f];let target=-1;let targetBefore=null;
 for(let tick=0;tick<20&&target<0;tick++){w.moved.fill(0);r.beginStep();const before=w.grid.slice();const masses=w.massKg.slice();const chems=w.chemicalEnergyKj.slice();r.update(f);for(let i=0;i<w.size;i++)if(w.grid[i]===M.FIRE&&before[i]===M.EMPTY){target=i;targetBefore={mass:masses[i],chemical:chems[i]};break;}}
 assert(target>=0);assert.equal(w.massKg[target],targetBefore.mass);assert.equal(w.chemicalEnergyKj[target],0);assert.equal(w.massKg[f],mass);
 const gasEnergy=w.energy[target];w.moved.fill(0);r.update(target);assert.equal(w.energy[target],gasEnergy);
 return {classification:'model-limitation',flameMassKg:w.massKg[target],previousAirMassKg:targetBefore.mass,flameChemicalEnergyKj:w.chemicalEnergyKj[target],fuelMassLostKg:mass-w.massKg[f],flameHeatGeneratedJ:w.energy[target]-gasEnergy};
});

test('F06','A tiny water neighbor cancels burning without a thermal or phase-change debit in reactions',()=>{
 const w=createWorld(7,7,{seed:2026});const f=24,q=25;w.setCell(f,M.WOOD);hot(w,f,500);w.setCell(q,M.WATER);w.massKg[q]=1e-12;w.energy[q]=energyAtTemperature(M.WATER,22,undefined,w.massKg[q]);const r=createReactions(w);r.beginStep();const before={fuelEnergy:w.energy[f],waterEnergy:w.energy[q],chemical:w.chemicalEnergyKj[f]};r.update(f);
 assert.equal(w.burning[f],0);assert.equal(w.energy[f],before.fuelEnergy);assert.equal(w.energy[q],before.waterEnergy);assert.equal(w.chemicalEnergyKj[f],before.chemical);assert.equal(w.grid[q],M.WATER);
 return {classification:'model-limitation',fixture:'reaction-only; thermal solver excluded deliberately',waterMassKg:w.massKg[q],fuelTemperatureC:w.temperatureAt(f),burning:w.burning[f],fuelEnergyChangeJ:0,waterEnergyChangeJ:0};
});

test('F07','A cold FIRE label ignites hydrogen/O2 before the generic fire pass can quench it',()=>{
 const w=sealed();const h=24,o=25,f=23;w.setCell(h,M.HYDROGEN);w.setCell(o,M.OXYGEN);w.setCell(f,M.FIRE);hot(w,h,22);hot(w,o,22);hot(w,f,22);const before=sum(w.chemicalEnergyKj);const count=createElementReactions(w).step();const releasedJ=(before-sum(w.chemicalEnergyKj))*1000;assert.equal(count,1);assert(releasedJ>0);
 return {classification:'ignition-defect',initialReactantTemperatureC:22,initialFireTemperatureC:22,reactions:count,releasedJ};
});

test('F08','The cold FIRE ignition defect also occurs through the full ordered physics pipeline',()=>{
 const w=sealed();w.setCell(24,M.HYDROGEN);w.setCell(25,M.OXYGEN);w.setCell(23,M.FIRE);hot(w,24,22);hot(w,25,22);hot(w,23,22);const p=createPhysics(w);const before=sum(w.chemicalEnergyKj);p.step();const releasedJ=(before-sum(w.chemicalEnergyKj))*1000;assert(releasedJ>0);
 return {classification:'integration-confirmation',allInitialTemperaturesC:22,releasedJ};
});

test('F09','Hot hydrogen cannot consume oxygen from explicitly sealed ordinary air',()=>{
 const w=sealed();const h=24,a=25;w.setCell(h,M.HYDROGEN);hot(w,h,600);w.setCell(a,M.EMPTY);const before=w.chemicalEnergyKj[h];const oxygen=w.oxygenKg[a];const count=createElementReactions(w).step();assert.equal(count,0);assert.equal(w.chemicalEnergyKj[h],before);assert(oxygen>0);
 return {classification:'documented-model-limitation',hydrogenTemperatureC:w.temperatureAt(h),adjacentOxygenKg:oxygen,reactions:count};
});

test('F10','Expired smoke in a sealed cavity becomes ambient-air identity at unchanged mass and heat',()=>{
 const w=sealed();const i=24;w.setCell(i,M.SMOKE);w.lifetime[i]=1;const before={mass:w.massKg[i],energy:w.energy[i],oxygen:w.oxygenKg[i]};const r=createReactions(w);r.beginStep();r.update(i);assert.equal(w.grid[i],M.EMPTY);assert.equal(w.massKg[i],before.mass);assert.equal(w.energy[i],before.energy);assert.equal(w.oxygenKg[i],before.oxygen);
 return {classification:'composition-loss',finalMaterial:'EMPTY/Air',massPreserved:true,heatPreserved:true,oxygenAddedKg:w.oxygenKg[i]-before.oxygen};
});

test('F11','Ignition brush in air spends external heat, preserves mass, and expires without smoke',()=>{
 const w=sealed();const i=24;w.setCell(i,M.EMPTY);w.setCell(i+1,M.EMPTY);const b=createBrush(w,()=>{});b.setMaterial(M.FIRE);b.setSize(1);const before={mass:w.massKg[i],external:w.ledger.externalEnergyAdded};b.paintCircle(3,3);assert.equal(w.grid[i],M.FIRE);assert.equal(w.ignitionFlame[i],1);assert.equal(w.massKg[i],before.mass);assert.equal(w.chemicalEnergyKj[i],0);const added=w.ledger.externalEnergyAdded-before.external;const r=createReactions(w);const lifespan=w.lifetime[i];for(let t=0;t<lifespan;t++){w.moved.fill(0);r.beginStep();r.update(i);if(t+1<lifespan)assert.equal(w.grid[i],M.FIRE);}assert.equal(w.grid[i],M.EMPTY);assert.equal(w.countMaterial(M.SMOKE),0);
 return {classification:'positive-control',massPreserved:true,externalHeatAddedJ:added,expiredAfterTicks:lifespan,finalMaterial:'EMPTY/Air',smokeCells:w.countMaterial(M.SMOKE)};
});

test('F12','Explicit hot H2/O2 reaction conserves tracked mass, momentum, and total energy',()=>{
 const w=sealed();const h=24,o=25;w.setCell(h,M.HYDROGEN);w.setCell(o,M.OXYGEN);hot(w,h,600);w.velocityX[h]=2;w.velocityY[o]=-1;const before={mass:sum(w.massKg),energy:energy(w),px:sum(w.massKg.map((m,i)=>m*w.velocityX[i])),py:sum(w.massKg.map((m,i)=>m*w.velocityY[i]))};const count=createElementReactions(w).step();const after={mass:sum(w.massKg),energy:energy(w),px:sum(w.massKg.map((m,i)=>m*w.velocityX[i])),py:sum(w.massKg.map((m,i)=>m*w.velocityY[i]))};assert.equal(count,1);for(const k of Object.keys(before))assert(Math.abs(before[k]-after[k])<1e-8);
 return {classification:'positive-control',massErrorKg:after.mass-before.mass,trackedTotalEnergyErrorJ:after.energy-before.energy,momentumXErrorKgMPerS:after.px-before.px,momentumYErrorKgMPerS:after.py-before.py};
});

const output={sourceArchive:'src(3).zip',scope:'12 targeted headless checks, including 2 full physics-tick checks; not the original application test suite',results};
fs.writeFileSync(path.join(__dirname,'fire-audit-results.json'),JSON.stringify(output,null,2)+'\n');
console.log(JSON.stringify(output,null,2));
if(results.some(r=>r.status==='harness-failure'))process.exitCode=1;
