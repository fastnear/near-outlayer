export async function createQuickJS() {
  try {
    const mod = await import('./vendor/quickjs-emscripten-core.mjs');
    if (mod.getQuickJS) {
      const QuickJS = await mod.getQuickJS();
      const vm = QuickJS.createVm();
      return {
        evalQuickJS(code) {
          const res = vm.evalCode(code);
          if (res.error) {
            const err = vm.dump(res.error);
            res.error.dispose?.();
            return { ok: false, error: String(err) };
          } else {
            const v = vm.dump(res.value);
            res.value.dispose?.();
            return { ok: true, value: v };
          }
        }
      };
    }
  } catch {}
  try {
    const mod2 = await import('./vendor/quickjs-emscripten.mjs');
    if (mod2.getQuickJS) {
      const QuickJS = await mod2.getQuickJS();
      const vm = QuickJS.createVm();
      return {
        evalQuickJS(code) {
          const res = vm.evalCode(code);
          if (res.error) {
            const err = vm.dump(res.error);
            res.error.dispose?.();
            return { ok: false, error: String(err) };
          } else {
            const v = vm.dump(res.value);
            res.value.dispose?.();
            return { ok: true, value: v };
          }
        }
      };
    } else if (mod2.newQuickJSWASMModule) {
      const qmod = await mod2.newQuickJSWASMModule(mod2.RELEASE_SYNC);
      const ctx = qmod.newContext();
      return {
        evalQuickJS(code) {
          const res = ctx.evalCode(code);
          if (res.error) {
            const err = ctx.dump(res.error);
            ctx.free(res.error);
            return { ok: false, error: String(err) };
          } else {
            const v = ctx.dump(res.value);
            ctx.free(res.value);
            return { ok: true, value: v };
          }
        }
      };
    }
  } catch {}
  return {
    evalQuickJS(code) {
      try {
        const f = new Function(`return (function(){ "use strict"; return (${code}); })()`);
        return { ok: true, value: f() };
      } catch (e) { return { ok: false, error: String(e) }; }
    }
  };
}
