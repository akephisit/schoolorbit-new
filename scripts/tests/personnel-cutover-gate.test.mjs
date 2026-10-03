import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync } from 'node:fs';
import { execFileSync, spawn } from 'node:child_process';
const workflow=readFileSync('.github/workflows/deploy-school-release.yml','utf8');
const helper='scripts/verify_personnel_cutover.sh';
const fragment=execFileSync('sh',['-c','. scripts/verify_personnel_cutover.sh; schoolorbit_personnel_cutover_filter'],{encoding:'utf8'});
const fullGate=workflow.match(/migration_completion_filter="([\s\S]*?)"\n\s*if ! podman/)[1].replace('$personnel_cutover_filter',fragment).replaceAll('\\"','"').replaceAll('\\$','$');
async function accepts(filter,report) {
 return new Promise((resolve,reject)=>{
  const child=spawn('podman',['run','--rm','-i','ghcr.io/jqlang/jq:1.7.1','-e',filter],{stdio:['pipe','ignore','pipe']});
  let error='';child.stderr.on('data',chunk=>error+=chunk);
  child.on('error',reject);child.on('close',code=>{if(code===125)reject(new Error(error));else resolve(code===0);});
  child.stdin.end(JSON.stringify(report));
 });
}
export const acceptsPersonnelCutover=(cutover)=>accepts(fragment,{personnelCutover:cutover});
export const acceptsMigrationStatus=(report)=>accepts(fullGate,report);
const codes=['PERSONNEL_SIMPLIFICATION_STAFF_PRESERVED','PERSONNEL_SIMPLIFICATION_POSITIONS_PRESERVED','PERSONNEL_SIMPLIFICATION_EDUCATION_TEXT_PRESERVED','PERSONNEL_SIMPLIFICATION_UNRELATED_FIELDS_PRESERVED','PERSONNEL_SIMPLIFICATION_CANONICAL_SCHEMA_VALID','PERSONNEL_SIMPLIFICATION_RETIRED_OWNERS_REMOVED','PERSONNEL_MIGRATION_HISTORY_VALID'];
export const validCutoverFixture={migrationVersion:84,status:'cutoverCompleted',passed:true,checks:codes.map(code=>({code,passed:true,count:0}))};
const school={migration_version:84,migration_status:'migrated',migration_error:null,academicCoreCutover:{migrationVersion:45,status:'cleanupCompleted',passed:true,checks:[{passed:true}]},gradebookResultsCutover:{migrationVersion:60,status:'cutoverCompleted',passed:true,checks:[{passed:true}]},personnelCutover:validCutoverFixture};
const report={latest_version:84,total_schools:2,migrated:2,pending:0,failed:0,outdated:0,schools:[school,school]};
test('personnel gate accepts only complete current distinct evidence',async()=>{
 assert.equal(await acceptsPersonnelCutover(validCutoverFixture),true);
 for(const invalid of [null,{},... [81,83].map(migrationVersion=>({...validCutoverFixture,migrationVersion})),{...validCutoverFixture,status:'cutoverPending'},{...validCutoverFixture,checks:[]},{...validCutoverFixture,checks:validCutoverFixture.checks.slice(1)},{...validCutoverFixture,checks:validCutoverFixture.checks.map((check,i)=>i===1?validCutoverFixture.checks[0]:check)},...[-1,'0',1.5].map(count=>({...validCutoverFixture,checks:validCutoverFixture.checks.map((check,i)=>i===0?{...check,count}:check)})),{...validCutoverFixture,checks:validCutoverFixture.checks.map((check,i)=>i===0?{...check,passed:false}:check)}]) assert.equal(await acceptsPersonnelCutover(invalid),false);
});
test('release gate requires complete tenant coverage and unrelated successful cutovers',async()=>{
 assert.equal(await acceptsMigrationStatus(report),true);
 for(const invalid of [{...report,schools:[school]},...['pending','failed','outdated'].map(key=>({...report,[key]:1})),{...report,schools:[school,{...school,migration_version:83}]},{...report,schools:[school,{...school,academicCoreCutover:{...school.academicCoreCutover,passed:false}}]},{...report,schools:[school,{...school,gradebookResultsCutover:{...school.gradebookResultsCutover,status:'cutoverPending'}}]}]) assert.equal(await acceptsMigrationStatus(invalid),false);
});
