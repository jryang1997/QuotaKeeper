import test from 'node:test';
import assert from 'node:assert/strict';
import { panelRect, project, releaseVelocity, rubberband } from './motion-tuning.js';

test('boundary resistance remains continuous and progressively softer', () => {
  assert.equal(rubberband(50, 0, 100), 50);
  assert.equal(rubberband(100, 0, 100), 100);
  const near = rubberband(120, 0, 100) - 100;
  const far = rubberband(200, 0, 100) - 100;
  assert.ok(near > 0 && near < 20);
  assert.ok(far > near && far < 100);
  assert.ok(far / 100 < near / 20);
  assert.ok(Math.abs(rubberband(-20, 0, 100) + near) < 1e-9);
});

test('a paused pointer has no stale fling velocity', () => {
  const samples = [{x:0,y:0,t:0},{x:20,y:10,t:20},{x:40,y:20,t:40}];
  assert.deepEqual(releaseVelocity(samples,40), {x:1000,y:500});
  assert.deepEqual(releaseVelocity(samples,120), {x:0,y:0});
  assert.ok(Math.abs(project(1000)-99) < 1e-9);
  assert.ok(Math.abs(project(-1000)+99) < 1e-9);
});

test('panel pixel geometry spans the board without accumulated column gaps', () => {
  assert.equal(panelRect({x:0,y:0,w:12,h:1},1200).width,1200);
  const last = panelRect({x:8,y:4,w:4,h:2},1200);
  assert.equal(last.x + last.width,1200);
  assert.equal(last.y,496);
  assert.equal(last.height,234);
});
