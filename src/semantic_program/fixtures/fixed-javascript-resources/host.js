import assert from 'node:assert/strict';
import {pathToFileURL} from 'node:url';

const api=await import(pathToFileURL(process.argv[2]));
const events=[];
assert.equal(api.run(1,2),3);
assert.equal(api.currentX(),1);
assert.equal(api.run(4294967301.5,-2.9),3);
const x={valueOf(){events.push('x');return 3.7;}};
const y={valueOf(){events.push('y');api.reset(100,200);return 4.9;}};
assert.equal(api.run(x,y),7);
assert.deepEqual(events,['y','x']);
assert.equal(api.currentX(),100);
events.length=0;
const sentinel={sentinel:true};
try {
  api.run(x,{valueOf(){events.push('throw-y');throw sentinel;}});
  assert.fail('source y conversion must throw');
} catch(error) {
  assert.equal(error,sentinel);
}
assert.deepEqual(events,['throw-y']);
assert.equal(api.run(-0,0),0);
process.stdout.write('fixed-resource-value-order-ok\n');
