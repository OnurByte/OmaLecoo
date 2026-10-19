const fs = require('fs');
const vm = require('vm');
const assert = require('node:assert/strict');
const source = fs.readFileSync('plugin/Panel.qml', 'utf8');
function extract(name) {
  const start = source.indexOf('  function ' + name + '(');
  assert(start >= 0, name);
  let depth = 0, end = source.indexOf('{', start);
  for (; end < source.length; end++) {
    if (source[end] === '{') depth++;
    if (source[end] === '}' && --depth === 0) break;
  }
  return source.slice(start, end + 1);
}
function panel() {
  const root = {queue:[],busy:false,stateSyncPending:false,stateGeneration:0,
    effect:2,brightness:1,speed:0,select:0,panelCount:20,panelNames:[],binary:'lecoo-gk301'};
  const context = vm.createContext({root,worker:{},console:{log(){}}});
  for (const name of ['enqueue','runNext','setEffect','pushPanel','parseStatus','effectLabel']) {
    vm.runInContext(extract(name),context); root[name] = context[name];
  }
  return {root,context};
}
{
  const {root,context} = panel();
  root.enqueue(['profile-load','gaming']);
  assert.equal(context.worker.command.at(-1),'--panel-only');
  root.setEffect(13);
  assert.equal(root.effect,2,'profile transition must block stale effect edits');
  assert.equal(root.queue.length,0);
  root.busy=false;
  root.parseStatus(JSON.stringify({effect:19,brightness:0}),root.stateGeneration-1);
  assert.equal(root.effect,2,'stale response must not replace settings');
  assert.equal(root.stateSyncPending,true);
  root.parseStatus(JSON.stringify({effect:4,brightness:5,speed:2,select:1}),root.stateGeneration);
  assert.equal(root.stateSyncPending,false);
  root.setEffect(13);
  assert.equal(root.effect,13);
  assert.equal(context.worker.command[4],'5','next effect must preserve loaded brightness');
}
for (const command of [['preset','night'],['restore']]) {
  const {root,context}=panel();root.enqueue(command);
  assert.equal(context.worker.command.at(-1),'--panel-only');
}
console.log('UI profile synchronization, stale-response handling and panel-only routing passed.');
